#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod gamma;

use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use goodnight_core::carapace::{Notice, Runtime};
use goodnight_core::warmth::{self, smoothstep};
use goodnight_core::{Action, Config, Event, GoodNight, Settings};

/// What the UI shows. Times are unix milliseconds so the UI formats them in the user's locale.
/// Almost everything comes from the shared engine (`goodnight-core`); the rest is Windows-only.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct View {
    enabled: bool,
    follow_sun: bool,
    level: f64,
    brightness: f64,
    kelvin: u32,
    preset: String,
    place_name: String,
    sunrise: f64,
    sunset: f64,
    next_warms: bool,
    next_at: f64,
    autostart: bool,
    error: Option<String>,
    version: &'static str,
    auto_update: bool,
    /// A newer signed release, when one was found.
    update: Option<String>,
}

enum Msg {
    Render { animated: bool },
    Quit,
}

struct State {
    /// The shared engine. Dropping it stops its thread.
    core: Runtime<GoodNight>,
    path: PathBuf,
    tx: Mutex<Sender<Msg>>,
    error: Mutex<Option<String>>,
    update: Mutex<Option<String>>,
}

impl State {
    fn core_state(&self) -> goodnight_core::State {
        serde_json::from_str(&self.core.snapshot()).expect("goodnight-core always writes valid state JSON")
    }

    /// (level, brightness) the screen should show now.
    fn target(&self) -> (f64, f64) {
        let t = self.core_state().target;
        (t.level, t.brightness)
    }

    fn act(&self, action: Action) {
        if let Err(e) = self.core.dispatch_wait(action) {
            eprintln!("goodnight: {e}");
        }
    }

    fn save(&self, s: &Settings) {
        let write = || -> std::io::Result<()> {
            if let Some(dir) = self.path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&self.path, serde_json::to_vec_pretty(s)?)
        };
        if let Err(e) = write() {
            eprintln!("goodnight: cannot save settings to {}: {e}", self.path.display());
        }
    }

    fn render(&self, animated: bool) {
        let _ = self.tx.lock().unwrap().send(Msg::Render { animated });
    }
}

fn view(app: &AppHandle) -> View {
    let st = app.state::<State>();
    let c = st.core_state();
    let error = st.error.lock().unwrap().clone();
    let update = st.update.lock().unwrap().clone();
    View {
        enabled: c.enabled,
        follow_sun: c.follow_sun,
        level: c.level,
        brightness: c.brightness,
        kelvin: c.kelvin,
        preset: c.preset,
        place_name: c.place_name,
        sunrise: c.sunrise,
        sunset: c.sunset,
        next_warms: c.next_warms,
        next_at: c.next_at,
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        error,
        version: env!("CARGO_PKG_VERSION"),
        auto_update: c.auto_update,
        update,
    }
}

/// Tell the UI and the tray about the latest state.
fn publish(app: &AppHandle) {
    let v = view(app);
    let _ = app.emit("state", &v);
    if let Some(tray) = app.tray_by_id("main") {
        let tip = if v.enabled { format!("Good Night · {}K · {}", v.kelvin, v.preset) } else { "Good Night · Off".into() };
        let _ = tray.set_tooltip(Some(tip));
    }
}

/// The engine speaks through notices: new state for the UI, and events for the shell.
fn on_notice(app: &AppHandle, notice: Notice<'_>) {
    match notice {
        Notice::State(_) => publish(app),
        Notice::Event(json) => match serde_json::from_str::<Event>(json) {
            Ok(Event::Render { animated }) => app.state::<State>().render(animated),
            Ok(Event::Persist { settings }) => app.state::<State>().save(&settings),
            Err(e) => eprintln!("goodnight: unreadable engine event {json}: {e}"),
        },
        Notice::Fault(text) => eprintln!("goodnight: engine fault: {text}"),
    }
}

/// Zone name and UTC offset of the system time zone right now.
fn system_zone() -> (String, i32) {
    (iana_time_zone::get_timezone().unwrap_or_default(), chrono::Local::now().offset().local_minus_utc())
}

// MARK: Commands

#[tauri::command]
fn state(app: AppHandle) -> View {
    view(&app)
}

/// The warmth curve sampled from the engine, so the UI draws exactly what the screen shows.
#[derive(Serialize)]
struct Curve {
    n: usize,
    kelvin: Vec<f64>,
    tint: Vec<[f64; 3]>,
}

#[tauri::command]
fn warmth_curve() -> Curve {
    const N: usize = 401;
    let (kelvin, tint) = warmth::curve(N);
    Curve { n: N, kelvin, tint }
}

#[tauri::command]
fn set_level(app: AppHandle, level: f64, animated: bool) {
    app.state::<State>().act(Action::SetLevel { level, animated });
}

#[tauri::command]
fn set_brightness(app: AppHandle, value: f64, animated: bool) {
    app.state::<State>().act(Action::SetBrightness { value, animated });
}

#[tauri::command]
fn set_enabled(app: AppHandle, on: bool) {
    app.state::<State>().act(Action::SetEnabled { on });
}

#[tauri::command]
fn set_follow_sun(app: AppHandle, on: bool) {
    app.state::<State>().act(Action::SetFollowSun { on });
}

#[tauri::command]
fn set_autostart(app: AppHandle, on: bool) -> Result<(), String> {
    let al = app.autolaunch();
    let r = if on { al.enable() } else { al.disable() };
    publish(&app);
    r.map_err(|e| format!("Couldn't change Start with Windows: {e}"))
}

#[tauri::command]
fn set_auto_update(app: AppHandle, on: bool) {
    app.state::<State>().act(Action::SetAutoUpdate { on });
}

/// Looks for a newer signed release. Returns its version, if any.
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let found = app.updater().map_err(|e| e.to_string())?.check().await
        .map_err(|e| format!("Couldn't check for updates: {e}"))?
        .map(|u| u.version);
    *app.state::<State>().update.lock().unwrap() = found.clone();
    let _ = app.emit("state", view(&app));
    Ok(found)
}

/// Downloads, verifies (minisign key in tauri.conf.json) and installs the update, then restarts.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let update = app.updater().map_err(|e| e.to_string())?.check().await
        .map_err(|e| format!("Couldn't check for updates: {e}"))?
        .ok_or("Good Night is up to date.")?;
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| format!("Couldn't install the update: {e}"))?;
    gamma::restore();
    app.restart();
}

#[tauri::command]
fn open_main(app: AppHandle) {
    show_main(&app);
}

#[tauri::command]
fn quit(app: AppHandle) {
    let _ = app.state::<State>().tx.lock().unwrap().send(Msg::Quit);
    std::thread::sleep(Duration::from_millis(80));
    app.exit(0);
}

fn nudge(app: &AppHandle, level: f64, brightness: f64) {
    app.state::<State>().act(Action::Nudge { level, brightness });
}

// MARK: Windows

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    // `--show-settings` opens the window on its settings, for testing.
    let page = if std::env::args().any(|a| a == "--show-settings") { "index.html?view=main&sheet=1" } else { "index.html?view=main" };
    let built = WebviewWindowBuilder::new(app, "main", WebviewUrl::App(page.into()))
        .title("Good Night")
        .inner_size(400.0, 660.0)
        .resizable(false)
        .maximizable(false)
        .decorations(false)
        .shadow(true)
        .center()
        .build();
    if let Err(e) = built {
        eprintln!("goodnight: cannot open the main window: {e}");
    }
}

/// The tray flyout, the Windows counterpart of the macOS menu bar popover.
/// Closed (not hidden) when it loses focus, so its web view is freed.
static FLYOUT_CLOSED: Mutex<Option<Instant>> = Mutex::new(None);

fn toggle_flyout(app: &AppHandle, click: PhysicalPosition<f64>) {
    if let Some(w) = app.get_webview_window("flyout") {
        let _ = w.close();
        return;
    }
    // Clicking the tray icon first blurs (and closes) an open flyout; that click should not reopen it.
    if FLYOUT_CLOSED.lock().unwrap().is_some_and(|t| t.elapsed() < Duration::from_millis(300)) {
        return;
    }
    // Height is refined by the page to fit its content.
    let (w, h) = (320.0, 480.0);
    let Ok(win) = WebviewWindowBuilder::new(app, "flyout", WebviewUrl::App("index.html?view=flyout".into()))
        .title("Good Night")
        .inner_size(w, h)
        .resizable(false)
        .decorations(false)
        .shadow(true)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .build()
    else {
        return eprintln!("goodnight: cannot open the tray flyout");
    };
    let scale = win.scale_factor().unwrap_or(1.0);
    let (pw, ph) = (w * scale, h * scale);
    let mut x = click.x - pw / 2.0;
    let mut y = click.y - ph - 16.0 * scale;
    if let Ok(Some(m)) = win.monitor_from_point(click.x, click.y) {
        let area = m.work_area();
        let (ax, ay) = (area.position.x as f64, area.position.y as f64);
        let (aw, ah) = (area.size.width as f64, area.size.height as f64);
        let margin = 12.0 * scale;
        x = x.clamp(ax + margin, ax + aw - pw - margin);
        // Taskbar at the top: open below the click instead.
        if y < ay { y = click.y + 16.0 * scale }
        y = y.clamp(ay + margin, ay + ah - ph - margin);
    }
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.show();
    let _ = win.set_focus();
    let handle = win.clone();
    win.on_window_event(move |e| {
        if let WindowEvent::Focused(false) = e {
            *FLYOUT_CLOSED.lock().unwrap() = Some(Instant::now());
            let _ = handle.close();
        }
    });
}

// MARK: Screen

/// One thread owns the screen: 1 s eased transitions at 60 fps, otherwise asleep. Once a
/// minute it follows the sun and re-applies the ramp, since games and driver resets can
/// replace it.
fn renderer(app: AppHandle, rx: mpsc::Receiver<Msg>) {
    let push = |app: &AppHandle, (l, b): (f64, f64)| {
        let r = gamma::apply(l, b);
        let st = app.state::<State>();
        let mut err = st.error.lock().unwrap();
        if r.as_ref().err() != err.as_ref() {
            if let Err(e) = &r {
                eprintln!("goodnight: {e}");
            }
            *err = r.err();
            drop(err);
            let _ = app.emit("state", view(app));
        }
    };
    let mut shown = (0.0, 1.0);
    let mut anim: Option<((f64, f64), (f64, f64), Instant)> = None;
    let mut displays = gamma::display_count();
    let mut last_tick = Instant::now();
    let mut last_zone = system_zone();
    loop {
        let wait = if anim.is_some() { Duration::from_millis(16) } else { Duration::from_secs(60) };
        match rx.recv_timeout(wait) {
            Ok(Msg::Render { animated }) => {
                let target = app.state::<State>().target();
                if animated {
                    anim = Some((shown, target, Instant::now()));
                } else {
                    anim = None;
                    shown = target;
                    push(&app, shown);
                }
            }
            Ok(Msg::Quit) | Err(RecvTimeoutError::Disconnected) => {
                gamma::restore();
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
        if let Some((from, to, start)) = anim {
            let p = (start.elapsed().as_secs_f64() / 1.0).min(1.0);
            let e = smoothstep(0.0, 1.0, p);
            shown = (from.0 + (to.0 - from.0) * e, from.1 + (to.1 - from.1) * e);
            push(&app, shown);
            if p >= 1.0 {
                anim = None;
            }
        } else if last_tick.elapsed() >= Duration::from_secs(55) {
            last_tick = Instant::now();
            let now = gamma::display_count();
            if now != displays {
                displays = now;
                gamma::capture();
            }
            // The engine follows the sun on its own clock; the shell tells it when the time zone moves.
            let zone = system_zone();
            if zone != last_zone {
                app.state::<State>().act(Action::SetTimeZone { zone: zone.0.clone(), utc_offset_seconds: zone.1 });
                last_zone = zone;
            }
            // Games and driver resets can replace the ramp: put ours back.
            push(&app, shown);
        }
    }
}

async fn tokio_sleep(d: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        gamma::restore();
        eprintln!("goodnight: {info}");
    }));

    let hidden = std::env::args().any(|a| a == "--hidden");
    let (tx, rx) = mpsc::channel();
    let mut rx = Some(rx);

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_main(app)))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec!["--hidden"])))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    match shortcut.key {
                        Code::ArrowUp => nudge(app, 0.0, 0.1),
                        Code::ArrowDown => nudge(app, 0.0, -0.1),
                        Code::ArrowRight => nudge(app, 0.05, 0.0),
                        Code::ArrowLeft => nudge(app, -0.05, 0.0),
                        _ => {}
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            state, set_level, set_brightness, set_enabled, set_follow_sun, set_autostart, set_auto_update,
            check_update, install_update, open_main, quit, warmth_curve
        ])
        .setup(move |app| {
            let path = app.path().app_config_dir()?.join("settings.json");
            let settings: Settings = std::fs::read(&path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).map_err(|e| eprintln!("goodnight: settings unreadable, using defaults: {e}")).ok())
                .unwrap_or_default();
            let (time_zone, utc_offset_seconds) = system_zone();
            let core = Runtime::<GoodNight>::start(Config { settings, time_zone, utc_offset_seconds, fixed_now: None });
            app.manage(State {
                core,
                path,
                tx: Mutex::new(tx.clone()),
                error: Mutex::new(None),
                update: Mutex::new(None),
            });
            // Subscribing replays what the engine said while starting (the first render).
            let notice_app = app.handle().clone();
            app.state::<State>().core.subscribe(move |n| on_notice(&notice_app, n));

            // Daily update check, first one a minute after launch.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut wait = Duration::from_secs(60);
                loop {
                    tokio_sleep(wait).await;
                    wait = Duration::from_secs(24 * 3600);
                    if handle.state::<State>().core_state().auto_update {
                        if let Err(e) = check_update(handle.clone()).await {
                            eprintln!("goodnight: {e}");
                        }
                    }
                }
            });

            gamma::capture();
            let handle = app.handle().clone();
            let rx = rx.take().expect("setup runs once");
            std::thread::Builder::new().name("renderer".into()).spawn(move || renderer(handle, rx))?;
            app.state::<State>().render(true);

            let mods = Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT);
            for code in [Code::ArrowUp, Code::ArrowDown, Code::ArrowRight, Code::ArrowLeft] {
                if let Err(e) = app.global_shortcut().register(Shortcut::new(mods, code)) {
                    eprintln!("goodnight: shortcut Ctrl+Alt+Shift+{code:?} is taken by another app: {e}");
                }
            }

            let open = MenuItem::with_id(app, "open", "Open Good Night", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "Turn On or Off", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &toggle, &PredefinedMenuItem::separator(app)?, &quit_item])?;
            TrayIconBuilder::with_id("main")
                .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .tooltip("Good Night")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id().as_ref() {
                    "open" => show_main(app),
                    "toggle" => {
                        let on = app.state::<State>().core_state().enabled;
                        set_enabled(app.clone(), !on);
                    }
                    "quit" => quit(app.clone()),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, position, .. } = e {
                        toggle_flyout(tray.app_handle(), position);
                    }
                })
                .build(app)?;
            publish(app.handle());

            if !hidden {
                show_main(app.handle());
            }
            // For testing the flyout without a tray click.
            if std::env::args().any(|a| a == "--show-flyout") {
                toggle_flyout(app.handle(), PhysicalPosition::new(1200.0, 1100.0));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Good Night failed to start");

    app.run(|app, event| match event {
        // Closing every window keeps Good Night running in the tray.
        RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
        RunEvent::Exit => {
            let _ = app.state::<State>().tx.lock().unwrap().send(Msg::Quit);
            gamma::restore();
        }
        _ => {}
    });
}
