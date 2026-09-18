// Quire — a local, GPU-accelerated, Notion-like document workspace.
// Single process: Slint UI on the main thread, Rust core behind it.
// All logic lives in the library crate (src/lib.rs); this binary only
// parses launch args, wires the window, and runs the event loop.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use quire::app::controller;
use quire::app::state::{AppState, HandleArgs};
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
}

fn parse_launch_args() -> LaunchArgs {
    let argv: Vec<String> = std::env::args().collect();
    let mut a = LaunchArgs {
        blocks: 0,
        auto_exit_secs: 0.0,
        bench_pages: 0,
        scroll: false,
        scene: None,
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
    // The UI event loop runs on its own thread with a generous stack:
    // Slint 1.18 evaluates the initial property/layout bindings of the
    // component tree recursively on the C stack, and Quire's shell sits
    // just above the 1 MB Windows default in debug builds (popups add
    // more when they open). 8 MB is the standard Linux default and costs
    // nothing but address-space reservation. See DECISIONS.md ADR-0009.
    let child = std::thread::Builder::new().stack_size(8 * 1024 * 1024)
        .spawn(|| real_main()).expect("spawn UI thread");
    child.join().map_err(|_| "UI thread panicked".to_string())??;
    Ok(())
}

fn real_main() -> Result<(), String> {
    let launch = parse_launch_args();
    let args = HandleArgs {
        blocks: launch.blocks,
        auto_exit_secs: launch.auto_exit_secs,
        bench_pages: launch.bench_pages,
    };
    let ui = AppWindow::new().map_err(|e| e.to_string())?;
    let state = AppState::new(&args);
    controller::bind(&ui, &state);
    controller::wire(&ui, &state);

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
    Ok(())
}


