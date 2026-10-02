use goodnight_core::*;

use carapace::Engine;

/// 2026-09-23 12:00 in New York (16:00 UTC, UTC-4): daylight.
const NOON_NY: f64 = 1_790_179_200.0;
const NY: (&str, i32) = ("America/New_York", -14400);

fn config(settings: Settings, now: f64) -> Config {
    Config {
        settings,
        time_zone: NY.0.into(),
        utc_offset_seconds: NY.1,
        fixed_now: Some(now),
    }
}

fn start(settings: Settings, now: f64) -> (Engine<GoodNight>, Vec<String>) {
    let (e, out) = Engine::<GoodNight>::start(config(settings, now));
    let events = out
        .events
        .iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .collect();
    (e, events)
}

fn json(e: &Engine<GoodNight>) -> serde_json::Value {
    serde_json::from_str(e.snapshot()).unwrap()
}

fn events(out: &carapace::Outcome<GoodNight>) -> Vec<String> {
    out.events
        .iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .collect()
}

#[test]
fn brightness_never_starts_below_the_launch_floor() {
    let (e, _) = start(
        Settings {
            brightness: 0.0,
            ..Settings::default()
        },
        NOON_NY,
    );
    assert_eq!(json(&e)["brightness"], LAUNCH_BRIGHTNESS);
}

#[test]
fn following_the_sun_is_day_level_at_noon_and_night_level_after_dark() {
    let (day, _) = start(Settings::default(), NOON_NY);
    assert_eq!(json(&day)["level"], 0.0);
    let (night, _) = start(Settings::default(), NOON_NY + 9.0 * 3600.0);
    assert_eq!(json(&night)["level"], 0.68);
}

#[test]
fn setting_the_level_edits_the_slot_for_the_current_time_of_day_and_persists() {
    let (mut e, _) = start(Settings::default(), NOON_NY + 9.0 * 3600.0);
    let out = e.dispatch(Action::SetLevel {
        level: 0.9,
        animated: false,
    });
    assert_eq!(json(&e)["level"], 0.9);
    let ev = events(&out);
    assert!(
        ev.iter()
            .any(|x| x.contains(r#""nightLevel":0.9"#) && x.contains("persist")),
        "{ev:?}"
    );
    assert!(
        ev.iter()
            .any(|x| x == r#"{"type":"render","animated":false}"#),
        "{ev:?}"
    );
}

#[test]
fn manual_mode_uses_the_manual_level_and_turning_on_follow_returns_to_the_schedule() {
    let (mut e, _) = start(
        Settings {
            follow_sun: false,
            ..Settings::default()
        },
        NOON_NY,
    );
    assert_eq!(json(&e)["level"], 0.45);
    e.dispatch(Action::SetFollowSun { on: true });
    assert_eq!(json(&e)["level"], 0.0);
}

#[test]
fn switching_off_targets_no_tint_but_keeps_the_levels() {
    let (mut e, _) = start(
        Settings {
            follow_sun: false,
            ..Settings::default()
        },
        NOON_NY,
    );
    e.dispatch(Action::SetEnabled { on: false });
    let v = json(&e);
    assert_eq!(v["target"]["level"], 0.0);
    assert_eq!(v["target"]["brightness"], 1.0);
    assert_eq!(v["level"], 0.45);
}

#[test]
fn nudges_clamp_and_turn_the_tint_on() {
    let (mut e, _) = start(
        Settings {
            enabled: false,
            follow_sun: false,
            ..Settings::default()
        },
        NOON_NY,
    );
    for _ in 0..30 {
        e.dispatch(Action::Nudge {
            level: 0.05,
            brightness: -0.1,
        });
    }
    let v = json(&e);
    assert_eq!(v["level"], 1.0);
    assert_eq!(v["brightness"], 0.0);
    assert_eq!(v["enabled"], true);
}

#[test]
fn refresh_animates_when_the_sun_moves_the_level() {
    let (mut e, _) = start(Settings::default(), NOON_NY);
    // Same instant: nothing to do.
    assert!(events(&e.dispatch(Action::Refresh)).is_empty());
}

#[test]
fn changing_time_zone_recomputes_the_place_and_the_level() {
    let (mut e, _) = start(Settings::default(), NOON_NY);
    assert!(json(&e)["placeName"]
        .as_str()
        .unwrap()
        .starts_with("New York"));
    // Same instant, but Tokyo: it is 01:00 there, so night.
    let out = e.dispatch(Action::SetTimeZone {
        zone: "Asia/Tokyo".into(),
        utc_offset_seconds: 32400,
    });
    let v = json(&e);
    assert!(v["placeName"].as_str().unwrap().starts_with("Tokyo"));
    assert_eq!(v["level"], 0.68);
    assert!(events(&out).iter().any(|x| x.contains("render")));
}

#[test]
fn state_reports_schedule_times_for_the_ui_to_format() {
    let (e, _) = start(Settings::default(), NOON_NY);
    let v = json(&e);
    let (rise, set) = (
        v["sunrise"].as_f64().unwrap(),
        v["sunset"].as_f64().unwrap(),
    );
    assert!(rise < NOON_NY * 1000.0 && NOON_NY * 1000.0 < set);
    assert_eq!(v["nextWarms"], true);
    assert_eq!(v["nextAt"].as_f64().unwrap(), set);
    assert_eq!(v["presets"].as_array().unwrap().len(), 6);
}

#[test]
fn queries_are_pure_and_the_ramp_pipeline_matches_the_scalar_functions() {
    use carapace::Queries;
    let Answer::Warmth {
        kelvin,
        gains,
        tint,
        ..
    } = GoodNight::query(Query::Warmth { level: 0.68 })
    else {
        panic!()
    };
    assert!((kelvin - warmth::kelvin(0.68)).abs() < 1e-9);
    assert_eq!(gains, warmth::gains(0.68));
    assert_eq!(tint, warmth::tint(0.68));
    let base: Vec<f64> = (0..256).map(|i| f64::from(i) / 255.0).collect();
    let Answer::Ramp { r, .. } = GoodNight::query(Query::Ramp {
        level: 1.0,
        brightness: 1.0,
        r: base.clone(),
        g: base.clone(),
        b: base,
    }) else {
        panic!()
    };
    assert!((r[255] - warmth::knee(1.0, warmth::dim(1.0)) * warmth::gains(1.0)[0]).abs() < 1e-12);
}

#[test]
fn the_schema_exports_and_generates_bindings() {
    let schema = carapace::schema_with_queries::<GoodNight>();
    let text = serde_json::to_string(&schema).unwrap();
    let g = carapace_codegen::generate(&text).unwrap();
    assert!(g.swift.contains("public enum GoodNightEngine: CarapaceApp"));
    assert!(g.typescript.contains("export interface State"));
}
