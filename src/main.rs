// Quire — a local, GPU-accelerated, Notion-like document workspace.
// Single process: Slint UI on the main thread, Rust core behind it.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
// Filled by later milestones; kept declared so the skeleton is real code,
// not wishes in a directory listing (docs/ARCHITECTURE.md).
mod core;
mod platform;
mod services;
mod storage;

slint::include_modules!();

use app::controller;
use app::state::{AppState, HandleArgs};
use slint::{ComponentHandle, Timer};

pub struct LaunchArgs {
    pub blocks: usize,
    pub auto_exit_secs: f64,
}

fn parse_launch_args() -> LaunchArgs {
    let argv: Vec<String> = std::env::args().collect();
    let mut a = LaunchArgs {
        blocks: 0,
        auto_exit_secs: 0.0,
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
            _ => {}
        }
        i += 1;
    }
    a
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let launch = parse_launch_args();
    let args = HandleArgs {
        blocks: launch.blocks,
        auto_exit_secs: launch.auto_exit_secs,
    };
    let ui = AppWindow::new()?;
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
        // Dropping a Timer cancels it; this one must outlive the binding scope.
        std::mem::forget(t);
    }

    ui.run()?;
    Ok(())
}
