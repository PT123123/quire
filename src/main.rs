// Quire — a local, GPU-accelerated, Notion-like document workspace.
// Single process: Slint UI on the main thread, Rust core behind it.
// All logic lives in the library crate (src/lib.rs); this binary only
// parses launch args, wires the window, and runs the event loop.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use quire::app::controller;
use quire::app::state::{AppState, HandleArgs};
use quire::services::logging;
use quire::AppWindow;
use slint::{ComponentHandle, Timer};

pub struct LaunchArgs {
    pub blocks: usize,
    pub auto_exit_secs: f64,
    /// Scene G: extra flat pages to switch between (--page-switch N).
    pub bench_pages: usize,
    /// Scene F: programmatic continuous scroll (--scroll).
    pub scroll: bool,
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
        scroll: false,
        scene: None,
        db: None,
        portable: false,
        dump_state: false,
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
            ("--scroll", _) => {
                a.scroll = true;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
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

/// The renderer name baked in at compile time (no runtime cost, no ambiguity:
/// each feature-gated build reports its own renderer).
fn renderer_name() -> &'static str {
    #[cfg(feature = "femtovg")]
    return "femtovg";
    #[cfg(all(not(feature = "femtovg"), feature = "femtovg-wgpu"))]
    return "femtovg-wgpu";
    #[cfg(all(
        not(feature = "femtovg"),
        not(feature = "femtovg-wgpu"),
        any(feature = "skia", feature = "skia-opengl")
    ))]
    return "skia";
    #[cfg(all(
        not(feature = "femtovg"),
        not(feature = "femtovg-wgpu"),
        not(any(feature = "skia", feature = "skia-opengl")),
        feature = "software"
    ))]
    return "software";
    #[cfg(all(
        not(feature = "femtovg"),
        not(feature = "femtovg-wgpu"),
        not(any(feature = "skia", feature = "skia-opengl")),
        not(feature = "software")
    ))]
    return "unknown";
}

/// A2 · first-paint measurement (--measure-startup).
///
/// Slint 1.18 exposes `Window::set_rendering_notifier`; femtovg and skia
/// support it and fire `AfterRendering` once per drawn frame, so the first
/// firing is the first painted frame (its exact boundary: after the scene is
/// rendered and the GPU commands submitted, immediately before presentation —
/// see `i-slint-renderer-femtovg` draw(); a sub-millisecond underestimate of
/// true on-screen time). The software renderer has no notifier; there the
/// proxy is the first timer to run inside the event loop, which lands before
/// any frame is drawn (so it under-reports paint, over-reports readiness —
/// flagged `"confidence":"low"` and documented in PERFORMANCE.md).
///
/// The window is still hidden at install time, so no frame can be missed.
/// Normal runs never call this — no timer, no thread, no extra frame.
fn install_startup_measurement(ui: &AppWindow, start: std::time::Instant) {
    let reported = std::rc::Rc::new(std::cell::Cell::new(false));
    let reported2 = reported.clone();
    let result = ui.window().set_rendering_notifier(move |s, _api| {
        if matches!(s, slint::RenderingState::AfterRendering) && !reported2.get() {
            reported2.set(true);
            eprintln!(
                "{{\"event\":\"first_paint\",\"renderer\":\"{}\",\"first_paint_ms\":{:.1},\"method\":\"AfterRendering\"}}",
                renderer_name(),
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    });
    if result.is_err() {
        eprintln!("quire: [measure] set_rendering_notifier failed: {:?}", result.err());
        // No notifier on this backend: the 1 ms single-shot runs as the first
        // event-loop callback, before any drawn frame exists.
        let t = Timer::default();
        t.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(1),
            move || {
                if !reported.get() {
                    reported.set(true);
                    eprintln!(
                        "{{\"event\":\"first_paint\",\"renderer\":\"{}\",\"first_paint_ms\":{:.1},\"method\":\"event_loop_proxy\",\"confidence\":\"low\"}}",
                        renderer_name(),
                        start.elapsed().as_secs_f64() * 1000.0
                    );
                }
            },
        );
        leak_timer(t);
    }
}

fn real_main(start: std::time::Instant) -> Result<(), String> {
    // The argument parser reads only strings — no I/O to fail — so running it
    // before logging costs no coverage: `logging::init` installs the panic
    // hook as its first act, and needs the flags to know which directory the
    // log belongs in (M8_FEEDBACK #13).
    let launch = parse_launch_args();
    let location = quire::storage::data_location::LaunchOptions {
        db_override: launch.db.clone(),
        portable: launch.portable,
    };
    quire::services::logging::init(&location); // the rotating log + panic hook (SPEC §二十五, M8 D9)

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
    let args = HandleArgs {
        blocks: launch.blocks,
        auto_exit_secs: launch.auto_exit_secs,
        bench_pages: launch.bench_pages,
    };
    let ui = AppWindow::new().map_err(|e| e.to_string())?;
    if launch.measure_startup {
        install_startup_measurement(&ui, start);
    }
    // Window::set_icon does not exist in Slint 1.18 (M8_FEEDBACK #4/#5 note):
    // the taskbar/explorer icon comes from the exe's embedded resource (D8),
    // and the frameless window shows no title bar — nothing user-visible is
    // missing. Revisit on Slint upgrade.
    let repo_for_lan = repo.clone();
    let state = AppState::new(&args, repo);
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
    // restore the remembered window size after the state is built (slint
    // still counts it as pre-first-paint)
    if let Some((w, h)) = state.window_size_setting() {
        ui.window()
            .set_size(slint::PhysicalSize::new(w as u32, h as u32));
    }
    controller::bind(&ui, &state);
    controller::wire(&ui, &state);

    // .md file association: double-clicking a markdown file lands here
    if let Some(path) = launch.open.clone() {
        let g = ui.global::<quire::UIState>();
        controller::import_from_path(&g, &state, &path);
    }

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

    if launch.dump_state {
        let pages = state.workspace.borrow().page_count();
        let blocks = {
            let d = state.doc.borrow();
            let mut n = 0;
            for pid in [102, 105] {
                n += d.page_blocks(quire::core::PageId(pid)).len();
            }
            n
        };
        eprintln!("dump-state: pages={pages} gs+atlas-blocks={blocks}");
    }

    if args.auto_exit_secs > 0.0 {
        let t = Timer::default();
        t.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_secs_f64(args.auto_exit_secs),
            || {
                let _ = slint::quit_event_loop();
            },
        );
        leak_timer(t);
    }

    if launch.scroll {
        // Scene F: programmatic continuous scroll. The editor's viewport-y
        // is two-way bound to UIState.editor-scroll-y, so advancing the
        // property drives the same repaint path a mouse wheel would. The
        // wrap detection (position stopped moving = bottom reached) keeps
        // the scroll going for the whole sampling window.
        let ui_w = ui.as_weak();
        let state_w = std::rc::Rc::downgrade(&state);
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
                if cur == state.last_scroll_y() && cur > 0.0 {
                    // bottom reached: wrap to top
                    state.set_last_scroll_y(0.0);
                    g.set_editor_scroll_y(0.0);
                } else {
                    let next = cur + 8.0;
                    state.set_last_scroll_y(cur);
                    g.set_editor_scroll_y(next);
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

    ui.run().map_err(|e| e.to_string())?;
    // remember the window size, then flush dirty state on close (SPEC §十九)
    let size = ui.window().size();
    state.record_window_size(size.width as f64, size.height as f64);
    state.persistence_force_flush();
    // The clean-exit record (M8_FEEDBACK #10, ADR-0018): the next start reads
    // its absence — with no panic report — as "killed, crashed natively, or
    // lost power". Normal exit path only, after the final flush, so a session
    // that dies any other way still counts as unclean.
    logging::note(logging::END_RECORD);
    Ok(())
}


