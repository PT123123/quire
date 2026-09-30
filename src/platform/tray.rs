// The native tray icon (Windows). Slint's own `SystemTrayIcon` cannot report a
// click on this platform, and the menu it pops is the shell's — light, whatever
// the app's theme says. This module owns the icon instead: the window's
// left-click shows the main window, and the right-click menu is themed to match
// the app (`DarkMode_Explorer` for a dark theme, the shell's own for a light
// one), read at the moment the menu opens so a theme switch needs no restart.
//
// The shape follows `platform::quit`: hand-declared FFI only, one background
// thread with one hidden window, and the app's answers hop to the UI thread
// through the closures the caller hands in — this module never touches Slint.
//
// Because the icon is not a Slint element it counts for no keepalive: the
// Windows session runs `run_event_loop_until_quit` (main.rs), so hiding the
// window does not end the session, and the menu's 「退出」 stays the one exit.
//
// Off-Windows the module still compiles — `install` refuses — so the callers
// need no cfg of their own beyond choosing the fallback.

/// The menu should follow the app's theme. Written by the settings' theme
/// handler and at startup; read on every right-click.
pub const THEME_LIGHT: u8 = 0;
pub const THEME_DARK: u8 = 1;
pub const THEME_SYSTEM: u8 = 2;

static THEME_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(THEME_LIGHT);

/// Tell the menu what the app looks like now. Relaxed is enough: the value is
/// cosmetic, and the menu re-reads it each time it opens.
pub fn set_theme_mode(mode: u8) {
    THEME_MODE.store(mode, std::sync::atomic::Ordering::Relaxed);
}

/// Map a `theme` settings id onto the menu mode. Everything but the light
/// palette is dark; `system` asks Windows what the apps are set to, answered
/// live at menu time — so a Windows theme flip needs no restart either.
pub fn theme_mode_of(theme: &str) -> u8 {
    match theme {
        "light" => THEME_LIGHT,
        "system" => THEME_SYSTEM,
        _ => THEME_DARK,
    }
}

pub struct Tray {
    hwnd: isize,
}

impl Tray {
    /// Remove the icon (NIM_DELETE). Called after the event loop has returned,
    /// so the notification area never shows this session's ghost.
    pub fn remove(&self) {
        #[cfg(target_os = "windows")]
        imp::remove_icon(self.hwnd);
    }
}

/// Put the icon in the notification area and start its thread.
///
/// `on_show` answers the icon's left-click and the menu's 「显示主界面」;
/// `on_quit` the menu's 「退出」. Both run on the tray thread and must hop
/// themselves into the UI thread (the callers wrap `invoke_from_event_loop`).
/// The id-string of the embedded icon resource is `IDI_MAIN` (`build.rs`'s
/// generated .rc), loaded by name.
pub fn install(
    on_show: Box<dyn Fn() + Send>,
    on_quit: Box<dyn Fn() + Send>,
) -> Result<Tray, String> {
    #[cfg(target_os = "windows")]
    {
        imp::install(on_show, on_quit)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (on_show, on_quit);
        Err("the native tray is Windows-only; the Slint tray is the fallback".into())
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::sync::atomic::Ordering;

    use super::{THEME_DARK, THEME_MODE, THEME_SYSTEM, Tray};

    // ── the hand-declared surface (the rule `platform::mod` spells out) ──────
    const WM_APP: u32 = 0x8000;
    // the tray's own callback message and the handle's remove request
    const TRAY_CALLBACK: u32 = WM_APP + 1;
    const TRAY_REMOVE: u32 = WM_APP + 2;
    // mouse messages the callback's lParam low word carries
    const WM_LBUTTONUP: usize = 0x0202;
    const WM_RBUTTONUP: usize = 0x0205;
    const WM_INITMENUPOPUP: u32 = 0x0117;
    const WM_CLOSE: u32 = 0x0010;
    const WM_DESTROY: u32 = 0x0002;
    const WM_NULL: usize = 0;

    const NIM_ADD: u32 = 0;
    const NIM_DELETE: u32 = 2;
    const NIF_MESSAGE: u32 = 1;
    const NIF_ICON: u32 = 2;
    const NIF_TIP: u32 = 4;

    const IMAGE_ICON: u32 = 1;
    const LR_DEFAULTSIZE: u32 = 0x40;

    const WS_OVERLAPPED: u32 = 0;
    const WS_EX_TOOLWINDOW: u32 = 0x80;

    const MF_STRING: u32 = 0;
    const MF_SEPARATOR: u32 = 0x800;
    const TPM_RIGHTBUTTON: u32 = 0x0002;
    const TPM_BOTTOMALIGN: u32 = 0x0020;
    const TPM_RETURNCMD: u32 = 0x0100;

    const MENU_SHOW: u32 = 1;
    const MENU_QUIT: u32 = 2;

    const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as isize;
    const KEY_READ: u32 = 0x20019;

    #[link(name = "shell32")]
    extern "system" {
        fn Shell_NotifyIconW(message: u32, data: *const NotifyIconDataW) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn RegisterClassW(class: *const WndClassW) -> u16;
        fn CreateWindowExW(
            ex_style: u32,
            class: *const u16,
            name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: isize,
            menu: isize,
            instance: isize,
            param: *mut std::ffi::c_void,
        ) -> isize;
        fn DefWindowProcW(hwnd: isize, msg: u32, wp: usize, lp: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *const Msg) -> i32;
        fn DispatchMessageW(msg: *const Msg) -> isize;
        fn PostQuitMessage(code: i32);
        fn DestroyWindow(hwnd: isize) -> i32;
        fn PostMessageW(hwnd: isize, msg: u32, wp: usize, lp: isize) -> i32;
        fn LoadImageW(
            instance: isize,
            name: *const u16,
            kind: u32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> isize;
        fn GetModuleHandleW(name: *const u16) -> isize;
        fn SetForegroundWindow(hwnd: isize) -> i32;
        fn GetCursorPos(point: *mut Point) -> i32;
        fn CreatePopupMenu() -> isize;
        fn AppendMenuW(menu: isize, flags: u32, id: usize, text: *const u16) -> i32;
        fn DestroyMenu(menu: isize) -> i32;
        fn TrackPopupMenu(
            menu: isize,
            flags: u32,
            x: i32,
            y: i32,
            reserved: i32,
            hwnd: isize,
            rect: *const std::ffi::c_void,
        ) -> i32;
        fn FindWindowW(class: *const u16, name: *const u16) -> isize;
        fn RegisterWindowMessageW(name: *const u16) -> u32;
    }
    #[link(name = "uxtheme")]
    extern "system" {
        fn SetWindowTheme(hwnd: isize, sub_app: *const u16, sub_list: *const u16) -> i32;
    }
    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            key: isize,
            sub_key: *const u16,
            options: u32,
            access: u32,
            result: *mut isize,
        ) -> i32;
        fn RegQueryValueExW(
            key: isize,
            name: *const u16,
            reserved: *mut u32,
            kind: *mut u32,
            data: *mut u8,
            bytes: *mut u32,
        ) -> i32;
        fn RegCloseKey(key: isize) -> i32;
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: isize,
        message: u32,
        wp: usize,
        lp: isize,
        time: u32,
        pt: Point,
    }

    #[repr(C)]
    struct WndClassW {
        style: u32,
        window_proc: unsafe extern "system" fn(isize, u32, usize, isize) -> isize,
        cls_extra: i32,
        wnd_extra: i32,
        instance: isize,
        icon: isize,
        cursor: isize,
        background: isize,
        menu_name: *const u16,
        class_name: *const u16,
        icon_small: isize,
    }

    /// The full Vista+ layout: the size the OS is told is the struct's own
    /// `size_of`, and every field after `szTip` is declared so the offset of
    /// the ones before it is the header's. Nothing past `szInfoTitle` is used.
    #[repr(C)]
    struct NotifyIconDataW {
        cb_size: u32,
        hwnd: isize,
        u_id: u32,
        u_flags: u32,
        callback_message: u32,
        icon: isize,
        tip: [u16; 128],
        state: u32,
        state_mask: u32,
        info: [u16; 256],
        version: u32,
        info_title: [u16; 64],
        info_flags: u32,
        guid_item: [u32; 4],
    }

    impl NotifyIconDataW {
        fn new(hwnd: isize, icon: isize) -> Self {
            let mut data = Self {
                cb_size: std::mem::size_of::<Self>() as u32,
                hwnd,
                u_id: 1,
                u_flags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                callback_message: TRAY_CALLBACK,
                icon,
                tip: [0; 128],
                state: 0,
                state_mask: 0,
                info: [0; 256],
                version: 0,
                info_title: [0; 64],
                info_flags: 0,
                guid_item: [0; 4],
            };
            for (slot, ch) in data.tip.iter_mut().zip("Quire".encode_utf16()) {
                *slot = ch;
            }
            data
        }
    }

    fn wide(s: &str) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// What Windows says the *apps* are set to: `AppsUseLightTheme` is 0 for
    /// dark. A missing row or a refused read answers "light" — the shell's own
    /// default, and the safe guess for a theme this build cannot see.
    fn system_is_light() -> bool {
        unsafe {
            let sub = wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
            let name = wide("AppsUseLightTheme");
            let mut key = 0isize;
            if RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_READ, &mut key) != 0 {
                return true;
            }
            let mut value = 1u32;
            let mut bytes = 4u32;
            let ok = RegQueryValueExW(
                key,
                name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                (&mut value as *mut u32).cast(),
                &mut bytes,
            ) == 0
                && bytes == 4;
            RegCloseKey(key);
            if ok {
                value == 1
            } else {
                true
            }
        }
    }

    /// The menu window (`#32768`) does not exist until the popup opens, so the
    /// theme rides `WM_INITMENUPOPUP`: find it and hand it the theme word that
    /// matches the app. `Explorer` is the shell's own (light); the `DarkMode_`
    /// variant is what turns a classic popup menu dark on this platform.
    fn theme_menu_window() {
        let dark = match THEME_MODE.load(Ordering::Relaxed) {
            THEME_DARK => true,
            THEME_SYSTEM => !system_is_light(),
            _ => false,
        };
        unsafe {
            let class = wide("#32768");
            let menu_window = FindWindowW(class.as_ptr(), std::ptr::null());
            if menu_window != 0 {
                let theme = wide(if dark { "DarkMode_Explorer" } else { "Explorer" });
                SetWindowTheme(menu_window, theme.as_ptr(), std::ptr::null());
            }
        }
    }

    struct Ctx {
        on_show: Box<dyn Fn() + Send>,
        on_quit: Box<dyn Fn() + Send>,
    }

    /// Post the remove request to the tray window: its own thread answers with
    /// NIM_DELETE and winds the message loop down.
    pub fn remove_icon(hwnd: isize) {
        unsafe {
            PostMessageW(hwnd, TRAY_REMOVE, 0, 0);
        }
    }

    thread_local! {
        static CTX: RefCell<Option<Ctx>> = const { RefCell::new(None) };
        static TASKBAR_CREATED: Cell<u32> = const { Cell::new(0) };
    }

    unsafe extern "system" fn wndproc(hwnd: isize, msg: u32, wp: usize, lp: isize) -> isize {
        if msg == TRAY_CALLBACK {
            match lp as usize {
                WM_LBUTTONUP => {
                    CTX.with(|c| {
                        if let Some(ctx) = c.borrow().as_ref() {
                            (ctx.on_show)();
                        }
                    });
                }
                WM_RBUTTONUP => {
                    // The two calls around TrackPopupMenu are the tray menu's
                    // own rite: without SetForegroundWindow the menu does not
                    // dismiss on an outside click, and the WM_NULL afterwards
                    // lets the window actually lose that foreground.
                    let menu = CreatePopupMenu();
                    if menu == 0 {
                        return 0;
                    }
                    let show = wide("显示主界面");
                    let quit = wide("退出");
                    AppendMenuW(menu, MF_STRING, MENU_SHOW as usize, show.as_ptr());
                    AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
                    AppendMenuW(menu, MF_STRING, MENU_QUIT as usize, quit.as_ptr());
                    let mut point = Point { x: 0, y: 0 };
                    GetCursorPos(&mut point);
                    SetForegroundWindow(hwnd);
                    theme_menu_window();
                    let picked = TrackPopupMenu(
                        menu,
                        TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD,
                        point.x,
                        point.y,
                        0,
                        hwnd,
                        std::ptr::null(),
                    );
                    PostMessageW(hwnd, WM_NULL as u32, 0, 0);
                    DestroyMenu(menu);
                    CTX.with(|c| {
                        if let Some(ctx) = c.borrow().as_ref() {
                            match picked as u32 {
                                MENU_SHOW => (ctx.on_show)(),
                                MENU_QUIT => (ctx.on_quit)(),
                                _ => {}
                            }
                        }
                    });
                }
                _ => {}
            }
            return 0;
        }
        if msg == WM_INITMENUPOPUP {
            // fires before each popup paints, so the theme is always current
            theme_menu_window();
            return 0;
        }
        if msg == TRAY_REMOVE {
            let mut data = NotifyIconDataW::new(hwnd, 0);
            data.u_flags = 0;
            unsafe {
                Shell_NotifyIconW(NIM_DELETE, &data);
                DestroyWindow(hwnd);
            }
            return 0;
        }
        if msg == WM_CLOSE {
            unsafe {
                let mut data = NotifyIconDataW::new(hwnd, 0);
                data.u_flags = 0;
                Shell_NotifyIconW(NIM_DELETE, &data);
                DestroyWindow(hwnd);
            }
            return 0;
        }
        if msg == WM_DESTROY {
            unsafe {
                let mut data = NotifyIconDataW::new(hwnd, 0);
                data.u_flags = 0;
                Shell_NotifyIconW(NIM_DELETE, &data);
                PostQuitMessage(0);
            }
            return 0;
        }
        // Explorer restarted and the tray went with it: re-add while the
        // session is still alive, or the app would be unreachable until it
        // quit for an unrelated reason.
        if TASKBAR_CREATED.get() != 0 && msg == TASKBAR_CREATED.get() {
            unsafe {
                let icon = LoadImageW(
                    GetModuleHandleW(std::ptr::null()),
                    wide("IDI_MAIN").as_ptr(),
                    IMAGE_ICON,
                    0,
                    0,
                    LR_DEFAULTSIZE,
                );
                let data = NotifyIconDataW::new(hwnd, icon);
                Shell_NotifyIconW(NIM_ADD, &data);
            }
            return 0;
        }
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }

    pub fn install(on_show: Box<dyn Fn() + Send>, on_quit: Box<dyn Fn() + Send>) -> Result<Tray, String> {
        // The window is created on the thread that will pump it: a tray
        // window's messages must be dispatched by its own loop, and a window
        // created on the UI thread would tie both to Slint's.
        let (tx, rx) = std::sync::mpsc::channel::<Result<isize, String>>();
        let spawned = std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || {
                let _hwnd = unsafe {
                    let class_name = wide("QuireTray");
                    let class = WndClassW {
                        style: 0,
                        window_proc: wndproc,
                        cls_extra: 0,
                        wnd_extra: 0,
                        instance: GetModuleHandleW(std::ptr::null()),
                        icon: 0,
                        cursor: 0,
                        background: 0,
                        menu_name: std::ptr::null(),
                        class_name: class_name.as_ptr(),
                        icon_small: 0,
                    };
                    if RegisterClassW(&class) == 0 {
                        let _ = tx.send(Err("RegisterClassW refused the tray window".into()));
                        return;
                    }
                    let class_for_window = wide("QuireTray");
                    let name = wide("Quire tray");
                    let hwnd = CreateWindowExW(
                        WS_EX_TOOLWINDOW,
                        class_for_window.as_ptr(),
                        name.as_ptr(),
                        WS_OVERLAPPED,
                        0,
                        0,
                        0,
                        0,
                        0,
                        0,
                        GetModuleHandleW(std::ptr::null()),
                        std::ptr::null_mut(),
                    );
                    if hwnd == 0 {
                        let _ = tx.send(Err("CreateWindowExW refused the tray window".into()));
                        return;
                    }
                    let icon = LoadImageW(
                        GetModuleHandleW(std::ptr::null()),
                        wide("IDI_MAIN").as_ptr(),
                        IMAGE_ICON,
                        0,
                        0,
                        LR_DEFAULTSIZE,
                    );
                    if icon == 0 {
                        let _ = tx.send(Err("the icon resource did not load".into()));
                        return;
                    }
                    let data = NotifyIconDataW::new(hwnd, icon);
                    if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
                        let _ = tx.send(Err("Shell_NotifyIconW refused the icon".into()));
                        return;
                    }
                    let created = wide("TaskbarCreated");
                    TASKBAR_CREATED.set(RegisterWindowMessageW(created.as_ptr()));
                    let _ = tx.send(Ok(hwnd));
                    hwnd
                };
                CTX.with(|c| {
                    *c.borrow_mut() = Some(Ctx { on_show, on_quit });
                });
                let mut msg = Msg {
                    hwnd: 0,
                    message: 0,
                    wp: 0,
                    lp: 0,
                    time: 0,
                    pt: Point { x: 0, y: 0 },
                };
                unsafe {
                    while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            })
            .map_err(|e| format!("the tray thread did not start: {e}"))?;
        let hwnd = rx
            .recv()
            .map_err(|_| "the tray thread ended before answering".to_string())?;
        let _ = spawned; // the thread outlives this function; the loop ends on quit
        hwnd.map(|hwnd| Tray { hwnd })
    }
}
