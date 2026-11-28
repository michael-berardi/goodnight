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
    core.subscribe(move |n| {
        let Notice::State(state) = n else { return };
        let state = state.to_string();
        let work = Box::new(move || drop(state)) as Box<dyn FnOnce() + Send>;
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
        let mut carried_out = 0;
        while let Ok(work) = queued.try_recv() {
            work();
            carried_out += 1;
        }
        let _ = done.send((changed, carried_out));
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
}

/// A sync `#[tauri::command]` runs on the UI thread on Windows. Every command that waits for the
/// engine must be async, so that wait happens on Tauri's async runtime instead.
#[test]
fn windows_setting_commands_run_off_the_ui_thread() {
    let main = read("windows/src-tauri/src/main.rs");
    for command in [
        "set_level",
        "set_brightness",
        "set_enabled",
        "set_follow_sun",
        "set_auto_update",
        "set_install_updates_automatically",
    ] {
        let marker = format!("#[tauri::command]\nasync fn {command}(");
        assert!(
            main.contains(&marker),
            "{command} must stay an async command: a sync command runs on the UI thread and waits \
             for the engine there, which is what hung Good Night (J-4889)"
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
    let on_notice = body(&main, "fn on_notice(");
    for forbidden in [
        "set_tooltip",
        "tray_by_id",
        "run_item_main_thread",
        "block_on",
    ] {
        assert!(
            !on_notice.contains(forbidden),
            "the engine's notice callback must not call {forbidden}: it runs on the engine's thread"
        );
    }
}
