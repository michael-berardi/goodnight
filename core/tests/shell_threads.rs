//! The Windows shell's thread contract (J-4889). On Windows the setting commands arrived on the
//! UI thread and waited there for the engine, while the engine reports every new state by calling
//! the shell's notice callback before it acknowledges the action. That callback set the tray
//! tooltip, which Tauri carries out on the UI thread and waits for: the UI thread waited for the
//! engine, the engine for the UI thread, and Good Night stopped responding on the first setting
//! click, leaving settings.json unwritten.
//!
//! The Windows crate only builds for Windows, so the shell cannot run in these tests. They hold
//! the two halves that can be checked on any machine: the engine's ordering (a behavioural test
//! with the real engine) and the shape of the shell source. The real Windows click is verified on
//! the release job's VM, not here.

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use carapace::{Notice, Runtime};
use goodnight_core::{Action, Config, GoodNight, Settings};

/// 2026-09-23 12:00 in New York: daylight, so a level change is visible in the state.
const NOON_NY: f64 = 1_790_179_200.0;

fn start() -> Runtime<GoodNight> {
    Runtime::start(Config {
        settings: Settings::default(),
        time_zone: "America/New_York".into(),
        utc_offset_seconds: -14400,
        fixed_now: Some(NOON_NY),
    })
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("core/ lives in the repository")
        .to_path_buf()
}

fn read(path: &str) -> String {
    fs::read_to_string(root().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// One function body from a Rust source file, from its `fn` line to the closing brace that starts
/// a line.
fn body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("no {signature}"));
    let rest = &source[start..];
    &rest[..rest
        .find("\n}\n")
        .expect("the function ends at a line-start brace")]
}

#[test]
fn the_engine_reports_a_state_before_it_acknowledges_the_action() {
    let core = start();
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let collect = seen.clone();
    core.subscribe(move |n| {
        if let Notice::State(state) = n {
            collect.lock().unwrap().push(state.to_string());
        }
    });
    let snapshot = core
        .dispatch_wait(Action::SetLevel {
            level: 0.4,
            animated: false,
        })
        .unwrap();
    let seen = seen.lock().unwrap();
    let last = seen
        .last()
        .expect("the subscriber was told about the new state before the wait returned");
    assert!(
        last.contains("\"level\":0.4"),
        "the notice carries the new state: {last}"
    );
    assert_eq!(
        last.as_str(),
        &*snapshot,
        "and it is the snapshot the caller gets back"
    );
}

/// The shape that hung Windows: the engine's callback needs the UI thread while the UI thread is
/// inside `dispatch_wait`. A callback that posts its UI work, as the shell does now, lets both
/// finish; one that waits for the UI thread never returns.
#[test]
fn a_setting_change_finishes_while_the_ui_thread_is_waiting_for_the_engine() {
    let core = start();
    // The UI thread's queue: tray work posted by the engine's callback.
    let (post, queued) = mpsc::channel::<Box<dyn FnOnce() + Send>>();
    let post = Mutex::new(post);
    let shown = Arc::new(Mutex::new(Vec::<String>::new()));
    let collect = shown.clone();
    core.subscribe(move |n| {
        let Notice::State(state) = n else { return };
        let state = state.to_string();
        let target = collect.clone();
        let work = Box::new(move || target.lock().unwrap().push(state)) as Box<dyn FnOnce() + Send>;
        // Posting, never waiting: waiting here is the J-4889 deadlock.
        let _ = post.lock().unwrap().send(work);
    });

    let ui_core = core.clone();
    let (done, finished) = mpsc::channel();
    let ui_thread = std::thread::spawn(move || {
        // The UI thread changes a setting, then carries out the work the change posted.
        let changed = ui_core
            .dispatch_wait(Action::SetLevel {
                level: 0.3,
                animated: false,
            })
            .is_ok();
        let mut carried_out = Vec::new();
        while let Ok(work) = queued.try_recv() {
            work();
            carried_out.push(());
        }
        let _ = done.send((changed, carried_out.len()));
    });
    let (changed, carried_out) = finished.recv_timeout(Duration::from_secs(10)).expect(
        "the engine was still waiting for the UI thread after 10 s: that is the J-4889 deadlock",
    );
    ui_thread.join().unwrap();
    assert!(changed, "the setting change succeeds");
    assert!(
        carried_out > 0,
        "the posted tray work runs once the UI thread is free"
    );
    // The work the callback posted carries the new state, so the tray and the UI show it.
    let shown = shown.lock().unwrap();
    assert!(
        shown.iter().any(|s| s.contains("\"level\":0.3")),
        "a posted state carries the new level: {shown:?}"
    );
}

/// Every command in the shell queues its action; none of them waits for the engine.
///
/// A sync `#[tauri::command]` runs on the UI thread on Windows, and an async one runs on Tauri's
/// async runtime. Either may queue, but a wait from either one is the J-4889 deadlock: the engine
/// answers through `on_notice`, whose tray update needs the UI thread.
#[test]
fn no_shell_command_waits_for_the_engine() {
    let main = read("windows/src-tauri/src/main.rs");
    let mut commands = 0;
    let mut rest = main.as_str();
    while let Some(at) = rest.find("#[tauri::command]") {
        rest = &rest[at + "#[tauri::command]".len()..];
        let end = rest.find("#[tauri::command]").unwrap_or(rest.len());
        let whole = &rest[..end];
        // Only the command's own body: an unindented closing brace ends the function.
        let block = &whole[..whole.find("\n}\n").map(|i| i + 1).unwrap_or(whole.len())];
        commands += 1;
        let name = block
            .split("fn ")
            .nth(1)
            .and_then(|s| s.split('(').next())
            .unwrap_or("?")
            .trim()
            .to_string();
        for forbidden in [".act(", "dispatch_wait"] {
            assert!(
                !block.contains(forbidden),
                "the {name} command must not call {forbidden}: a command runs on the UI thread or on \
                 the async runtime, and the engine's notice callback needs the UI thread to carry \
                 out the tray update, so the two wait on each other (J-4889)"
            );
        }
    }
    assert!(commands >= 12, "the shell's commands are all scanned: {commands} found");
    // The setting commands queue in arrival order, which is what the UI thread gives them.
    for command in [
        "set_level",
        "set_brightness",
        "set_enabled",
        "set_follow_sun",
        "set_auto_update",
        "set_install_updates_automatically",
    ] {
        let marker = format!("fn {command}(");
        let body = body(&main, &marker);
        assert!(
            body.contains("send(Action::"),
            "{command} must queue its action with State::send"
        );
    }
    // The tray menu and the global shortcuts arrive on the UI thread: they may only queue.
    assert!(
        main.contains("self.core.dispatch(action)"),
        "State::send must queue the action without waiting for the engine"
    );
    assert!(
        body(&main, "fn nudge(").contains(".send("),
        "the shortcuts queue their nudge"
    );
    assert!(
        main.contains("app.state::<State>().send(Action::SetEnabled { on: !on })"),
        "the tray menu queues its toggle"
    );
}

#[test]
fn windows_publish_posts_the_tray_tooltip_instead_of_waiting_for_it() {
    let main = read("windows/src-tauri/src/main.rs");
    let publish = body(&main, "fn publish(app: &AppHandle)");
    let post = publish
        .find("run_on_main_thread")
        .expect("the tray update goes through the UI thread");
    let tooltip = publish
        .find("set_tooltip")
        .expect("publish still updates the tray tooltip");
    assert!(
        post < tooltip,
        "set_tooltip must be called inside the closure handed to run_on_main_thread, not from the \
         engine's thread, which may not wait for the UI thread (J-4889)"
    );
    assert!(
        !publish[post..tooltip].contains(");"),
        "the tooltip call must be inside the posted closure, not after it"
    );
    // Nothing else on the engine's callback path may touch the tray or wait for the UI thread.
    for name in ["fn on_notice(", "fn view("] {
        let block = body(&main, name);
        for forbidden in [
            "set_tooltip",
            "tray_by_id",
            "run_item_main_thread",
            "dispatch_wait",
            ".act(",
            "recv_timeout",
            ".recv(",
            "block_on",
        ] {
            assert!(
                !block.contains(forbidden),
                "{name} must not call {forbidden}: it runs on the engine's thread or the UI \
                 thread and may not wait for the other one"
            );
        }
    }
}
