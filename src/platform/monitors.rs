// The monitors, as rectangles — the only way to answer "will the window the
// user left behind still be somewhere they can reach it?".
//
// Hand-declared FFI for the same reason the clipboard in `mod.rs` is: the
// project policy (ADR-0002's neighbours in DECISIONS) is that user32 is already
// linked for winit, so a three-call question does not earn a dependency. The
// calls are `EnumDisplayMonitors` + `GetMonitorInfoW`, and they answer with
// *work areas* rather than monitor rectangles on purpose: a maximized window
// sits in the work area (winit keeps `WS_CAPTION` on a frameless window and
// fakes the frame in `WM_NCCALCSIZE`, so Windows still reserves the taskbar),
// and a restored window the user can reach is a window inside the same area.
//
// Physical pixels, not logical. The process is per-monitor-DPI aware (winit sets
// that up before the window exists), so these are the same coordinates
// `Window::position()` / `Window::size()` report and the same ones `set_position`
// takes. The unit question — a rectangle measured on a 150% display and replayed
// on a 100% one — is answered by the clamp below rather than by a unit change,
// because the size is persisted in physical pixels for the zoom's sake
// (ADR-0095: "the window's physical size never changes") and mixing units in one
// record is how a saved window ends up a third of its intended size.

/// A rectangle in physical screen pixels, virtual-desktop coordinates (so a
/// secondary monitor to the left of the primary carries negative x — that is
/// correct, not a bug).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

impl Rect {
    pub fn width(&self) -> f64 {
        (self.right - self.left).max(0.0)
    }
    pub fn height(&self) -> f64 {
        (self.bottom - self.top).max(0.0)
    }
    fn contains(&self, other: &Rect) -> bool {
        other.left >= self.left
            && other.top >= self.top
            && other.right <= self.right
            && other.bottom <= self.bottom
    }
    /// How much of `other` lands on this one, in square pixels. The measure that
    /// decides *which* monitor a partly-off-screen window belonged to — which is
    /// why there is no `intersects` here: "overlaps at all" is not the question,
    /// "overlaps *most*" is, and a boolean would throw away the answer.
    fn overlap_area(&self, other: &Rect) -> f64 {
        let w = (self.right.min(other.right) - self.left.max(other.left)).max(0.0);
        let h = (self.bottom.min(other.bottom) - self.top.max(other.top)).max(0.0);
        w * h
    }
}

#[cfg(target_os = "windows")]
#[allow(non_snake_case)] // the Win32 field names are the ABI's, not this file's
mod ffi {
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct RECT {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    /// `MONITORINFO`, the size Windows documents for it. `rcMonitor` is the
    /// whole panel and `rcWork` is the same panel minus the taskbar and any
    /// app bars; only `rcWork` is read, and the first field is what makes the
    /// struct's own size match the ABI's.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct MONITORINFO {
        pub cbSize: u32,
        pub rcMonitor: RECT,
        pub rcWork: RECT,
        pub dwFlags: u32,
    }

    pub const MONITORINFOF_PRIMARY: u32 = 0x0000_0001;

    pub type MonitorEnumProc = unsafe extern "system" fn(isize, isize, *mut RECT, isize) -> i32;

    #[link(name = "user32")]
    extern "system" {
        pub fn EnumDisplayMonitors(
            hdc: isize,
            lprc_clip: *const RECT,
            lpfn_enum: Option<MonitorEnumProc>,
            dw_data: isize,
        ) -> i32;
        pub fn GetMonitorInfoW(hmon: isize, lpmi: *mut MONITORINFO) -> i32;
    }
}

/// Every monitor's work area, primary first if it can be identified.
///
/// Empty on a non-Windows target and on a headless session (a session-0
/// service has no desktop to enumerate); callers must treat "no monitors" as
/// "cannot place the window", not as "the window is at 0,0".
pub fn work_areas() -> Vec<Rect> {
    #[cfg(target_os = "windows")]
    {
        use std::cell::RefCell;

        // `EnumDisplayMonitors` is a callback API and the only way to carry a
        // vector through it is thread-local scratch. winit's event loop owns
        // this thread, so the value is read on the line after the call returns
        // and cannot outlive it.
        thread_local! {
            static SCRATCH: RefCell<Vec<Rect>> = const { RefCell::new(Vec::new()) };
        }
        SCRATCH.with(|c| c.borrow_mut().clear());

        unsafe extern "system" fn collect(
            hmon: isize,
            _hdc: isize,
            _rect: *mut ffi::RECT,
            _data: isize,
        ) -> i32 {
            let mut info = ffi::MONITORINFO {
                cbSize: std::mem::size_of::<ffi::MONITORINFO>() as u32,
                rcMonitor: ffi::RECT { left: 0, top: 0, right: 0, bottom: 0 },
                rcWork: ffi::RECT { left: 0, top: 0, right: 0, bottom: 0 },
                dwFlags: 0,
            };
            if ffi::GetMonitorInfoW(hmon, &mut info) == 0 {
                // A monitor that will not describe itself is skipped rather than
                // returned as a zero rectangle: a zero rect is "nowhere", and
                // clamping into it would put the window at the origin.
                return 1;
            }
            let rect = Rect {
                left: info.rcWork.left as f64,
                top: info.rcWork.top as f64,
                right: info.rcWork.right as f64,
                bottom: info.rcWork.bottom as f64,
            };
            // `rcMonitor` (the whole panel) is initialized because the ABI's
            // struct layout depends on it being there, and read by nothing here:
            // this module asks about work areas, because that is where a window
            // a user can reach lives — a maximized one included.
            let _ = info.rcMonitor;
            let primary = info.dwFlags & ffi::MONITORINFOF_PRIMARY != 0;
            SCRATCH.with(|c| {
                let mut v = c.borrow_mut();
                // The primary goes in front so a caller that wants "the first
                // one" as a fallback gets the right monitor.
                if primary {
                    v.insert(0, rect);
                } else {
                    v.push(rect);
                }
            });
            1 // TRUE: keep enumerating
        }

        unsafe {
            ffi::EnumDisplayMonitors(0, std::ptr::null(), Some(collect), 0);
        }
        SCRATCH.with(|c| c.borrow().clone())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

/// The window rectangle to open with: the one that was saved, moved or shrunk
/// just enough that a user can reach all of it.
///
/// Three cases, in order:
/// 1. **Fully inside some work area** — returned untouched. This is the ordinary
///    launch, and the whole point is that it is not touched.
/// 2. **Overlapping one** (the window hangs off an edge, or it is bigger than the
///    display it lands on) — shrunk to that work area if it does not already fit,
///    then moved inside it. A window whose bottom third is under the taskbar is
///    a window whose ＋ 添加任务 line cannot be clicked.
/// 3. **On no monitor at all** — the monitor it was saved on is gone, which is
///    what unplugging a second display looks like from here. Centred on the
///    primary, size kept if it fits and shrunk if it does not. Keeping the size
///    matters: someone who liked a 1600-wide window on a 4K panel should get
///    that window back on a 1080p primary, not a 1080-wide one.
///
/// Pure on purpose — every branch above is a decision about numbers, so the
/// tests at the bottom run without a desktop, and `main.rs` gets the same answer
/// on a machine with one monitor and on one with three.
pub fn place(saved: Rect, areas: &[Rect]) -> Rect {
    // Nothing to check against: keep the window exactly as it was rather than
    // guessing a desktop it might be on.
    if areas.is_empty() {
        return saved;
    }
    if areas.iter().any(|a| a.contains(&saved)) {
        return saved;
    }
    let home = areas
        .iter()
        .max_by(|a, b| {
            a.overlap_area(&saved)
                .partial_cmp(&b.overlap_area(&saved))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
        .unwrap_or(areas[0]);
    let w = saved.width().min(home.width());
    let h = saved.height().min(home.height());
    // Centre rather than pin to a corner: the saved top-left belonged to a
    // monitor that may be anywhere, and a window in the corner of an unrelated
    // panel reads as a bug even when it is reachable.
    let x = home.left + ((home.width() - w) / 2.0).round();
    let y = home.top + ((home.height() - h) / 2.0).round();
    Rect { left: x, top: y, right: x + w, bottom: y + h }
}

impl Rect {
    /// Whether the point sits on this panel. `right`/`bottom` are exclusive the
    /// way Windows draws them: a cursor on the last pixel column of the primary
    /// is on the primary, and one on the second monitor's first column is not.
    fn carries(&self, x: f64, y: f64) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// The `w`×`h` rectangle centred in `area`. Pure, so the placement is a number
/// anyone can test without a desktop. A rectangle bigger than its area is pinned
/// to the area's top-left rather than centred on a negative offset: the alternative
/// is to put the composer's *bottom* — the 保存 row — off the panel.
fn center_in(area: Rect, w: f64, h: f64) -> Rect {
    let x = area.left + ((area.width() - w) / 2.0).max(0.0).round();
    let y = area.top + ((area.height() - h) / 2.0).max(0.0).round();
    Rect { left: x, top: y, right: x + w, bottom: y + h }
}

/// Where a pop-up that answers to the *mouse* — Alt+N's 速记, first press of the
/// session — should open: centred in the work area the cursor is on.
///
/// `None` when there is nothing to place it on (no monitors, no cursor). The
/// caller then leaves the window where the platform put it rather than guessing
/// an origin, which is `place`'s rule for the same question.
pub fn centered_on_cursor(size: (f64, f64)) -> Option<Rect> {
    let areas = work_areas();
    let (w, h) = size;
    let home = match cursor() {
        Some((x, y)) => areas.iter().find(|a| a.carries(x, y)).copied(),
        // A cursor the desktop will not report (a session with no pointing
        // device) still has a primary to open on.
        None => None,
    }
    .or_else(|| areas.first().copied())?;
    Some(center_in(home, w, h))
}

/// The pointer's position, in the same physical pixels as the work areas.
#[cfg(target_os = "windows")]
fn cursor() -> Option<(f64, f64)> {
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
    }
    let mut p = Point { x: 0, y: 0 };
    let ok = unsafe { GetCursorPos(&mut p) };
    if ok != 0 {
        Some((p.x as f64, p.y as f64))
    } else {
        None
    }
}

#[cfg(not(target_os = "windows"))]
fn cursor() -> Option<(f64, f64)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(l: f64, t: f64, rr: f64, b: f64) -> Rect {
        Rect { left: l, top: t, right: rr, bottom: b }
    }

    /// One 1920×1080 primary and a 2560×1440 secondary to its right — the desk
    /// this module exists for.
    fn two_monitors() -> Vec<Rect> {
        vec![r(0.0, 0.0, 1920.0, 1040.0), r(1920.0, 0.0, 4480.0, 1400.0)]
    }

    #[test]
    fn a_window_inside_a_monitor_is_returned_untouched() {
        let areas = two_monitors();
        let saved = r(300.0, 120.0, 1500.0, 920.0);
        assert_eq!(place(saved, &areas), saved, "the ordinary launch is not adjusted");
    }

    #[test]
    fn a_window_on_the_secondary_comes_back_on_the_secondary() {
        // Negative x: a panel to the *left* of the primary. The virtual desktop
        // origin is the primary's, so this is a normal coordinate, not a bug.
        let areas = vec![r(0.0, 0.0, 1920.0, 1040.0), r(-2560.0, -200.0, 0.0, 1240.0)];
        let saved = r(-2000.0, 100.0, -600.0, 900.0);
        assert_eq!(place(saved, &areas), saved);
    }

    #[test]
    fn a_window_hanging_off_the_bottom_is_moved_back_inside() {
        let areas = two_monitors();
        // Saved 900 tall on a 1040-tall work area, but 120 from the bottom.
        let placed = place(r(100.0, 940.0, 1300.0, 1840.0), &areas);
        assert!(areas[0].contains(&placed), "{placed:?} must be inside {:?}", areas[0]);
        assert_eq!(placed.width(), 1200.0, "a window that already fits is not resized");
        assert_eq!(placed.height(), 900.0, "nor in the other axis");
    }

    #[test]
    fn a_window_too_big_for_the_display_it_lands_on_is_shrunk_to_fit() {
        let areas = two_monitors();
        // 2400×1300 saved: wider and taller than the primary's work area, and
        // starting at the origin, so it lands on the primary and does not fit.
        let placed = place(r(0.0, 0.0, 2400.0, 1300.0), &areas);
        assert!(areas[0].contains(&placed), "{placed:?} must be inside {:?}", areas[0]);
        assert_eq!(placed.width(), 1920.0);
        assert_eq!(placed.height(), 1040.0);
    }

    #[test]
    fn a_window_whose_monitor_is_gone_lands_centred_on_the_primary() {
        let areas = vec![r(0.0, 0.0, 1920.0, 1040.0)];
        // Saved at x = 3000: a 2560-wide panel that is no longer plugged in.
        let placed = place(r(3000.0, 200.0, 4400.0, 1100.0), &areas);
        assert!(areas[0].contains(&placed), "{placed:?} must be inside {:?}", areas[0]);
        assert_eq!(
            placed.width(),
            1400.0,
            "the size the user chose is kept when the primary can hold it"
        );
    }

    #[test]
    fn a_window_gone_monitor_and_too_big_is_shrunk_and_centred() {
        let areas = vec![r(0.0, 0.0, 1280.0, 720.0)];
        let placed = place(r(3000.0, 200.0, 6000.0, 2000.0), &areas);
        assert_eq!(placed.width(), 1280.0);
        assert_eq!(placed.height(), 720.0);
        assert_eq!(placed.left, 0.0);
        assert_eq!(placed.top, 0.0);
    }

    #[test]
    fn no_monitors_means_keep_the_window_rather_than_guess() {
        let saved = r(10.0, 20.0, 900.0, 700.0);
        assert_eq!(place(saved, &[]), saved, "a headless session gets no opinion");
    }

    #[test]
    fn the_enumeration_agrees_with_this_machine_when_there_is_one() {
        // Not a fixture test: a session with a desktop must enumerate at least
        // one work area, and every one of them must be a real rectangle. A
        // session-0 service (or a CI box) has none, and that is allowed.
        let areas = work_areas();
        if areas.is_empty() {
            return;
        }
        assert!(
            areas.iter().all(|a| a.width() > 0.0 && a.height() > 0.0),
            "a monitor that will not describe itself is skipped, not returned empty: {areas:?}"
        );
    }

    // ─── where a pop-up that answers to the mouse goes ───────────────────────

    #[test]
    fn a_panel_carries_its_top_left_and_not_its_bottom_right() {
        let primary = r(0.0, 0.0, 1920.0, 1040.0);
        assert!(primary.carries(0.0, 0.0));
        assert!(primary.carries(1919.0, 1039.0), "the last real pixel is on the panel");
        assert!(!primary.carries(1920.0, 500.0), "the next panel's first column is not this one");
        assert!(!primary.carries(500.0, 1040.0), "nor the row below the taskbar");
        // The virtual desktop's origin is the primary's, so a left-hand panel
        // carries negative x — and the primary does not answer for it.
        let left = r(-2560.0, 0.0, 0.0, 1400.0);
        assert!(left.carries(-2560.0, 10.0));
        assert!(left.carries(-1.0, 10.0), "x = -1 is the left panel's last column");
        assert!(!primary.carries(-1.0, 10.0), "and nothing of it is on the primary");
    }

    #[test]
    fn a_pop_up_is_centred_in_the_area_it_lands_on() {
        let area = r(0.0, 0.0, 1920.0, 1040.0);
        let placed = center_in(area, 560.0, 340.0);
        assert_eq!(placed, r(680.0, 350.0, 1240.0, 690.0));
        assert!(area.contains(&placed));
        // A secondary's own coordinates, taskbar offset and all: centring is
        // arithmetic on *that* panel, never on the virtual desktop.
        let second = r(1920.0, 0.0, 4480.0, 1400.0);
        let placed = center_in(second, 560.0, 340.0);
        assert_eq!((placed.left, placed.top), (2920.0, 530.0));
        assert!(second.contains(&placed));
    }

    #[test]
    fn a_pop_up_bigger_than_its_panel_keeps_its_buttons_on_screen() {
        // 900 tall on a 700-tall work area: centring would put the top at -100
        // and the 保存 row off the bottom of the panel. Pinning to the origin
        // loses the off-screen overflow instead, which is the half the user
        // cannot reach anyway.
        let small = r(0.0, 0.0, 800.0, 700.0);
        let placed = center_in(small, 1000.0, 900.0);
        assert_eq!((placed.left, placed.top), (0.0, 0.0));
        // Same on a panel that does not start at the origin.
        let left = r(-2560.0, -200.0, 0.0, 1200.0);
        let placed = center_in(left, 4000.0, 2000.0);
        assert_eq!((placed.left, placed.top), (-2560.0, -200.0));
    }
}
