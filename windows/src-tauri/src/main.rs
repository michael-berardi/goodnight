#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod gamma;
mod updates;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tauri_plugin_updater::Update;
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
    install_updates_automatically: bool,
    /// A newer signed release, when one was found.
    update: Option<UpdateView>,
    /// Cause of the last failed check, shown in Settings rather than as an alert.
    update_error: Option<String>,
}

/// What the update prompt shows: both versions and the signed feed's release notes.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UpdateView {
    version: String,
    current: String,
    notice: String,
    notes: String,
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
    /// updates.json: what the update checks found (see updates.rs).
    cache: Mutex<updates::Cache>,
    /// The release the prompt shows.
    update: Mutex<Option<UpdateView>>,
    /// An opt-in update waiting for an idle moment.
    pending: Mutex<Option<(Update, UpdateView)>>,
    checking: AtomicBool,
}

/// Clears the in-flight flag even when a check fails.
struct CheckGuard<'a>(&'a AtomicBool);
impl Drop for CheckGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
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

    fn dir(&self) -> &std::path::Path {
        self.path.parent().unwrap_or(std::path::Path::new("."))
    }

    /// Remembers and saves the update cache.
    fn keep(&self, cache: updates::Cache) {
        if let Err(e) = cache.save(&self.dir().join("updates.json")) {
            eprintln!("goodnight: cannot save the update cache: {e}");
        }
        *self.cache.lock().unwrap() = cache;
    }

    /// Background update events only, in one bounded file.
    fn log(&self, line: &str) {
        let _ = updates::append_log(&self.dir().join("updates.log"), updates::now(), line);
    }
}

fn view(app: &AppHandle) -> View {
    let st = app.state::<State>();
    let c = st.core_state();
    let error = st.error.lock().unwrap().clone();
    let update = st.update.lock().unwrap().clone();
    let update_error = st.cache.lock().unwrap().last_error.clone();
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
        install_updates_automatically: c.install_updates_automatically,
        update,
        update_error,
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
    let st = app.state::<State>();
    st.act(Action::SetAutoUpdate { on });
    if !on {
        // No checks, no update state on disk. The NSIS uninstaller removes the same files.
        if let Some(dir) = st.path.parent() {
            updates::clear(dir);
        }
        *st.cache.lock().unwrap() = updates::Cache::default();
        *st.update.lock().unwrap() = None;
        *st.pending.lock().unwrap() = None;
        publish(&app);
    }
}

#[tauri::command]
fn set_install_updates_automatically(app: AppHandle, on: bool) {
    let st = app.state::<State>();
    st.act(Action::SetInstallUpdatesAutomatically { on });
    if !on {
        *st.pending.lock().unwrap() = None;
    }
}

/// The updater for one request: the fixed User-Agent, a 4 s check timeout, and the screen
/// restored before the installer takes over (on Windows it ends this process).
fn updater<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<tauri_plugin_updater::Updater, String> {
    use tauri_plugin_updater::UpdaterExt;
    let handle = app.clone();
    let before_exit = move || {
        gamma::restore();
        handle.cleanup_before_exit();
    };
    app.updater_builder()
        .header("User-Agent", updates::USER_AGENT)
        .and_then(|b| b.timeout(Duration::from_secs(4)).on_before_exit(before_exit).build())
        .map_err(|e| e.to_string())
}

/// Downloads with a longer timeout than the check, verifies the minisign signature against the
/// key in tauri.conf.json and only then runs the installer. A failure changes nothing.
async fn download_and_install(mut update: Update) -> Result<(), String> {
    update.timeout = Some(Duration::from_secs(600));
    update.download_and_install(|_, _| {}, || {}).await.map_err(|e| e.to_string())
}

/// One anonymous request to the signed feed (`latest.json`). Background checks run at most
/// once a day; "Check Now" always asks. A failure is kept in updates.json with its cause.
async fn check(app: &AppHandle, manual: bool) -> Result<Option<UpdateView>, String> {
    let st = app.state::<State>();
    if st.checking.swap(true, Ordering::SeqCst) {
        return Ok(st.update.lock().unwrap().clone());
    }
    let _guard = CheckGuard(&st.checking);
    let at = updates::now();
    let mut cache = st.cache.lock().unwrap().clone();
    if !manual && !cache.check_due(at) {
        return Ok(None);
    }
    cache.last_check = Some(at);
    let current = env!("CARGO_PKG_VERSION");
    let found = match updater(app) {
        Ok(u) => u.check().await.map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    let found = match found {
        Ok(found) => found.filter(|u| updates::is_newer(&u.version, current)),
        Err(e) => {
            let cause = format!("Couldn't check for updates: {e}");
            if !manual {
                st.log(&cause);
            }
            cache.last_error = Some(cause.clone());
            st.keep(cache);
            publish(app);
            return Err(cause);
        }
    };
    cache.last_error = None;
    let auto = st.core_state().install_updates_automatically;
    let shown = found.as_ref().filter(|u| cache.should_prompt(&u.version, current, at, manual)).map(|u| UpdateView {
        version: u.version.clone(),
        current: current.into(),
        notice: updates::notice_text("Good Night", &u.version, current),
        notes: u.body.clone().unwrap_or_default(),
    });
    if let Some(v) = &shown {
        cache.shown_version = Some(v.version.clone());
        cache.shown_at = Some(at);
    }
    st.keep(cache);
    // With automatic installs on, a background find waits for an idle moment instead of prompting.
    let background_install = !manual && auto && shown.is_some();
    *st.pending.lock().unwrap() = if background_install { found.zip(shown.clone()) } else { None };
    *st.update.lock().unwrap() = if background_install { None } else { shown.clone() };
    publish(app);
    Ok(shown)
}

/// Seconds since the last keyboard or mouse input.
#[cfg(windows)]
fn idle_seconds() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: `info` is a valid LASTINPUTINFO with its size set, as the call requires.
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    // SAFETY: GetTickCount has no preconditions.
    Some(u64::from(unsafe { GetTickCount() }.wrapping_sub(info.dwTime)) / 1000)
}

#[cfg(not(windows))]
fn idle_seconds() -> Option<u64> {
    None
}

/// Installs a pending opt-in update once the user is away and no Good Night window is open.
/// The installer replaces the app and starts the new version. If anything fails, the current
/// version keeps running and the prompt comes back with the cause.
fn install_when_idle(app: &AppHandle) {
    let st = app.state::<State>();
    let window_open = app.webview_windows().values().any(|w| w.is_visible().unwrap_or(false));
    if st.pending.lock().unwrap().is_none() || !updates::idle_enough(idle_seconds(), window_open) {
        return;
    }
    let Some((update, shown)) = st.pending.lock().unwrap().take() else { return };
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let st = handle.state::<State>();
        st.log(&format!("installing {}", update.version));
        if let Err(e) = download_and_install(update).await {
            let cause = format!("Automatic install failed: {e}");
            st.log(&cause);
            let mut cache = st.cache.lock().unwrap().clone();
            cache.last_error = Some(cause);
            st.keep(cache);
            *st.update.lock().unwrap() = Some(shown);
            publish(&handle);
        }
    });
}

#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<UpdateView>, String> {
    check(&app, true).await
}

/// "Remind Me Later": the prompt returns with the next daily check.
#[tauri::command]
fn dismiss_update(app: AppHandle) {
    *app.state::<State>().update.lock().unwrap() = None;
    publish(&app);
}

/// "Skip This Version": no background prompt for it until a newer release appears.
#[tauri::command]
fn skip_update(app: AppHandle, version: String) {
    let st = app.state::<State>();
    let mut cache = st.cache.lock().unwrap().clone();
    cache.skipped = Some(version);
    st.keep(cache);
    *st.update.lock().unwrap() = None;
    *st.pending.lock().unwrap() = None;
    publish(&app);
}

/// "Install Update": downloads, verifies and installs, then the new version starts.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let update = updater(&app)?.check().await
        .map_err(|e| format!("Couldn't check for updates: {e}"))?
        .ok_or("Good Night is up to date.")?;
    download_and_install(update).await.map_err(|e| format!("Couldn't install the update: {e}"))?;
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
            set_install_updates_automatically, check_update, install_update, dismiss_update, skip_update,
            open_main, quit, warmth_curve
        ])
        .setup(move |app| {
            let dir = app.path().app_config_dir()?;
            let path = dir.join("settings.json");
            let cache = updates::Cache::load(&dir.join("updates.json"));
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
                cache: Mutex::new(cache),
                update: Mutex::new(None),
                pending: Mutex::new(None),
                checking: AtomicBool::new(false),
            });
            // Subscribing replays what the engine said while starting (the first render).
            let notice_app = app.handle().clone();
            app.state::<State>().core.subscribe(move |n| on_notice(&notice_app, n));

            // Update checks are background work: the first one a minute after launch, then the
            // daily cache decides. CI and GOODNIGHT_NO_UPDATE_CHECK=1 turn them off.
            let handle = app.handle().clone();
            let background = updates::background_allowed(|k| std::env::var(k).ok());
            tauri::async_runtime::spawn(async move {
                tokio_sleep(Duration::from_secs(60)).await;
                updates::remove_staged_installers(&std::env::temp_dir(), &handle.package_info().name);
                loop {
                    if background && handle.state::<State>().core_state().auto_update {
                        let _ = check(&handle, false).await;
                        install_when_idle(&handle);
                    }
                    tokio_sleep(Duration::from_secs(60)).await;
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

#[cfg(test)]
mod tests {
    //! The real updater against a feed on 127.0.0.1 (no other network): the check is one
    //! anonymous request, and an installer whose signature does not match is refused before
    //! anything could be installed.
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::Arc;

    /// A throwaway minisign key made for these tests, not the release key.
    const TEST_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEFDNDlFNkY1MDBFNzlEQTIKUldTaW5lY0E5ZVpKclA5WjhtODd3NHQzVnhteVhXZW9yUks2Q0pwSlM5SHZWS3E5RWwwenhOeEwK";
    /// Its signature of `INSTALLER`.
    const SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVTaW5lY0E5ZVpKclBwSmVXRFNOdTVCeG51Tk1TaGJkZXNoTkVLNjZNVVlHbCthTEo4dFBSU1hBVlk0emdIRUpVeUovUDZJL3lYSXVoSnNteGtKamsvOFM1cDNIcFhJVGc0PQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzk1NzMwNTQxCWZpbGU6cGF5bG9hZC5iaW4KZTFNSkRTaXdDR0pJczM3VzlCMklFaXZJUlN6YTdpK0lldlRRZW44WmFoU0ZHNTV6MmgxT0VGSXBPN2ZtZ3VSTFlyT05SemlFbGV0SzFKSHU0RzJZQnc9PQo=";
    const INSTALLER: &[u8] = b"Good Night test installer\n";

    /// Serves latest.json and an installer body; returns the base URL and each request head.
    fn serve(installer: &'static [u8]) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (url, log) = (base.clone(), seen.clone());
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut head = String::new();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                while reader.read_line(&mut head).unwrap_or(0) > 2 {}
                let body = if head.starts_with("GET /latest.json ") {
                    let platform = serde_json::json!({ "signature": SIGNATURE, "url": format!("{url}/installer.exe") });
                    let platforms: serde_json::Map<String, serde_json::Value> = ["darwin-aarch64", "darwin-x86_64", "linux-x86_64", "windows-x86_64"]
                        .iter()
                        .map(|t| (t.to_string(), platform.clone()))
                        .collect();
                    serde_json::to_vec(&serde_json::json!({ "version": "9.9.9", "notes": "Fixes.", "platforms": platforms })).unwrap()
                } else {
                    installer.to_vec()
                };
                log.lock().unwrap().push(head);
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(&body);
            }
        });
        (base, seen)
    }

    fn app(base: &str) -> tauri::App<tauri::test::MockRuntime> {
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().plugins.0.insert(
            "updater".into(),
            serde_json::json!({ "pubkey": TEST_PUBKEY, "endpoints": [format!("{base}/latest.json")], "dangerousInsecureTransportProtocol": true }),
        );
        tauri::test::mock_builder().plugin(tauri_plugin_updater::Builder::new().build()).build(context).unwrap()
    }

    #[test]
    fn the_check_is_one_anonymous_request_and_a_tampered_installer_is_refused() {
        let (base, seen) = serve(b"Good Night test installer, tampered\n");
        let app = app(&base);
        tauri::async_runtime::block_on(async {
            let update = updater(app.handle()).unwrap().check().await.unwrap().expect("9.9.9 is newer");
            assert_eq!((update.version.as_str(), update.body.as_deref()), ("9.9.9", Some("Fixes.")));
            let refused = update.download(|_, _| {}, || {}).await.unwrap_err();
            assert!(matches!(refused, tauri_plugin_updater::Error::Minisign(_)), "{refused}");
        });
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "one feed request and one download");
        let check = seen[0].to_lowercase();
        assert!(check.starts_with("get /latest.json http/1.1\r\n"), "no query string: {check}");
        assert!(check.contains("\r\nuser-agent: goodnight-update-check\r\n"), "{check}");
        for header in check.lines().skip(1).filter_map(|l| l.split(':').next()).filter(|h| !h.is_empty()) {
            assert!(["host", "user-agent", "accept", "accept-encoding"].contains(&header), "unexpected header {header}");
        }
    }

    #[test]
    fn a_correctly_signed_installer_is_accepted() {
        let (base, _) = serve(INSTALLER);
        let app = app(&base);
        let bytes = tauri::async_runtime::block_on(async {
            let update = updater(app.handle()).unwrap().check().await.unwrap().unwrap();
            update.download(|_, _| {}, || {}).await.unwrap()
        });
        assert_eq!(bytes, INSTALLER);
    }
}
