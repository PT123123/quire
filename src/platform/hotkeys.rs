// Global hotkeys (Windows): Alt+N opens the note capture, Alt+M shows the main
// window. Both are **settings, off by default** — a keystroke that reaches the
// app from inside every other application is not something to turn on for a
// user who never asked, so the registration only happens when the settings row
// says so, and `set_enabled` is how the settings' toggles flip it live.
//
// `RegisterHotKey` is thread-affine: the keys are registered on this module's
// own thread (one hidden window, one message loop — the same shape
// `platform::tray` uses), and the answers hop to the UI thread through the
// closures the caller handed in. `set_enabled` rides `SendMessageW`, so the
// registration result — a key another application already owns — comes back to
// the caller as an `Err` instead of dying in the log.
//
// Off-Windows the module compiles and refuses, so the callers need no cfg of
// their own.

/// The two ids the settings toggles and the registration share.
pub const ALT_N: u8 = 1;
pub const ALT_M: u8 = 2;

pub struct Hotkeys {
    hwnd: isize,
}

impl Hotkeys {
    /// Register (`on`) or unregister the hotkey named by `id`. Synchronous: the
    /// answer is the OS's own, so a taken shortcut reads as an `Err` at the
    /// very moment the user flipped the switch.
    pub fn set_enabled(&self, id: u8, on: bool) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            imp::set_enabled(self.hwnd, id, on)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (id, on);
            Err("global hotkeys are Windows-only".into())
        }
    }
}

/// Start the hotkey thread. `on_alt_n` / `on_alt_m` run on this thread when
/// their key fires and must hop themselves into the UI thread (the callers wrap
/// `invoke_from_event_loop`). Registration itself waits for the toggles.
pub fn install(
    on_alt_n: Box<dyn Fn() + Send>,
    on_alt_m: Box<dyn Fn() + Send>,
) -> Result<Hotkeys, String> {
    #[cfg(target_os = "windows")]
    {
        imp::install(on_alt_n, on_alt_m)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (on_alt_n, on_alt_m);
        Err("global hotkeys are Windows-only".into())
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::Hotkeys;

    const WM_APP: u32 = 0x8000;
    // wparam carries the id, lparam the on/off: one message, both directions
    const HK_SET: u32 = WM_APP;
    const WM_HOTKEY: u32 = 0x0312;
    const WM_DESTROY: u32 = 0x0002;
    const MOD_ALT: u32 = 0x0001;
    // holding the key must not queue a burst of notes
    const MOD_NOREPEAT: u32 = 0x4000;
    const WS_OVERLAPPED: u32 = 0;
    const WS_EX_TOOLWINDOW: u32 = 0x80;
    // 'N' and 'M' — the two keys the settings name
    const VK_N: u32 = 0x4E;
    const VK_M: u32 = 0x4D;

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
        fn SendMessageW(hwnd: isize, msg: u32, wp: usize, lp: isize) -> isize;
        fn RegisterHotKey(hwnd: isize, id: i32, modifiers: u32, vk: u32) -> i32;
        fn UnregisterHotKey(hwnd: isize, id: i32) -> i32;
        fn GetModuleHandleW(name: *const u16) -> isize;
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

    fn wide(s: &str) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    struct Ctx {
        on_alt_n: Box<dyn Fn() + Send>,
        on_alt_m: Box<dyn Fn() + Send>,
    }

    thread_local! {
        static CTX: std::cell::RefCell<Option<Ctx>> = const { std::cell::RefCell::new(None) };
    }

    fn virtual_key(id: i32) -> u32 {
        if id == super::ALT_N as i32 {
            VK_N
        } else {
            VK_M
        }
    }

    /// Ask the hotkey window (synchronously) to register or unregister one key.
    /// The window's answer is the OS's own result, so a shortcut another
    /// application owns reads as the caller's `Err`.
    pub fn set_enabled(hwnd: isize, id: u8, on: bool) -> Result<(), String> {
        let ok = unsafe { SendMessageW(hwnd, HK_SET, id as usize, on as isize) };
        if ok != 0 {
            Ok(())
        } else {
            Err(format!(
                "the hotkey id {id} could not be registered — another application likely owns it"
            ))
        }
    }

    unsafe extern "system" fn wndproc(hwnd: isize, msg: u32, wp: usize, lp: isize) -> isize {
        if msg == HK_SET {
            let id = wp as i32;
            // the return value is the answer `set_enabled` reads
            return if lp != 0 {
                RegisterHotKey(hwnd, id, MOD_ALT | MOD_NOREPEAT, virtual_key(id)) as isize
            } else {
                UnregisterHotKey(hwnd, id);
                1 // unregistering cannot meaningfully fail here
            };
        }
        if msg == WM_HOTKEY {
            CTX.with(|c| {
                if let Some(ctx) = c.borrow().as_ref() {
                    match wp as u8 {
                        super::ALT_N => (ctx.on_alt_n)(),
                        _ => (ctx.on_alt_m)(),
                    }
                }
            });
            return 0;
        }
        if msg == WM_DESTROY {
            // the keys die with the thread's window; nothing to unhook by hand
            unsafe { PostQuitMessage(0) };
            return 0;
        }
        unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
    }

    pub fn install(
        on_alt_n: Box<dyn Fn() + Send>,
        on_alt_m: Box<dyn Fn() + Send>,
    ) -> Result<Hotkeys, String> {
        let (tx, rx) = std::sync::mpsc::channel::<Result<isize, String>>();
        std::thread::Builder::new()
            .name("hotkeys".into())
            .spawn(move || {
                let _hwnd: isize = unsafe {
                    let class_name = wide("QuireHotkeys");
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
                        let _ = tx.send(Err("RegisterClassW refused the hotkey window".into()));
                        return;
                    }
                    let class_for_window = wide("QuireHotkeys");
                    let name = wide("Quire hotkeys");
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
                        let _ = tx.send(Err("CreateWindowExW refused the hotkey window".into()));
                        return;
                    }
                    let _ = tx.send(Ok(hwnd));
                    hwnd
                };
                CTX.with(|c| {
                    *c.borrow_mut() = Some(Ctx { on_alt_n, on_alt_m });
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
            .map_err(|e| format!("the hotkey thread did not start: {e}"))?;
        let hwnd = rx
            .recv()
            .map_err(|_| "the hotkey thread ended before answering".to_string())?;
        hwnd.map(|hwnd| Hotkeys { hwnd })
    }
}
