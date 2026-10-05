// Quire — a local, GPU-accelerated, Notion-like document workspace.
// Single process: Slint UI on the main thread, Rust core behind it.
// All logic lives in the library crate (src/lib.rs); this binary only
// parses launch args, wires the window, and runs the event loop.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use quire::app::controller;
use quire::app::state::{AppState, HandleArgs};
use quire::platform;
use quire::platform::monitors::Rect as WindowGeometry;
use quire::services::logging;
use quire::AppWindow;
use slint::{ComponentHandle, Timer};

pub struct LaunchArgs {
    pub blocks: usize,
    pub auto_exit_secs: f64,
    /// Scene G: extra flat pages to switch between (--page-switch N).
    pub bench_pages: usize,
    /// Scene D/F with media: image rows on the bench page (--pictures N).
    pub pictures: usize,
    /// Scene D with inline marks: rows of the bench page carrying a bold mark
    /// (--marks N). Without it the bench page has no marks at all, and the
    /// runs channel a marked line is drawn from stays invisible to the gate.
    pub marks: usize,
    /// Scene D with highlighted code: code rows on the bench page
    /// (--code N), each carrying a language so its layers are drawn.
    pub code: usize,
    /// Scene F: programmatic continuous scroll (--scroll).
    pub scroll: bool,
    /// Scene F: how far one frame advances (--scroll-step, default 8 px). A
    /// page of tall image rows needs a wheel-flick-sized step to cross a
    /// picture at all, so the harness can ask.
    pub scroll_step: f32,
    /// Headless state setup for screenshots (--scene <name>).
    pub scene: Option<String>,
    /// Database file (--db <path>; default appdata/quire.db).
    pub db: Option<std::path::PathBuf>,
    /// Keep the library beside the working directory (--portable) instead of
    /// in the per-user profile (--db still wins, see storage::data_location).
    pub portable: bool,
    /// Markdown file to import and open at startup (--open <path>; also the
    /// bare positional, which is what the .md file association passes).
    pub open: Option<std::path::PathBuf>,
    /// Serve the workspace read-only on the LAN (--share [port], default
    /// 5877).
    pub share: Option<u16>,
    /// Pull a framed workspace from another Quire and import it
    /// (--pull http://host:5877).
    pub pull: Option<String>,
    /// Debug: print loaded page/block counts to stderr (--dump-state).
    pub dump_state: bool,
    /// Verification: close the main window to the tray, then run the Alt+N press
    /// and report what the platform shows (--quick-note). Nothing but a test
    /// harness asks for it; see ADR-0145.
    pub quick_note: bool,
    /// A2: measure the first painted frame and print one JSON line to stderr
    /// (--measure-startup). Normal runs never set this: no timer, no thread,
    /// no extra frame.
    pub measure_startup: bool,
}

fn parse_launch_args() -> LaunchArgs {
    let argv: Vec<String> = std::env::args().collect();
    let mut a = LaunchArgs {
        blocks: 0,
        auto_exit_secs: 0.0,
        bench_pages: 0,
        pictures: 0,
        marks: 0,
        code: 0,
        scroll: false,
        scroll_step: 8.0,
        scene: None,
        db: None,
        portable: false,
        dump_state: false,
        quick_note: false,
        open: None,
        share: None,
        pull: None,
        measure_startup: false,
    };
    let mut i = 1;
    while i < argv.len() {
        match (argv[i].as_str(), argv.get(i + 1)) {
            ("--blocks", Some(v)) => {
                a.blocks = v.parse().unwrap_or(0);
                i += 1;
            }
            ("--auto-exit", Some(v)) => {
                a.auto_exit_secs = v.parse().unwrap_or(0.0);
                i += 1;
            }
            ("--page-switch", Some(v)) => {
                a.bench_pages = v.parse().unwrap_or(0);
                i += 1;
            }
            ("--pictures", Some(v)) => {
                a.pictures = v.parse().unwrap_or(0);
                i += 1;
            }
            ("--marks", Some(v)) => {
                a.marks = v.parse().unwrap_or(0);
                i += 1;
            }
            ("--code", Some(v)) => {
                a.code = v.parse().unwrap_or(0);
                i += 1;
            }
            ("--scroll", _) => {
                a.scroll = true;
            }
            ("--scroll-step", Some(v)) => {
                a.scroll_step = v.parse().unwrap_or(8.0f32).max(1.0);
                i += 1;
            }
            ("--scene", Some(v)) => {
                a.scene = Some(v.clone());
                i += 1;
            }
            ("--db", Some(v)) => {
                a.db = Some(std::path::PathBuf::from(v));
                i += 1;
            }
            ("--portable", _) => {
                a.portable = true;
            }
            ("--dump-state", _) => {
                a.dump_state = true;
            }
            ("--quick-note", _) => {
                a.quick_note = true;
            }
            ("--open", Some(v)) => {
                a.open = Some(std::path::PathBuf::from(v));
                i += 1;
            }
            ("--share", _) => {
                a.share = Some(quire::services::lan_server::DEFAULT_PORT);
            }
            ("--share-port", Some(v)) => {
                if let Ok(p) = v.parse() {
                    a.share = Some(p);
                }
                i += 1;
            }
            ("--pull", Some(v)) => {
                a.pull = Some(v.clone());
                i += 1;
            }
            ("--measure-startup", _) => {
                a.measure_startup = true;
            }
            (positional, _) if !positional.starts_with('-') => {
                if a.open.is_none() {
                    a.open = Some(std::path::PathBuf::from(positional));
                }
            }
            _ => {}
        }
        i += 1;
    }
    a
}

/// Keep a timer alive for the whole run (dropping a Timer cancels it).
fn leak_timer(t: Timer) {
    std::mem::forget(t);
}

/// The "show the main window" answer, packaged for a thread that is not the
/// UI's: the native tray's left click and menu, and the Alt+M hotkey, all hop
/// through here. Clearing `minimized` first is what makes this a *restore* for
/// a window the user minimized to the taskbar rather than closed to the tray —
/// `show()` alone would bring it back still minimized.
fn show_window_closure(ui: &AppWindow) -> Box<dyn Fn() + Send> {
    let ui_w = ui.as_weak();
    Box::new(move || {
        // the outer closure is an `Fn` — callable many times — so the inner
        // hop clones the weak rather than moving the only copy out of it
        let ui_w = ui_w.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_w.upgrade() {
                ui.window().set_minimized(false);
                let _ = ui.show();
            }
        });
    })
}

/// Keep the 速记 pop-up out of the taskbar, on the press that shows it (ADR-0145).
///
/// On *every* press rather than once, because the bit does not survive the window
/// being put down: winit owns the extended style and rewrites the whole of it from
/// its own flags whenever any of them changes — and `VISIBLE` is one of them, so
/// the hide that ends the composer takes `WS_EX_TOOLWINDOW` with it and hands the
/// window back to the shell as an application window. A write on the first show
/// only would therefore work exactly once. Showing is the only path that makes
/// this window visible, so re-writing it there is the whole rule; `--quick-note`
/// reads the bit back off the live window and `hide_from_taskbar` skips a window
/// that already carries it.
///
/// Twice per press because of one sentence in Slint's own documentation: the
/// window handle only becomes reachable after the event loop has run a frame past
/// `show()`. The first try is the frame the press is in, and the retry is the hop
/// later, for the press that built the HWND right now.
///
/// Cosmetic when both fail, so the answer is a log line and not a dropped note.
fn quick_note_keep_off_taskbar(window: &quire::QuickNoteWindow) {
    if platform::hide_from_taskbar(window.window()).is_ok() {
        return;
    }
    let weak = window.as_weak();
    let t = Timer::default();
    t.start(
        slint::TimerMode::SingleShot,
        std::time::Duration::from_millis(60),
        move || {
            if let Some(window) = weak.upgrade() {
                if let Err(e) = platform::hide_from_taskbar(window.window()) {
                    logging::warn(&format!("速记 taskbar: {e}"));
                }
            }
        },
    );
    leak_timer(t);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `--quit` is not a session. It asks the *running* instance to end its own
    // — through the same route the tray menu's 「退出」 uses, so that instance's
    // flush runs and it writes its clean-exit record — and then leaves without
    // a log line, a database or a window of its own. Answering it here, before
    // the UI thread exists, is what keeps it from becoming a second session,
    // and the exit code is the answer a deploy reads: 0 only if an instance
    // accepted, non-zero if nothing was listening (ADR-0105).
    if std::env::args().skip(1).any(|a| a == "--quit") {
        return match quire::platform::quit::request_quit() {
            Ok(()) => {
                println!("quire: the running instance accepted the quit request");
                Ok(())
            }
            Err(reason) => Err(reason.into()),
        };
    }
    let start = std::time::Instant::now();
    // The UI event loop runs on its own thread with a generous stack:
    // Slint 1.18 evaluates the initial property/layout bindings of the
    // component tree recursively on the C stack, and Quire's shell sits
    // just above the 1 MB Windows default in debug builds (popups add
    // more when they open). 8 MB is the standard Linux default and costs
    // nothing but address-space reservation. See DECISIONS.md ADR-0009.
    let child = std::thread::Builder::new().stack_size(8 * 1024 * 1024)
        .spawn(move || real_main(start)).expect("spawn UI thread");
    child.join().map_err(|_| "UI thread panicked".to_string())??;
    Ok(())
}

/// A2 follow-up · phase stamps (--measure-startup only).
///
/// `first_paint_ms` says how long the start took, not where. This records the
/// wall time of each step of `real_main` so the window-up → first-paint gap
/// gets decomposed instead of guessed at. One `Instant::now()` per mark, and
/// normal runs pass `None` and never build it.
#[derive(Clone)]
struct PhaseLog {
    start: std::time::Instant,
    last: std::rc::Rc<std::cell::RefCell<std::time::Instant>>,
    rows: std::rc::Rc<std::cell::RefCell<Vec<(&'static str, f64, f64)>>>,
}

impl PhaseLog {
    fn new(start: std::time::Instant) -> Self {
        Self {
            start,
            last: std::rc::Rc::new(std::cell::RefCell::new(start)),
            rows: std::rc::Rc::new(std::cell::RefCell::new(Vec::new())),
        }
    }

    fn mark(&self, name: &'static str) {
        let now = std::time::Instant::now();
        let delta = {
            let mut last = self.last.borrow_mut();
            let d = now.duration_since(*last).as_secs_f64() * 1000.0;
            *last = now;
            d
        };
        let at = now.duration_since(self.start).as_secs_f64() * 1000.0;
        self.rows.borrow_mut().push((name, delta, at));
    }

    fn json(&self) -> String {
        let rows = self.rows.borrow();
        let parts: Vec<String> = rows
            .iter()
            .map(|(n, d, t)| format!("{{\"phase\":\"{n}\",\"ms\":{d:.1},\"at_ms\":{t:.1}}}"))
            .collect();
        format!("[{}]", parts.join(","))
    }
}

/// A2 · first-paint measurement (--measure-startup).
///
/// Slint 1.18 exposes `Window::set_rendering_notifier`; femtovg and skia
/// support it and fire `AfterRendering` once per drawn frame, so the first
/// firing is the first painted frame (its exact boundary: after the scene is
/// rendered and the GPU commands submitted, immediately before presentation —
/// see `i-slint-renderer-femtovg` draw(); a sub-millisecond underestimate of
/// true on-screen time). Measured caveat, 2026-09-20: skia only notifies on a
/// surface whose `with_graphics_api` is real, so `--features skia-opengl` does
/// fire and the default `--features skia` build (wgpu/softbuffer candidates)
/// stays silent while still returning `Ok(())` — never read a silent skia run
/// as "no frame was drawn". The software renderer has no notifier; there the
/// proxy is the first timer to run inside the event loop, which lands before
/// any frame is drawn (so it under-reports paint, over-reports readiness —
/// flagged `"confidence":"low"` and documented in PERFORMANCE.md).
///
/// The window is still hidden at install time, so no frame can be missed.
/// Normal runs never call this — no timer, no thread, no extra frame.
fn install_startup_measurement(
    ui: &AppWindow,
    start: std::time::Instant,
    phases: Option<PhaseLog>,
) {
    // The stamps after this point are still written by `real_main`, which keeps
    // running until `ui.run()` — PhaseLog is Rc inside, so the read here sees
    // every mark made before the first frame.
    let paint_json = |phases: &Option<PhaseLog>| {
        phases
            .as_ref()
            .map(|p| format!(",\"phases\":{}", p.json()))
            .unwrap_or_default()
    };
    let reported = std::rc::Rc::new(std::cell::Cell::new(false));
    let reported2 = reported.clone();
    let phases2 = phases.clone();
    let result = ui.window().set_rendering_notifier(move |s, _api| {
        if matches!(s, slint::RenderingState::AfterRendering) && !reported2.get() {
            reported2.set(true);
            eprintln!(
                "{{\"event\":\"first_paint\",\"renderer\":\"{}\",\"first_paint_ms\":{:.1},\"method\":\"AfterRendering\"{}}}",
                quire::app::controller::renderer_id(),
                start.elapsed().as_secs_f64() * 1000.0,
                paint_json(&phases2)
            );
        }
    });
    if result.is_err() {
        eprintln!("quire: [measure] set_rendering_notifier failed: {:?}", result.err());
        // No notifier on this backend: the 1 ms single-shot runs as the first
        // event-loop callback, before any drawn frame exists.
        let t = Timer::default();
        let phases3 = phases.clone();
        t.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(1),
            move || {
                if !reported.get() {
                    reported.set(true);
                    eprintln!(
                        "{{\"event\":\"first_paint\",\"renderer\":\"{}\",\"first_paint_ms\":{:.1},\"method\":\"event_loop_proxy\",\"confidence\":\"low\"{}}}",
                        quire::app::controller::renderer_id(),
                        start.elapsed().as_secs_f64() * 1000.0,
                        paint_json(&phases3)
                    );
                }
            },
        );
        leak_timer(t);
    }
}

fn real_main(start: std::time::Instant) -> Result<(), String> {
    // ── the single-instance claim (ADR-0136) ─────────────────────────────────
    // The very first thing a session does, before logging, the database or the
    // window. Order is the rule, not a preference: a launch that is going to be
    // told to go away must not open the database first, because that is a second
    // writer on a file a live instance already holds, and it must not draw a
    // window, because the user asked for one app and would get two.
    //
    // The quit this can send goes through the *old* instance's own channel
    // (ADR-0105), so the session being replaced runs its final flush and writes
    // its clean-exit record instead of being killed — which is the property that
    // lets a deploy overwrite a running install without the exe being locked.
    //
    // `on_quit` answers `false` while this process has no event loop yet: the
    // claim is made before the window exists, so a peer that asked to be replaced
    // in that window is told honestly that nothing accepted, rather than this
    // process posting a quit to a loop that has not started.
    let quitting = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    // `--quick-note` (ADR-0145) is a harness run rather than a session, and the
    // rule above is what a harness cannot pass: it would read the live instance's
    // pipe, and if that instance turned out to be an *older* build the claim would
    // ask it to quit — the developer's session, ended by a test. So the harness
    // leaves the channel alone entirely. It opens no shared library either (the
    // door is paired with `--db` on a throwaway file), which is the other half of
    // why skipping the claim is safe here and nowhere else.
    let harness = std::env::args().skip(1).any(|a| a == "--quick-note");
    let second = if harness {
        None
    } else {
        quire::platform::quit::claim(
        env!("CARGO_PKG_VERSION"),
        {
            let quitting = quitting.clone();
            move || {
                // The window this process owns is the one that has to answer, and
                // it is only reachable once its event loop is running.
                if quitting.load(std::sync::atomic::Ordering::SeqCst) {
                    slint::invoke_from_event_loop(|| {
                        let _ = slint::quit_event_loop();
                    })
                    .is_ok()
                } else {
                    false
                }
            }
        },
        |verdict| {
            // The user-facing half of the rule, said plainly: which build is
            // running, and why nothing else happened. A MessageBox rather than a
            // toast because there is no window yet to hang a toast off, and
            // because this is the one launch that ends in a dialog.
            let (title, body) = match verdict {
                quire::platform::quit::SecondLaunch::Primary => return,
                quire::platform::quit::SecondLaunch::AlreadyRunning { version } => (
                    "Quire 已在运行",
                    format!("Quire {version} 已经在运行，本次启动已退出。\n\n可在任务栏托盘图标上右键选择「显示主界面」。"),
                ),
                quire::platform::quit::SecondLaunch::RunningIsNewer { version } => (
                    "已有更新版本在运行",
                    format!(
                        "Quire {version} 正在运行，它比本次启动的版本更新，本次启动已退出。"
                    ),
                ),
                quire::platform::quit::SecondLaunch::Replacing {
                    quit_accepted,
                    replaced,
                } => {
                    if *quit_accepted {
                        // The old session is on its way out and this one takes its
                        // place — a window that vanishes and another that appears,
                        // with no dialog in between. Naming the version that left
                        // is what makes the swap legible instead of looking like
                        // the app restarted itself behind the user's back.
                        (
                            "已更新到新版本",
                            format!(
                                "Quire {replaced} 已在运行，本次启动已让旧版本退出并由新版本接替。"
                            ),
                        )
                    } else {
                        (
                            "旧版本未能退出",
                            "已请求正在运行的旧版本退出，但它没有响应。\n\n请从托盘图标右键菜单选择「退出」后重试。".to_string(),
                        )
                    }
                }
            };
            quire::platform::notify(title, &body);
        },
    )
    };
    if let Some(verdict) = second {
        // Every non-Primary verdict is a launch that does not become a session:
        // it never opens the database and never draws a window, so there is
        // nothing below to run.
        if !matches!(verdict, quire::platform::quit::SecondLaunch::Replacing { .. }) {
            return Ok(());
        }
    }
    // The argument parser reads only strings — no I/O to fail — so running it
    // before logging costs no coverage: `logging::init` installs the panic
    // hook as its first act, and needs the flags to know which directory the
    // log belongs in (M8_FEEDBACK #13).
    let launch = parse_launch_args();
    let phases = launch
        .measure_startup
        .then(|| PhaseLog::new(start));
    let mark = {
        let phases = phases.clone();
        move |name: &'static str| {
            if let Some(p) = &phases {
                p.mark(name);
            }
        }
    };
    mark("args_parsed");
    let location = quire::storage::data_location::LaunchOptions {
        db_override: launch.db.clone(),
        portable: launch.portable,
    };
    quire::services::logging::init(&location); // the rotating log + panic hook (SPEC §二十五, M8 D9)
    mark("logging_init");

    // Persistence (M3): open (or create) the database. A failure to open
    // means the session runs in memory only — never fall back to writing
    // over a database we could not read.
    let mut recovered: Option<std::path::PathBuf> = None;
    let mut moved_from: Option<std::path::PathBuf> = None;
    let repo: Option<std::sync::Arc<quire::storage::SqliteRepository>> = {
        // D12: this is the start's one resolve of the placement rules (M8
        // FEEDBACK #13) — it also carries a legacy `appdata/` library across.
        // What comes back is the file to open plus, once per install, the
        // folder it was moved out of, which the open reports back so the
        // notice bar can say it.
        let requested = launch
            .db
            .clone()
            .unwrap_or_else(|| std::path::PathBuf::from("appdata/quire.db"));
        let moved = quire::storage::data_location::migration(
            &location,
            &requested,
            quire::storage::data_location::roaming_root().as_deref(),
        );
        mark("data_location");
        match quire::storage::SqliteRepository::open_at(&moved.path, moved.from) {
            Ok((r, report)) => {
                if let Some(from) = &report.recovered_from {
                    recovered = Some(from.clone());
                }
                moved_from = report.migrated_from.clone();
                report.log();
                Some(std::sync::Arc::new(r))
            }
            Err(e) => {
                eprintln!("quire: database unavailable ({}); running in memory", e);
                None
            }
        }
    };
    mark("repo_open");
    let args = HandleArgs {
        blocks: launch.blocks,
        auto_exit_secs: launch.auto_exit_secs,
        bench_pages: launch.bench_pages,
        pictures: launch.pictures,
        marks: launch.marks,
        code: launch.code,
    };
    let ui = AppWindow::new().map_err(|e| e.to_string())?;
    mark("appwindow_new");
    if launch.measure_startup {
        install_startup_measurement(&ui, start, phases);
    }
    // Window::set_icon does not exist in Slint 1.18 (M8_FEEDBACK #4/#5 note):
    // the taskbar/explorer icon comes from the exe's embedded resource (D8),
    // and the frameless window shows no title bar — nothing user-visible is
    // missing. Revisit on Slint upgrade.
    let repo_for_lan = repo.clone();
    let state = AppState::new(&args, repo);
    mark("state_new");
    // startup notices (restore-from-backup, the D12 library move, and the
    // previous session's abort record — `db_notice` is a queue, #9)
    let mut notices: Vec<String> = Vec::new();
    if let Some(from) = &recovered {
        notices.push(format!(
            "the database was damaged — restored from a backup ({})",
            from.display()
        ));
    }
    if moved_from.is_some() {
        notices.push("the library moved to your user profile".into());
    }
    if !notices.is_empty() {
        state.set_db_notice(format!("{}.", notices.join("; ")));
    }
    // Restore the remembered window rectangle after the state is built (slint
    // still counts it as pre-first-paint).
    //
    // The rectangle is only *hints* until `monitors::place` has looked at the
    // displays: the monitor it was saved on may be gone, may be a different size
    // than it was, and a size that no longer fits anywhere is a window whose
    // bottom — the ＋ line, the window controls' row — is off the desk. Set the
    // size before the position, because `set_position` is what asks winit to
    // clamp against a window that already has its final size.
    if let Some(g) = state.window_geometry_setting() {
        let placed = platform::monitors::place(g, &platform::monitors::work_areas());
        ui.window().set_size(slint::PhysicalSize::new(
            placed.width() as u32,
            placed.height() as u32,
        ));
        ui.window().set_position(slint::PhysicalPosition::new(
            placed.left as i32,
            placed.top as i32,
        ));
    }
    controller::bind(&ui, &state);
    controller::wire(&ui, &state);
    mark("bind_wire");

    // The system tray (SPEC §二十七, ADR-0096): closing the window hides it, and
    // the session ends only through the tray menu's 「退出」.
    //
    // Windows carries the *native* tray (`platform::tray`): the icon's left
    // click shows the main window, and the right-click menu is drawn dark when
    // the app's theme is — two things Slint's own tray cannot do here (it
    // reports no click, and the shell paints its menu light). The native icon
    // is not a Slint element and counts for no keepalive, which is why the
    // Windows loop below is `run_event_loop_until_quit` and not `ui.run()`.
    // The Slint tray stays for every platform the native one does not cover,
    // and as the fallback when the native icon is refused (a headless run):
    // there its visibility is what holds the keepalive, so `_tray` must stay
    // bound for the whole run — dropping it removes the icon and can end the
    // session.
    #[cfg(windows)]
    let native_tray = {
        let on_show = show_window_closure(&ui);
        let on_quit: Box<dyn Fn() + Send> = Box::new(|| {
            let _ = slint::invoke_from_event_loop(|| {
                let _ = slint::quit_event_loop();
            });
        });
        match quire::platform::tray::install(on_show, on_quit) {
            Ok(tray) => Some(tray),
            Err(e) => {
                eprintln!("quire: no native tray ({e})");
                None
            }
        }
    };
    #[cfg(windows)]
    let _slint_tray_fallback = if native_tray.is_none() {
        match quire::app::tray::install(&ui) {
            Ok(tray) => Some(tray),
            Err(e) => {
                eprintln!("quire: no system tray ({e}); closing the window will not be recoverable");
                None
            }
        }
    } else {
        None
    };
    #[cfg(not(windows))]
    let _tray = match quire::app::tray::install(&ui) {
        Ok(tray) => Some(tray),
        Err(e) => {
            eprintln!("quire: no system tray ({e}); closing the window will not be recoverable");
            None
        }
    };

    // A `--quick-note` run's unmet rules, collected by its own stages and read
    // once the loop ends — so the verdict is also the exit code a harness checks
    // rather than a word it has to grep for. Empty for every other run.
    let quick_note_failures: std::rc::Rc<std::cell::RefCell<Vec<String>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));

    // The 全局快捷键 (settings rows): Alt+N opens the 速记 window — the same card
    // the notes page's ＋ opens, 标签建议 and all, in a small always-on-top window
    // that does *not* bring the main interface out (ADR-0145) — and Alt+M shows
    // the window. Alt+N registers unless its row says otherwise, Alt+M only when
    // its row says so, and the dialog's toggles flip the keys live through the
    // handle below.
    #[cfg(windows)]
    {
        // 速记 is the one thing this window cannot do for itself: a top-level
        // `Window` is shown from Rust, so the handle lives here (the state keeps
        // it alive) and the hotkey thread reaches it as a `Weak` it can send.
        let quick_note =
            quire::QuickNoteWindow::new().map_err(|e| e.to_string())?;
        // Its ✕ and its Esc are the card's own 「放弃」; this is the door the
        // platform closes instead (Alt+F4, a taskbar's 关闭窗口). The draft goes
        // with it — a composer nobody can see is a composer that lied about
        // holding the user's words — and the window hides rather than dies, so
        // the next Alt+N brings back the same card with its position kept.
        {
            let ui_w = ui.as_weak();
            quick_note
                .window()
                .on_close_requested(move || {
                    if let Some(ui) = ui_w.upgrade() {
                        ui.global::<quire::UIState>().invoke_org_capture_closed();
                    }
                    slint::CloseRequestResponse::HideWindow
                });
        }
        // The window outlives this block because the state holds it — the handle
        // *is* its strong reference, and `real_main` drops its own binding the
        // moment the hotkeys are wired. A 速记 window that no longer exists is a
        // hotkey with nowhere to put the note, so it goes in last, once the
        // closure below has taken the `Weak` it can send to the hotkey thread.
        //
        // The press is an `Arc` rather than the boxed closure the hotkey alone
        // would need, because a second door calls the *same* press: `--quick-note`
        // (ADR-0145). A harness that ran its own copy of these lines would be
        // testing a path no user is ever on.
        let press: std::sync::Arc<dyn Fn() + Send + Sync> = {
            let ui_w = ui.as_weak();
            let qn_w = quick_note.as_weak();
            // Centred on the cursor once per session: the first press has no
            // place of its own yet, and every later one has exactly the place the
            // user dragged the card to. `Weak` and atomics because the hotkey
            // thread owns this closure — `Rc` would not cross to it.
            let placed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            std::sync::Arc::new(move || {
                let ui_w = ui_w.clone();
                let qn_w = qn_w.clone();
                let placed = placed.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    // A hotkey owes the user somewhere to type, and nowhere else:
                    // the main window stays hidden in the tray, and the card
                    // arrives over whatever the user was working in.
                    let (Some(ui), Some(quick)) = (ui_w.upgrade(), qn_w.upgrade()) else {
                        return;
                    };
                    if let Err(e) = quick.window().show() {
                        logging::warn(&format!("速记 window: {e}"));
                    }
                    // Every press, not only the first: putting the window down can
                    // leave it a new HWND on the next one, and a 速记 that came back
                    // as an application window is a taskbar button the user never
                    // asked for.
                    quick_note_keep_off_taskbar(&quick);
                    if !placed.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        let size = quick.window().size();
                        if let Some(rect) = platform::monitors::centered_on_cursor((
                            size.width as f64,
                            size.height as f64,
                        )) {
                            quick.window().set_position(slint::PhysicalPosition::new(
                                rect.left as i32,
                                rect.top as i32,
                            ));
                        }
                    }
                    // the tick, not a direct call: a counter always changes,
                    // so every press fires the changed hook exactly once
                    let g = ui.global::<quire::UIState>();
                    g.set_org_quick_note_tick(g.get_org_quick_note_tick() + 1);
                    // …and the caret goes in on the frame the window is in front of
                    // the user. The card's own `init` ran once, when this window was
                    // built and invisible; a press that re-shows it is a second
                    // arrival the `init` cannot see.
                    quick.invoke_raise_input();
                });
            })
        };
        let on_alt_n: Box<dyn Fn() + Send> = {
            let press = press.clone();
            Box::new(move || press())
        };
        let qn_w = quick_note.as_weak();
        state.install_quick_note(quick_note);

        // `--quick-note`: the verification door, and the only reason a run
        // reaches it. Every stage calls the *real* door — the hotkey's own closure,
        // the card's ✕ callback, the ＋'s callback — and prints what the platform
        // then reports about the two windows. So the whole contract of ADR-0145 is
        // a handful of lines a harness reads plus one verdict line, rather than a
        // screenshot someone squints at.
        //
        // The rules are checked here rather than by a script reading the log, so
        // that the window a stage reports on is the window the action settled
        // into and not a frame the reader has to guess at.
        if launch.quick_note {
            // Stage 0's setup is the state a global hotkey is actually pressed
            // from: the main window closed to the tray.
            let _ = ui.window().hide();
            let t = Timer::default();
            let press = press.clone();
            let ui_w = ui.as_weak();
            let stages = std::rc::Rc::new(std::cell::Cell::new(0u32));
            let failures = quick_note_failures.clone();
            t.start(
                slint::TimerMode::Repeated,
                std::time::Duration::from_millis(900),
                move || {
                    let (Some(ui), Some(quick)) = (ui_w.upgrade(), qn_w.upgrade()) else {
                        eprintln!("quick-note: a window is gone");
                        return;
                    };
                    let g = ui.global::<quire::UIState>();
                    // a tick per action and a tick per report, so each report
                    // reads the frame the action settled into
                    let stage = stages.get();
                    stages.set(stage + 1);
                    match stage {
                        0 | 6 => press(),
                        2 => g.invoke_org_capture_closed(),
                        4 => g.invoke_org_capture_opened(),
                        s @ (1 | 3 | 5 | 7) => {
                            // What each door leaves behind, in the platform's own
                            // words: the 速记 window holds the draft for Alt+N and
                            // the ＋ hands it back to the notes page's overlay.
                            let (door, wants_note, wants_capture) = match s {
                                1 => ("press", true, true),
                                3 => ("dismiss", false, false),
                                5 => ("plus", false, true),
                                _ => ("press-again", true, true),
                            };
                            let ex = platform::window_ex_style(quick.window());
                            let note_visible = quick.window().is_visible();
                            let toolwindow = ex
                                .map(|bits| bits & platform::WS_EX_TOOLWINDOW != 0)
                                .unwrap_or(false);
                            let host = g.get_quick_note_host();
                            let capture = g.get_org_capture_open();
                            eprintln!(
                                "quick-note {door}: main-visible={} note-visible={} capture-open={} host={} area={} tick={} note-ex={} toolwindow={}",
                                ui.window().is_visible(),
                                note_visible,
                                capture,
                                host,
                                g.get_active_area(),
                                g.get_org_quick_note_tick(),
                                ex.map(|bits| format!("0x{bits:08X}"))
                                    .unwrap_or_else(|| "-".into()),
                                toolwindow,
                            );
                            // One draft, one surface: `host` is the flag that
                            // says which, and the rule the controller keeps is
                            // that the small window hosts the draft *exactly*
                            // while it is on screen — the two are written
                            // together on every path, so a mismatch is one of
                            // them having been set without the other.
                            for (name, ok) in [
                                ("主界面没有跟着出来", !ui.window().is_visible()),
                                ("小窗的显与隐对得上", note_visible == wants_note),
                                ("草稿的有无对得上", capture == wants_capture),
                                ("草稿只有一个宿主", host == note_visible),
                                ("小窗可见时不在任务栏", !note_visible || toolwindow),
                            ] {
                                if !ok {
                                    failures.borrow_mut().push(format!("{door}: {name}"));
                                }
                            }
                        }
                        8 => {
                            // The verdict, and then the session ends on its own:
                            // a harness that has to kill the process would leave
                            // the app's own log saying it crashed.
                            let bad = failures.borrow();
                            eprintln!(
                                "quick-note verdict: {}",
                                if bad.is_empty() {
                                    "pass".to_string()
                                } else {
                                    bad.join(", ")
                                }
                            );
                            let _ = slint::quit_event_loop();
                        }
                        _ => {}
                    }
                },
            );
            leak_timer(t);
        }
        let on_alt_m: Box<dyn Fn() + Send> = show_window_closure(&ui);
        match quire::platform::hotkeys::install(on_alt_n, on_alt_m) {
            Ok(hotkeys) => {
                let alt_n_on = state.setting_flag_or("hotkeys.alt-n", true);
                let alt_m_on = state.setting_flag_or("hotkeys.alt-m", false);
                if let Err(e) = hotkeys.set_enabled(quire::platform::hotkeys::ALT_N, alt_n_on) {
                    eprintln!("quire: Alt+N unavailable: {e}");
                }
                if let Err(e) = hotkeys.set_enabled(quire::platform::hotkeys::ALT_M, alt_m_on) {
                    eprintln!("quire: Alt+M unavailable: {e}");
                }
                state.install_hotkeys(hotkeys);
            }
            Err(e) => eprintln!("quire: global hotkeys unavailable ({e})"),
        }
    }

    // A close request is a hide, never a quit. This is what the title bar's X
    // reaches through `root.close()`, and it is also the answer the platform's
    // own close would get — spelled out so the two cannot drift apart.
    ui.window()
        .on_close_requested(|| slint::CloseRequestResponse::HideWindow);

    // .md file association: double-clicking a markdown file lands here
    if let Some(path) = launch.open.clone() {
        let g = ui.global::<quire::UIState>();
        controller::import_from_path(&g, &state, &path);
    }
    mark("import_open");

    // LAN share: serve the committed workspace read-only on its own thread.
    // Enabled via --share or the persisted settings toggle (lan.share).
    let share_port = launch.share.or_else(|| {
        if state.setting_flag("lan.share") {
            Some(quire::services::lan_server::DEFAULT_PORT)
        } else {
            None
        }
    });
    if let Some(port) = share_port {
        match &repo_for_lan {
            Some(repo) => {
                let server =
                    quire::services::lan_server::LanServer::new(repo.clone(), port);
                std::thread::spawn(move || match server.bind() {
                    Ok(listener) => server.serve(listener),
                    Err(e) => eprintln!("quire: lan bind failed: {e}"),
                });
            }
            None => eprintln!("quire: --share needs a database; sharing disabled"),
        }
    }

    // LAN pull: import every page from a peer's share endpoint
    if let Some(url) = &launch.pull {
        match quire::services::lan_client::pull_workspace(url) {
            Ok(pages) => {
                let g = ui.global::<quire::UIState>();
                let count = controller::import_lan_pages(&g, &state, url, pages);
                eprintln!("quire: imported {count} pages from {url}");
            }
            Err(e) => eprintln!("quire: pull failed: {e}"),
        }
    }
    mark("lan_setup");

    if launch.dump_state {
        let pages = state.workspace.borrow().page_count();
        let (blocks, marked, coloured) = {
            let d = state.doc.borrow();
            let mut n = 0;
            let mut marked = 0;
            let mut coloured = 0;
            for pid in [102, 105] {
                let rows = d.page_blocks(quire::core::PageId(pid));
                n += rows.len();
                marked += rows.iter().filter(|b| !b.marks.is_empty()).count();
                coloured += rows.iter().filter(|b| b.lang != quire::core::Lang::Plain).count();
            }
            (n, marked, coloured)
        };
        // `coloured` is the highlight arm's identity: a binary without the
        // language field has no such count to print.
        eprintln!("dump-state: pages={pages} gs+atlas-blocks={blocks} marked={marked} coloured={coloured}");
    }

    if args.auto_exit_secs > 0.0 {
        let t = Timer::default();
        // The bench harness reads the decode cache's high-water mark off
        // stderr here; a normal run has no reason to ask for it.
        let state_w = std::rc::Rc::downgrade(&state);
        let report_cache = launch.dump_state;
        t.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_secs_f64(args.auto_exit_secs),
            move || {
                if report_cache {
                    if let Some(state) = state_w.upgrade() {
                        eprint!("{}", state.attachment_cache_report());
                    }
                }
                let _ = slint::quit_event_loop();
            },
        );
        leak_timer(t);
    }

    if launch.scroll {
        // Scene F: programmatic continuous scroll. The editor's viewport-y is
        // two-way bound to UIState.editor-scroll-y, so advancing the property
        // drives the same repaint path a mouse wheel would — and Slint's
        // viewport-y is *negative* the way down, so the step subtracts.
        //
        // A ListView only knows the height of the rows it has realized, so the
        // clamp grows a frame behind the scroll and the position stalls for a
        // tick or two without the bottom being near. One stalled tick used to
        // mean "wrap to top"; now a wrap needs the position to sit still for a
        // whole second of ticks.
        let ui_w = ui.as_weak();
        let state_w = std::rc::Rc::downgrade(&state);
        let step = launch.scroll_step;
        let stalls = std::rc::Rc::new(std::cell::Cell::new(0u32));
        let t = Timer::default();
        t.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(16),
            move || {
                let (Some(ui), Some(state)) = (ui_w.upgrade(), state_w.upgrade()) else {
                    return;
                };
                let g = ui.global::<quire::UIState>();
                let cur = g.get_editor_scroll_y();
                // A ListView that has not measured its rows yet reports a
                // shorter content height, so the clamp moves under the scroll
                // and the read-back differs from what was set for a tick. Only
                // a position that was actually *put* there counts as stuck.
                let stuck = cur == state.last_scroll_y();
                state.set_last_scroll_y(cur);
                if stuck && cur < 0.0 {
                    stalls.set(stalls.get() + 1);
                    if stalls.get() < 60 {
                        return; // the ListView is still catching up
                    }
                    // bottom reached and held: wrap to top
                    stalls.set(0);
                    g.set_editor_scroll_y(0.0);
                } else {
                    stalls.set(0);
                    g.set_editor_scroll_y(cur - step);
                }
            },
        );
        leak_timer(t);
    }

    if launch.bench_pages > 0 {
        // Scene G: cycle through the bench pages, exercising model swap +
        // delegate rebuild at a fixed rate.
        let ids: Vec<i32> = state
            .workspace
            .borrow()
            .dfs_order()
            .into_iter()
            .filter(|id| *id >= quire::app::workspace::BENCH_ID_BASE)
            .collect();
        let ui_w = ui.as_weak();
        let state_w = std::rc::Rc::downgrade(&state);
        let cursor = std::cell::Cell::new(0usize);
        let t = Timer::default();
        t.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(120),
            move || {
                let (Some(ui), Some(state)) = (ui_w.upgrade(), state_w.upgrade()) else {
                    return;
                };
                let mut c = cursor.get();
                controller::bench_switch_next(&ui, &state, &ids, &mut c);
                cursor.set(c);
            },
        );
        leak_timer(t);
    }

    if let Some(scene) = launch.scene {
        let ui_w = ui.as_weak();
        let state_w = std::rc::Rc::downgrade(&state);
        let name = scene.clone();
        let t = Timer::default();
        t.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(600),
            move || {
                if let (Some(ui), Some(state)) = (ui_w.upgrade(), state_w.upgrade()) {
                    controller::apply_scene(&ui, &state, &name);
                }
            },
        );
        leak_timer(t);
    }

    mark("pre_event_loop");
    // From here the event loop exists, so the channel's quit is deliverable: until
    // this flag is set, `on_quit` above answers `false` rather than posting a
    // quit to a loop that has not started — which is what makes a peer that asks
    // to be replaced during startup hear `err` rather than a silent success.
    quitting.store(true, std::sync::atomic::Ordering::SeqCst);
    // Windows runs the loop **to the explicit quit**: the native tray icon is
    // not a Slint element and counts for no keepalive, so `ui.run()` — which
    // ends the session when the last visible thing goes — would end it the
    // moment the user closed to the tray. `run_event_loop_until_quit` outlives
    // every window; the exits are the tray menu's 「退出」 and `--quit` (ADR-0096).
    // Everywhere else `ui.run()` keeps the shape it had, keepalive tray and all.
    #[cfg(windows)]
    slint::run_event_loop_until_quit().map_err(|e| e.to_string())?;
    #[cfg(not(windows))]
    ui.run().map_err(|e| e.to_string())?;
    // the icon goes with the session: NIM_DELETE before the process ends, so
    // the notification area never carries this run's ghost
    #[cfg(windows)]
    if let Some(tray) = native_tray {
        tray.remove();
    }
    // Remember the window rectangle, then flush dirty state on close (SPEC §十九).
    // A maximized window reports the *monitor's* size, so the rectangle to record
    // is the one snapshotted when it went maximized — otherwise a session that
    // ended maximized saved the monitor's dimensions as the next launch's window,
    // which is how a maximized window on a secondary display came back sized for
    // a display that might not be plugged in any more.
    let size = ui.window().size();
    let position = ui.window().position();
    let (x, y) = (position.x as f64, position.y as f64);
    let live = WindowGeometry {
        left: x,
        top: y,
        right: x + size.width as f64,
        bottom: y + size.height as f64,
    };
    state.record_window_geometry(state.restore_geometry().unwrap_or(live));
    state.persistence_force_flush();
    // The clean-exit record (M8_FEEDBACK #10, ADR-0018): the next start reads
    // its absence — with no panic report — as "killed, crashed natively, or
    // lost power". Normal exit path only, after the final flush, so a session
    // that dies any other way still counts as unclean.
    logging::note(logging::END_RECORD);
    // The harness's verdict *is* its exit code, and it comes after the clean-exit
    // record: a run that checked the rules and then died on its own way out would
    // leave the app's log saying it crashed, and the next session's warning would
    // be about the wrong thing.
    if !quick_note_failures.borrow().is_empty() {
        return Err(format!(
            "quick-note: {} rule(s) unmet",
            quick_note_failures.borrow().len()
        ));
    }
    Ok(())
}


