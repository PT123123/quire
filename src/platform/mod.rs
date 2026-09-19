// Platform adapters — only where Slint/Windows forces us to (M8).
// Policy (ADR-0002): never implement TSF/IME ourselves.

/// Copy `text` to the system clipboard, as CF_UNICODETEXT via the same FFI
/// `read_clipboard` uses (ADR-0025's write half: `clip.exe`'s OEM-codepage
/// stdin garbles non-ASCII, and "Copy page as Markdown" must carry CJK).
/// No clipboard crate — user32/kernel32 are already linked for winit.
/// Other targets report failure instead of pretending.
pub fn copy_to_clipboard(text: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        const CF_UNICODETEXT: u32 = 13;
        const GMEM_MOVEABLE: u32 = 0x0002;

        #[link(name = "user32")]
        extern "system" {
            fn OpenClipboard(hwnd: isize) -> i32;
            fn CloseClipboard() -> i32;
            fn EmptyClipboard() -> i32;
            fn SetClipboardData(format: u32, hmem: isize) -> isize;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
            fn GlobalLock(hmem: isize) -> *mut u16;
            fn GlobalUnlock(hmem: isize) -> i32;
            fn GlobalFree(hmem: isize) -> isize;
        }

        // UTF-16 units plus the terminating nul; the allocator wants bytes
        let mut units: Vec<u16> = text.encode_utf16().collect();
        units.push(0);
        let bytes = units.len() * std::mem::size_of::<u16>();

        unsafe {
            // another process may hold the clipboard open: a short retry
            // beats failing a copy over a transient lock
            let mut opened = false;
            for _ in 0..5 {
                if OpenClipboard(0) != 0 {
                    opened = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            if !opened {
                return false;
            }
            let ok = (|| {
                if EmptyClipboard() == 0 {
                    return false;
                }
                let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
                if handle == 0 {
                    return false;
                }
                let ptr = GlobalLock(handle);
                if ptr.is_null() {
                    GlobalFree(handle);
                    return false;
                }
                std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
                GlobalUnlock(handle);
                // success transfers ownership: the clipboard frees the block
                SetClipboardData(CF_UNICODETEXT, handle) != 0
            })();
            CloseClipboard();
            ok
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = text;
        false
    }
}

/// Open a folder in the system file manager (Windows: `explorer.exe <dir>`)
/// — the settings storage row's "Open folder" affordance. Only the launch
/// is checked; explorer returns odd exit codes by design.
pub fn open_folder(path: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .is_ok()
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_write_and_read_round_trip_unicode() {
        // the write path is FFI now (ADR-0025's second half): CJK survives
        let sample = "中文标题\n\n- item **bold**\nquire://page/7";
        assert!(copy_to_clipboard(sample), "the FFI write must succeed");
        let read = read_clipboard().expect("the FFI read must succeed");
        assert_eq!(read, sample, "the UTF-16 round trip preserves the text");
    }
}
