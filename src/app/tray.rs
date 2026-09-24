// The system tray (SPEC §二十七 「窗口」, ADR-0096). Everything about *being* the
// tray is here: the icon and its menu live in `ui/TrayIcon.slint`; this module
// builds that component and answers its two menu actions.
//
// The shape of the feature is "closing the window is not quitting". Slint keeps
// the event loop alive while anything visible is left, and it counts visible
// windows and visible tray icons in the same counter (`SlintContext`'s
// keepalive). So the window hiding decrements that counter and the tray's own
// presence holds it above zero: `AppWindow::run()` outlives its window, and the
// only thing that ends the session is this menu's 「退出」.
//
// That is why `install` *returns* the component instead of stashing it: the
// caller has to keep it alive for as long as the app should stay in the tray.
// Dropping it removes the icon and releases the last keepalive, which would end
// the session — the correct teardown, but only after `run()` has returned.

use crate::{AppTray, AppWindow};
use slint::ComponentHandle;

/// Create the tray icon and wire its menu to `ui`.
///
/// Errors are the platform's, not the app's: a machine with no notification
/// area (a headless benchmark run, a session with no shell) can refuse the
/// icon. The caller decides what that costs — here the window simply stops
/// having somewhere to hide to.
pub fn install(ui: &AppWindow) -> Result<AppTray, slint::PlatformError> {
    let tray = AppTray::new()?;

    // 「显示主界面」, and the icon's own left-click on platforms that report one.
    // Clearing `minimized` first is what makes this a *restore* for a window the
    // user minimized to the taskbar rather than closed to the tray: `show()`
    // alone would bring it back still minimized.
    let ui_weak = ui.as_weak();
    tray.on_show_window(move || {
        if let Some(ui) = ui_weak.upgrade() {
            ui.window().set_minimized(false);
            let _ = ui.show();
        }
    });

    // 「退出」 — the only exit. Everything `real_main` does after `ui.run()`
    // (remember the window size, flush the queue, write the clean-exit record)
    // hangs off that call returning, so this must stay the quit path and not
    // `std::process::exit`.
    tray.on_quit(move || {
        let _ = slint::quit_event_loop();
    });

    Ok(tray)
}
