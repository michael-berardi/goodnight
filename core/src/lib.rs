//! Good Night's engine, written once for every platform.
//!
//! The shells (SwiftUI on macOS, the Tauri window on Windows) own the screen, the tray or
//! menu bar, shortcuts and login items. This crate owns everything else: what the settings
//! mean, which warmth to show now, when the sun rises, and the colour pipeline that turns a
//! level and a brightness into a ramp.

pub mod sun;
pub mod warmth;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub use carapace;
use carapace::{App, Cx, Queries};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use warmth::{clamp, kelvin_label, nearest};

/// Never start on a screen too dark to find the controls.
pub const LAUNCH_BRIGHTNESS: f64 = 0.3;
const CLOCK: Duration = Duration::from_secs(60);

/// What is saved between launches. Same meaning on every platform.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub enabled: bool,
    pub follow_sun: bool,
    /// Warmth when not following the sun.
    pub manual_level: f64,
    /// Warmth by day when following the sun.
    pub day_level: f64,
    /// Warmth by night when following the sun.
    pub night_level: f64,
    pub brightness: f64,
    pub auto_update: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            follow_sun: true,
            manual_level: 0.45,
            day_level: 0.0,
            night_level: 0.68,
            brightness: 1.0,
            auto_update: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Saved settings from the last run.
    pub settings: Settings,
    /// IANA name of the system time zone, e.g. "America/New_York".
    pub time_zone: String,
    /// Current offset of the system time zone from UTC, in seconds.
    pub utc_offset_seconds: i32,
    /// Freeze the clock at this unix time (seconds). For tests and screenshots only.
    pub fixed_now: Option<f64>,
}

/// What the screen should show right now.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub level: f64,
    pub brightness: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PresetInfo {
    pub name: String,
    pub level: f64,
    pub kelvin: u32,
}

/// Everything the UI renders. Times are unix milliseconds so each UI formats them in the user's locale.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub enabled: bool,
    pub follow_sun: bool,
    pub auto_update: bool,
    /// The warmth the user sees and edits, 0 (no tint) … 1 (deep night).
    pub level: f64,
    pub brightness: f64,
    pub kelvin: u32,
    /// Name of the preset nearest to `level`.
    pub preset: String,
    pub presets: Vec<PresetInfo>,
    /// What the display should actually show: no tint and full brightness when switched off.
    pub target: Target,
    pub place_name: String,
    pub sunrise: f64,
    pub sunset: f64,
    pub next_warms: bool,
    pub next_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    /// `animated: false` while a slider is being dragged.
    SetLevel {
        level: f64,
        animated: bool,
    },
    SetBrightness {
        value: f64,
        animated: bool,
    },
    /// Relative change from the shortcuts. Both are animated.
    Nudge {
        level: f64,
        brightness: f64,
    },
    SetEnabled {
        on: bool,
    },
    SetFollowSun {
        on: bool,
    },
    SetAutoUpdate {
        on: bool,
    },
    /// The system time zone changed (travel, daylight saving).
    SetTimeZone {
        zone: String,
        utc_offset_seconds: i32,
    },
    /// Re-read the clock: after wake from sleep, or from the core's own minute timer.
    Refresh,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    /// Put `state.target` on the screen, easing over a second when `animated`.
    Render { animated: bool },
    /// Save these settings.
    Persist { settings: Settings },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Query {
    /// Colour numbers for one warmth level.
    Warmth { level: f64 },
    /// The full tint for one display: its own calibration curve (three tables of 0…1) in, the
    /// ramp to apply out.
    Ramp {
        level: f64,
        brightness: f64,
        r: Vec<f64>,
        g: Vec<f64>,
        b: Vec<f64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Answer {
    #[serde(rename_all = "camelCase")]
    Warmth {
        kelvin: f64,
        kelvin_label: u32,
        dim: f64,
        /// Per-channel gains, gamma-encoded.
        gains: [f64; 3],
        /// How white looks once the tint is on (gains times dim), for drawing the UI.
        tint: [f64; 3],
    },
    Ramp {
        r: Vec<f64>,
        g: Vec<f64>,
        b: Vec<f64>,
    },
}

pub struct GoodNight {
    s: Settings,
    level: f64,
    zone: String,
    offset: i32,
    place: sun::Place,
    fixed_now: Option<f64>,
}

impl GoodNight {
    fn now(&self) -> f64 {
        self.fixed_now.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0)
        })
    }

    fn nightness(&self) -> f64 {
        sun::nightness(&self.place, self.now(), self.offset)
    }

    fn auto_level(&self) -> f64 {
        self.s.day_level + (self.s.night_level - self.s.day_level) * self.nightness()
    }

    fn target(&self) -> (f64, f64) {
        if self.s.enabled {
            (self.level, self.s.brightness)
        } else {
            (0.0, 1.0)
        }
    }

    fn set_zone(&mut self, zone: String, offset: i32) {
        self.place = sun::place(&zone, offset);
        self.zone = zone;
        self.offset = offset;
    }

    fn persist(&self, cx: &mut Cx<Self>) {
        cx.emit(Event::Persist {
            settings: self.s.clone(),
        });
    }

    fn render(&self, cx: &mut Cx<Self>, animated: bool) {
        cx.emit(Event::Render { animated });
    }

    fn set_level(&mut self, level: f64) {
        self.level = clamp(level, 0.0, 1.0);
        if self.s.follow_sun {
            if self.nightness() >= 0.5 {
                self.s.night_level = self.level;
            } else {
                self.s.day_level = self.level;
            }
        } else {
            self.s.manual_level = self.level;
        }
        self.s.enabled = true;
    }
}

impl App for GoodNight {
    type State = State;
    type Action = Action;
    type Event = Event;
    type Config = Config;
    const NAME: &'static str = "GoodNightEngine";

    fn init(config: Config, cx: &mut Cx<Self>) -> Self {
        let mut s = config.settings;
        s.brightness = s.brightness.max(LAUNCH_BRIGHTNESS);
        let mut app = GoodNight {
            level: s.manual_level,
            place: sun::place(&config.time_zone, config.utc_offset_seconds),
            zone: config.time_zone,
            offset: config.utc_offset_seconds,
            fixed_now: config.fixed_now,
            s,
        };
        if app.s.follow_sun {
            app.level = app.auto_level();
        }
        cx.every("clock", CLOCK, Action::Refresh);
        app.render(cx, true);
        app
    }

    fn update(&mut self, action: Action, cx: &mut Cx<Self>) {
        match action {
            Action::SetLevel { level, animated } => {
                self.set_level(level);
                self.persist(cx);
                self.render(cx, animated);
            }
            Action::SetBrightness { value, animated } => {
                self.s.brightness = clamp(value, 0.0, 1.0);
                self.s.enabled = true;
                self.persist(cx);
                self.render(cx, animated);
            }
            Action::Nudge { level, brightness } => {
                if level != 0.0 {
                    self.set_level(self.level + level);
                }
                if brightness != 0.0 {
                    self.s.brightness = clamp(self.s.brightness + brightness, 0.0, 1.0);
                    self.s.enabled = true;
                }
                self.persist(cx);
                self.render(cx, true);
            }
            Action::SetEnabled { on } => {
                self.s.enabled = on;
                self.persist(cx);
                self.render(cx, true);
            }
            Action::SetFollowSun { on } => {
                self.s.follow_sun = on;
                self.level = if on {
                    self.auto_level()
                } else {
                    self.s.manual_level
                };
                self.persist(cx);
                self.render(cx, true);
            }
            Action::SetAutoUpdate { on } => {
                self.s.auto_update = on;
                self.persist(cx);
            }
            Action::SetTimeZone {
                zone,
                utc_offset_seconds,
            } => {
                self.set_zone(zone, utc_offset_seconds);
                cx.send(Action::Refresh);
            }
            Action::Refresh => {
                if self.s.follow_sun {
                    let target = self.auto_level();
                    if (target - self.level).abs() > 0.0005 {
                        self.level = target;
                        self.render(cx, true);
                    }
                }
            }
        }
    }

    fn state(&self) -> State {
        let now = self.now();
        let (rise, set) = sun::events(&self.place, now, self.offset);
        let (next_warms, next_at) = sun::next_change(&self.place, now, self.offset);
        let (level, brightness) = self.target();
        State {
            enabled: self.s.enabled,
            follow_sun: self.s.follow_sun,
            auto_update: self.s.auto_update,
            level: self.level,
            brightness: self.s.brightness,
            kelvin: kelvin_label(self.level),
            preset: nearest(self.level).name.to_string(),
            presets: warmth::PRESETS
                .iter()
                .map(|p| PresetInfo {
                    name: p.name.to_string(),
                    level: p.level,
                    kelvin: kelvin_label(p.level),
                })
                .collect(),
            target: Target { level, brightness },
            place_name: self.place.name.clone(),
            sunrise: rise * 1000.0,
            sunset: set * 1000.0,
            next_warms,
            next_at: next_at * 1000.0,
        }
    }
}

impl Queries for GoodNight {
    type Query = Query;
    type Answer = Answer;

    fn query(query: Query) -> Answer {
        match query {
            Query::Warmth { level } => {
                let level = clamp(level, 0.0, 1.0);
                Answer::Warmth {
                    kelvin: warmth::kelvin(level),
                    kelvin_label: kelvin_label(level),
                    dim: warmth::dim(level),
                    gains: warmth::gains(level),
                    tint: warmth::tint(level),
                }
            }
            Query::Ramp {
                level,
                brightness,
                r,
                g,
                b,
            } => {
                let [r, g, b] = warmth::ramp(
                    clamp(level, 0.0, 1.0),
                    clamp(brightness, 0.0, 1.0),
                    [&r, &g, &b],
                );
                Answer::Ramp { r, g, b }
            }
        }
    }
}

carapace::export!(GoodNight, queries);
