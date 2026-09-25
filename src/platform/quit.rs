// The quit channel (ADR-0105): how a *running* instance is asked to end its
// session from outside.
//
// Why it exists. Closing the window only hides to the tray (ADR-0096), so the
// one graceful exit the app has is the tray menu's 「退出」 — and a deploy that
// wants to replace the exe cannot click a menu. Before this, replacing a
// running build meant killing it, which skips the final flush and writes no
// clean-exit record, so the next start reported a crash that never happened
// (ADR-0018 reads the missing record exactly that way).
//
// The mechanism is a byte-mode named pipe, and it is deliberately the smallest
// one that works:
//
//   * The server is created with FILE_FLAG_FIRST_PIPE_INSTANCE, which is the
//     whole ownership rule — the first process to name the pipe gets it, and a
//     second one fails to create it instead of silently becoming another
//     instance of the same name. No mutex, no PID file.
//   * The client is std only. A named pipe is openable as a file, so
//     `OpenOptions::open(r"\\.\pipe\quire-desktop-quit")` *is* CreateFileW.
//   * Two hand-declared `extern "system"` functions and no `windows` crate —
//     the same rule `platform::mod` states for the clipboard. kernel32 is
//     already linked.
//
// The protocol is one line each way: the client writes `quit\n`, and the server
// answers `ok\n` after it has *scheduled* the quit, `err\n` if the schedule
// failed. The answer is the point — it lets a caller tell "an instance
// accepted, it is going to exit" from "nothing was listening", and a caller
// that gets no answer must not assume the app is going away.
//
// `quire.exe --quit` is the client half and runs before any session starts: no
// log line, no database, no window, no event loop.

use std::io::{BufRead, BufReader, Write};

/// The pipe the running instance owns. One name, because there is one session
/// to end.
pub const PIPE_NAME: &str = r"\\.\pipe\quire-desktop-quit";

/// How long the client waits for the answer. The server replies as soon as it
/// has read the request, so this only bounds a *wedged* instance — without it a
/// deploy would hang on one instead of reporting it.
const ANSWER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Start answering quit requests, and return whether this process got the
/// channel.
///
/// This returns as soon as the name is claimed: the listening itself happens on
/// a thread of its own, because `ConnectNamedPipe` blocks until a client
/// arrives and the caller still has a window to build. `false` means another
/// instance already owns the name, which is also the only single-instance
/// signal this app has: the second launch keeps running (there is no guard —
/// DECISIONS.md records that), it just cannot be reached by `--quit`.
///
/// `on_quit` returns whether the session's exit was actually scheduled, and
/// that is what the answer carries.
pub fn serve(on_quit: impl Fn() -> bool + Send + 'static) -> bool {
    imp::serve_named(PIPE_NAME, on_quit)
}

/// Ask the instance that owns [`PIPE_NAME`] to end its session.
///
/// `Ok(())` means an instance answered `ok` — it has scheduled its own exit
/// through the same route the tray menu uses, so its flush runs and its
/// clean-exit record is written. Every other outcome is a reason the caller has
/// to say out loud, because an unanswered request is not a stopped app.
pub fn request_quit() -> Result<(), String> {
    imp::request_named(PIPE_NAME)
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{BufRead, BufReader, Write};
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};

    // CreateNamedPipeW / ConnectNamedPipe
    const INVALID_HANDLE_VALUE: isize = -1;
    const PIPE_ACCESS_DUPLEX: u32 = 0x0000_0003;
    /// Refuse the name if it is already taken (see the module comment).
    const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
    // byte mode, byte reads, blocking — all three are the value 0, spelled out
    // so the argument order below reads as the documented signature
    const PIPE_TYPE_BYTE: u32 = 0x0000_0000;
    const PIPE_READMODE_BYTE: u32 = 0x0000_0000;
    const PIPE_WAIT: u32 = 0x0000_0000;
    /// `ConnectNamedPipe`'s "a client connected first" result, which is a
    /// connection and not an error.
    const ERROR_PIPE_CONNECTED: i32 = 535;
    /// `CreateFileW` on a name whose only instance is in use.
    const ERROR_PIPE_BUSY: i32 = 231;
    const ERROR_FILE_NOT_FOUND: i32 = 2;

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateNamedPipeW(
            name: *const u16,
            open_mode: u32,
            pipe_mode: u32,
            max_instances: u32,
            out_buffer: u32,
            in_buffer: u32,
            timeout: u32,
            security: *mut std::ffi::c_void,
        ) -> isize;
        fn ConnectNamedPipe(handle: isize, overlapped: *mut std::ffi::c_void) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    /// Claim the name and hand back a fresh, *unconnected* instance.
    ///
    /// Claiming happens here and not at the first connect, because
    /// `ConnectNamedPipe` blocks until a client arrives: a `serve` that waited
    /// for one would never return, and its caller has a window to build.
    fn create_instance(name: &[u16]) -> Option<std::fs::File> {
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                // FILE_FLAG_FIRST_PIPE_INSTANCE refuses any other count
                1,
                0,
                0,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }
        // the handle is owned from here, so an early return cannot leak it
        Some(unsafe { std::fs::File::from_raw_handle(handle as RawHandle) })
    }

    /// Block until a client is on this instance. A fresh instance is
    /// disconnected, so this is where a request starts being waited for, not
    /// where the name was claimed.
    fn connect(file: &std::fs::File) -> bool {
        (unsafe { ConnectNamedPipe(file.as_raw_handle() as isize, std::ptr::null_mut()) }) != 0
            || std::io::Error::last_os_error().raw_os_error() == Some(ERROR_PIPE_CONNECTED)
    }

    pub fn serve_named(pipe: &str, on_quit: impl Fn() -> bool + Send + 'static) -> bool {
        let name = wide(pipe);
        let Some(first) = create_instance(&name) else {
            return false;
        };
        // The name is claimed above, so a client arriving the instant this
        // returns already finds it. If the thread cannot be spawned the
        // instance is dropped instead and the name goes with it.
        std::thread::Builder::new()
            .name("quit-channel".into())
            .spawn(move || {
                let mut file = first;
                loop {
                    if !connect(&file) {
                        break;
                    }
                    let mut request = String::new();
                    if BufReader::new(&file).read_line(&mut request).is_ok()
                        && request.trim() == "quit"
                    {
                        let accepted = on_quit();
                        let _ = file.write_all(if accepted { b"ok\n" } else { b"err\n" });
                        let _ = file.flush();
                    }
                    // The answer is out and this instance is done. Dropping it
                    // closes the handle, and a close delivers what the client
                    // has not read yet — which is why this is a close and not
                    // `DisconnectNamedPipe`, which can drop the answer.
                    drop(file);
                    // A quit that failed to schedule leaves the app running, so
                    // the channel has to outlive the request that carried it.
                    let Some(next) = create_instance(&name) else {
                        break;
                    };
                    file = next;
                }
            })
            .is_ok()
    }

    /// Connect to the channel, retrying while the server is between instances.
    ///
    /// The server closes its instance and creates the next one, so for an
    /// instant the name is `ERROR_PIPE_BUSY` (it is connected but spent) or
    /// `ERROR_FILE_NOT_FOUND`. Both are "ask again" and both clear in
    /// microseconds, but a caller cannot tell either from "there is no
    /// instance" — so these retries are also what makes the genuinely-absent
    /// case take about half a second instead of failing on a coin flip.
    fn open_channel(pipe: &str) -> Result<std::fs::File, String> {
        let mut last: Option<std::io::Error> = None;
        for _ in 0..5 {
            match std::fs::OpenOptions::new().read(true).write(true).open(pipe) {
                Ok(file) => return Ok(file),
                Err(e) => {
                    let code = e.raw_os_error();
                    if code != Some(ERROR_PIPE_BUSY) && code != Some(ERROR_FILE_NOT_FOUND) {
                        return Err(format!("no instance answered {pipe} ({e})"));
                    }
                    last = Some(e);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Err(match last {
            Some(e) => format!("no instance answered {pipe} ({e})"),
            None => format!("no instance answered {pipe}"),
        })
    }

    /// One exchange, no timeout of its own — `request_named` bounds it.
    fn exchange(pipe: &str) -> Result<(), String> {
        let mut file = open_channel(pipe)?;
        file.write_all(b"quit\n")
            .map_err(|e| format!("the quit request could not be written ({e})"))?;
        let _ = file.flush();
        let mut answer = String::new();
        BufReader::new(&file)
            .read_line(&mut answer)
            .map_err(|e| format!("the answer could not be read ({e})"))?;
        match answer.trim() {
            "ok" => Ok(()),
            "err" => Err("the instance refused the request: it could not schedule its exit".into()),
            "" => Err("the instance closed the channel without answering".into()),
            other => Err(format!("the channel answered '{other}', which is not Quire")),
        }
    }

    pub fn request_named(pipe: &str) -> Result<(), String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let name = pipe.to_string();
        std::thread::spawn(move || {
            let _ = tx.send(exchange(&name));
        });
        match rx.recv_timeout(super::ANSWER_TIMEOUT) {
            Ok(result) => result,
            Err(_) => Err(format!(
                "the instance did not answer within {:?}",
                super::ANSWER_TIMEOUT
            )),
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    /// No channel off Windows: the deploy that needs it is a Windows one, and
    /// reporting failure is what `platform::mod` does everywhere else rather
    /// than pretending.
    pub fn serve_named(_pipe: &str, _on_quit: impl Fn() -> bool + Send + 'static) -> bool {
        false
    }

    pub fn request_named(_pipe: &str) -> Result<(), String> {
        Err("the quit channel is Windows-only".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// The whole round trip in one process, which is the only way to test two
    /// halves of one pipe: a request reaches the server, the server's answer
    /// reaches the client, and the channel survives to serve again.
    ///
    /// The name is this test's own, not [`PIPE_NAME`] — a real Quire running on
    /// the machine owns that one, and the first-instance rule would then make
    /// this fail for the wrong reason.
    ///
    /// The regression this pins first: `serve` used to do its first
    /// `ConnectNamedPipe` inline, which blocks until a client shows up. That
    /// made `serve` itself the deadlock — it never returned, so the client
    /// below could never be called, and a caller with a window still to build
    /// would have hung before drawing anything.
    #[test]
    fn a_request_reaches_the_server_and_its_answer_comes_back() {
        let asked = Arc::new(AtomicUsize::new(0));
        let hit = asked.clone();
        let name = format!(r"\\.\pipe\quire-desktop-quit-test-{}", std::process::id());
        assert!(
            imp::serve_named(&name, move || {
                hit.fetch_add(1, Ordering::SeqCst);
                true
            }),
            "the first server for this name must get it"
        );
        imp::request_named(&name).expect("the server must answer ok");
        assert_eq!(asked.load(Ordering::SeqCst), 1, "the server must have run");
        // a quit that failed to schedule leaves the app running, so the channel
        // has to outlive the request that carried it
        imp::request_named(&name).expect("the channel must serve a second request");
        assert_eq!(asked.load(Ordering::SeqCst), 2);
    }

    /// A refusal is told apart from an answer, because a caller acting on
    /// "stopped" that was really "still running" locks the file it overwrites.
    #[test]
    fn a_refused_request_is_not_an_answer() {
        let name = format!(
            r"\\.\pipe\quire-desktop-quit-test-refuse-{}",
            std::process::id()
        );
        assert!(imp::serve_named(&name, || false));
        let err = imp::request_named(&name).expect_err("a refused request is an error");
        assert!(err.contains("refused"), "the reason must say so: {err}");
    }

    /// Nothing listening is an error, not a silent success: the deploy calls
    /// this to decide whether it may overwrite the exe.
    #[test]
    fn no_server_is_reported_as_such() {
        let name = format!(
            r"\\.\pipe\quire-desktop-quit-test-absent-{}",
            std::process::id()
        );
        let err = imp::request_named(&name).expect_err("nothing is listening");
        assert!(err.contains("no instance answered"), "{err}");
    }
}
