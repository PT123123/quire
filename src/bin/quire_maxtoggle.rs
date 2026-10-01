// quire-maxtoggle — TEMPORARY freeze-repro harness (delete after diagnosis).
//
// Drives the real app window through the same property write the maximize
// button performs (`root.maximized = !root.maximized`): set_maximized on the
// window item + update_window_properties, which lands in the winit adapter's
// set_maximized → synchronous ShowWindow inside the event loop. One stderr
// line per toggle; a per-toggle time that explodes (or lines that stop
// coming) is the freeze showing itself.
//
// Build: cargo build --bin quire-maxtoggle
// Usage: quire-maxtoggle --db <path> [--toggles 60] [--interval-ms 250]

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use quire::app::controller;
use quire::app::state::{AppState, HandleArgs};
use quire::storage::SqliteRepository;
use quire::AppWindow;
use slint::{ComponentHandle, Timer, TimerMode};

struct Args {
    db: std::path::PathBuf,
    toggles: usize,
    interval: Duration,
    blocks: usize,
}

fn parse() -> Args {
    let argv: Vec<String> = std::env::args().collect();
    let get = |key: &str| {
        argv.iter()
            .position(|a| a == key)
            .and_then(|i| argv.get(i + 1))
            .cloned()
    };
    let num = |key: &str, default: u64| {
        get(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    Args {
        db: get("--db")
            .map(std::path::PathBuf::from)
            .expect("--db <path> is required"),
        toggles: num("--toggles", 60) as usize,
        interval: Duration::from_millis(num("--interval-ms", 250)),
        blocks: num("--blocks", 0) as usize,
    }
}

fn main() {
    // 8 MB stack: same rationale as the GUI binary (ADR-0009).
    let child = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(run)
        .expect("spawn UI thread");
    match child.join() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            eprintln!("quire-maxtoggle: {e}");
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("quire-maxtoggle: UI thread panicked");
            std::process::exit(2);
        }
    }
}

fn run() -> Result<(), String> {
    let args = parse();
    let repo =
        Arc::new(SqliteRepository::open(&args.db).map_err(|e| format!("open {:?}: {e}", args.db))?);
    let ui = AppWindow::new().map_err(|e| e.to_string())?;
    let handle = HandleArgs {
        blocks: args.blocks,
        auto_exit_secs: 0.0,
        bench_pages: 0,
        pictures: 0,
        marks: 0,
        code: 0,
    };
    let state = AppState::new(&handle, Some(repo.clone()));
    controller::bind(&ui, &state);
    controller::wire(&ui, &state);
    ui.show().map_err(|e| e.to_string())?;

    let w = ui.window();
    let size = w.size();
    eprintln!(
        "start: zoom={} base_scale={} window_scale={:.3} physical=({}x{}) maximized={}",
        state.zoom(),
        state.base_scale(),
        w.scale_factor(),
        size.width,
        size.height,
        w.is_maximized(),
    );

    let ui_w = ui.as_weak();
    let done = Rc::new(Cell::new(0usize));
    let done_tick = done.clone();
    let toggles = args.toggles;
    let interval = args.interval;

    // settle for a second (first paint, geometry restore), then toggle.
    // toggles = 0 means: no timer at all — a plain running app for the
    // external click driver to poke.
    if toggles > 0 {
        let starter: &'static Timer = Box::leak(Box::new(Timer::default()));
    starter.start(TimerMode::SingleShot, Duration::from_millis(1000), move || {
        // clones: the inner closure is built on every firing, and it must not
        // consume the outer closure's captures (the timer wants an FnMut)
        let ui_w = ui_w.clone();
        let done_tick = done_tick.clone();
        let t: &'static Timer = Box::leak(Box::new(Timer::default()));
        t.start(TimerMode::Repeated, interval, move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let i = done_tick.get() + 1;
            done_tick.set(i);
            let started = Instant::now();
            let to = !ui.window().is_maximized();
            eprintln!("toggle {i}/{toggles} -> maximized={to}");
            ui.window().set_maximized(to);
            let took = started.elapsed();
            let scale = ui.window().scale_factor();
            let size = ui.window().size();
            eprintln!(
                "toggle {i} done in {took:?} scale={scale:.3} logical=({:.0},{:.0})",
                size.width as f32 / scale,
                size.height as f32 / scale,
            );
            if i >= toggles {
                eprintln!("all {toggles} toggles done, quitting");
                let _ = slint::quit_event_loop();
            }
        });
    });
    }

    ui.run().map_err(|e| e.to_string())?;
    eprintln!("event loop ended cleanly");
    Ok(())
}
