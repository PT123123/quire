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
