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
/// PowerShell's `Get-Clipboard -Raw` is the same zero-dependency route
/// `copy_to_clipboard` takes with `clip.exe`; UTF-8 output encoding keeps
/// CJK text intact across the console codepage. The subprocess costs
/// ~100-300 ms and blocks the caller — acceptable for a user-initiated
/// paste, and the reason this is not used anywhere hot. Other targets
/// report absence instead of pretending.
pub fn read_clipboard() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[Console]::OutputEncoding=[Text.Encoding]::UTF8; Get-Clipboard -Raw",
            ])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let text = String::from_utf8(out.stdout).ok()?;
        let text = text.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            None
        } else {
            Some(text.to_string())
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
