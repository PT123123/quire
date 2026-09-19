// Platform adapters — only where Slint/Windows forces us to (M8).
// Policy (ADR-0002): never implement TSF/IME ourselves.

/// Copy `text` to the system clipboard. Windows routes through the always
/// present `clip.exe` — the URLs this app copies (`quire://block/<id>`) are
/// ASCII, which is all clip's OEM-codepage stdin handles correctly, so no
/// clipboard crate is pulled in (dependency policy in DECISIONS). Other
/// targets report failure instead of pretending.
pub fn copy_to_clipboard(text: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let Ok(mut child) = Command::new("clip")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        // dropping stdin closes the pipe and lets clip finish
        child
            .stdin
            .take()
            .and_then(|mut s| s.write_all(text.as_bytes()).ok())
            .is_some()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = text;
        false
    }
}

/// Read the system clipboard as text (the rich-paste path, SPEC §二十七).
/// Direct Win32 FFI: a `Get-Clipboard` subprocess was measured at 7-10 s on
/// the dev desktop (PowerShell startup under AV), which no paste can wait
/// for, while `OpenClipboard`/`GetClipboardData` are microseconds and the
/// MSVC toolchain already links user32/kernel32 for winit — so no clipboard
/// crate is pulled in (dependency policy in DECISIONS). Reads
/// CF_UNICODETEXT only; other targets report absence instead of pretending.
pub fn read_clipboard() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        const CF_UNICODETEXT: u32 = 13;

        #[link(name = "user32")]
        extern "system" {
            fn IsClipboardFormatAvailable(format: u32) -> i32;
            fn OpenClipboard(hwnd: isize) -> i32;
            fn CloseClipboard() -> i32;
            fn GetClipboardData(format: u32) -> isize;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalLock(hmem: isize) -> *mut u16;
            fn GlobalUnlock(hmem: isize) -> i32;
        }

        unsafe {
            if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
                return None;
            }
            // another process may hold the clipboard open: a short retry
            // beats failing a paste over a transient lock
            let mut opened = false;
            for _ in 0..5 {
                if OpenClipboard(0) != 0 {
                    opened = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            if !opened {
                return None;
            }
            let text = (|| {
                let handle = GetClipboardData(CF_UNICODETEXT);
                if handle == 0 {
                    return None;
                }
                let ptr = GlobalLock(handle);
                if ptr.is_null() {
                    return None;
                }
                let mut len = 0usize;
                while *ptr.add(len) != 0 {
                    len += 1;
                }
                let s = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
                GlobalUnlock(handle);
                Some(s)
            })();
            CloseClipboard();
            let text = text?.trim_end_matches(['\r', '\n']).to_string();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
