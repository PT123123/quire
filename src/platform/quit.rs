// The instance channel (ADR-0105, ADR-0136): how a launching process finds out
// whether a Quire is already running, which one, and — when the answer says it
// should — asks that one to end its session from outside.
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
// The protocol is one line each way, and the request names the ask:
//
//   * `version\n`  → the server answers `version <v>\n`. This is what a second
//     launch asks first: whether it is about to be a second session at all, and
//     which build it would be a second session of (ADR-0136).
//   * `quit\n`     → the server answers `ok\n` after it has *scheduled* the quit,
//     `err\n` if the schedule failed.
//
// The answer is the point — it lets a caller tell "an instance accepted, it is
// going to exit" from "nothing was listening", and a caller that gets no answer
// must not assume the app is going away.
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

/// How long a replacing launch waits for the session it asked to quit. What it
/// waits for is one final flush and an event-loop unwind — normally well under
/// a second — so the bound only answers an instance that wedged on the way out.
const SESSION_END_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

/// What a launching process decided to do about the instance it found
/// (ADR-0136). The decision is made once, before any session starts, so the
/// answer is one value rather than a set of booleans to misread in pairs.
#[derive(Debug, PartialEq, Eq)]
pub enum SecondLaunch {
    /// Nothing was listening: this process owns the channel and becomes the one
    /// session. The normal path, and the only one that reaches a window.
    Primary,
    /// An instance of the **same** build is running. It keeps running; this
    /// process says so and exits, so two sessions never share one database.
    AlreadyRunning { version: String },
    /// An **older** build is running. Same outcome as [`Self::AlreadyRunning`] —
    /// the running one is not displaced by a downgrade — and the same notice,
    /// which names the running version so the user can see *why* nothing
    /// happened.
    RunningIsNewer { version: String },
    /// A **newer** build is running, so the old one was told to end its own
    /// session and this process carries on to start. `quit_accepted` is false
    /// when the request went out and nothing answered: the newer instance then
    /// keeps running, and this process must not claim to have replaced it.
    ///
    /// `replaced` is the version that was asked to leave, carried so the notice
    /// can name it — a replacement the user was never told about is the one
    /// outcome of this rule that looks like the app lost its window by itself.
    Replacing {
        quit_accepted: bool,
        replaced: String,
    },
}
/// Work out what this launch should do, and act on it.
///
/// This is the whole of the single-instance rule (ADR-0136), and it runs before
/// logging, the database or the window: a second launch that is going to be told
/// to go away must not open a database first, because opening one is a second
/// writer on a file the live instance already has.
///
/// The rule is one comparison of two versions, and the direction is the whole of
/// it:
///
///   * nothing running → `None` (this process owns the channel and goes on to
///     build a window);
///   * same build → tell the user, exit. **Both** halves matter: a second
///     session would be a second writer, and a silent one is how the user finds
///     out.
///   * launching is **newer** → ask the old one to exit through its own quit
///     channel (so its flush and clean-exit record run, ADR-0105) and carry on
///     starting. This is what makes a deploy land on a running install: the old
///     exe releases its files without a kill, which is the property ADR-0105 was
///     built for and what the deploy script has been asking for by hand.
///   * launching is **older** → the running one wins. A downgrade must not
///     displace a newer install; the user is told and this process exits.
///
/// `ours` is this build's version and must be `env!("CARGO_PKG_VERSION")` — the
/// key `build.rs` stamps the exe's version block from, so the number a peer is
/// told is the number the file on disk reports.
///
/// `on_quit` is the caller's, and not this module's: it has to post to whatever
/// loop the caller's window runs on, and this claim is made *before* that window
/// exists. The caller is what knows whether it is up yet — see `main`, which
/// answers `false` for the window between the claim and `ui.run()` rather than
/// posting to a loop that has not started. `announce` is how a non-`Primary`
/// answer reaches the user; it is a callback rather than a print because that
/// half of the rule is a window's job, not this module's.
pub fn claim(
    ours: &str,
    on_quit: impl Fn() -> bool + Send + 'static,
    announce: impl Fn(&SecondLaunch),
) -> Option<SecondLaunch> {
    match imp::request_version(PIPE_NAME) {
        // Nothing is listening. Claim the name and become the one session. The
        // claim is the last thing that can fail, and it is reported rather than
        // assumed: a pipe this process could not own is a pipe a second launch
        // will read a *version* from, which is the one state with no good
        // answer.
        Err(_) => {
            if imp::serve_named(PIPE_NAME, ours, on_quit) {
                None
            } else {
                let blocked = SecondLaunch::AlreadyRunning {
                    version: "unknown".into(),
                };
                announce(&blocked);
                Some(blocked)
            }
        }
        Ok(running) => {
            let decision = if ours == running {
                SecondLaunch::AlreadyRunning { version: running }
            } else if is_newer(ours, &running) {
                // The old instance ends its own session first, and the answer
                // decides what this launch claims. `quit` after `version` is a
                // second exchange on the same pipe, which is why the server
                // re-creates its instance after every request.
                let replaced = running.clone();
                let accepted = imp::request_quit(PIPE_NAME).is_ok();
                if accepted {
                    // The `ok` says the old session *scheduled* its exit — its
                    // final flush and clean-exit record are still in flight
                    // while this process is already starting. Opening the
                    // database here is what races both halves of that handover:
                    // a load that reads before the last flush restores a stale
                    // page, and this session's first open then writes that
                    // stale "current-page" over the real one — so every
                    // relaunch lands one session behind. The wait is bounded,
                    // so an instance that wedges between scheduling its quit
                    // and running it costs the launch the wait and no more.
                    imp::wait_for_session_end(PIPE_NAME, SESSION_END_TIMEOUT);
                }
                SecondLaunch::Replacing {
                    quit_accepted: accepted,
                    replaced,
                }
            } else {
                SecondLaunch::RunningIsNewer { version: running }
            };
            announce(&decision);
            Some(decision)
        }
    }
}
/// Compare two dotted version strings by their numeric parts.
///
/// Not a general semver: a build stamp this app stamps is `MAJOR.MINOR.PATCH`,
/// and that is all this has to tell apart. Anything non-numeric compares as `0`
/// rather than being rejected, so a hand-edited `Cargo.toml` degrades to "the
/// two look the same" (a prompt, which is safe) instead of panicking on a launch.
fn is_newer(ours: &str, theirs: &str) -> bool {
    let parts = |v: &str| -> Vec<u64> {
        v.split(['.', '-'])
            .map(|p| p.parse::<u64>().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parts(ours), parts(theirs));
    // Padding to equal length so 0.1.12 vs 0.1 is decided by the 12 rather than
    // by the missing component sorting as absent.
    let n = a.len().max(b.len());
    (0..n)
        .map(|i| (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0)))
        .find(|(x, y)| x != y)
        .map(|(x, y)| x > y)
        .unwrap_or(false)
}
/// Ask the instance that owns [`PIPE_NAME`] to end its session.
///
/// `Ok(())` means an instance answered `ok` — it has scheduled its own exit
/// through the same route the tray menu uses, so its flush runs and its
/// clean-exit record is written. Every other outcome is a reason the caller has
/// to say out loud, because an unanswered request is not a stopped app.
pub fn request_quit() -> Result<(), String> {
    imp::request_quit(PIPE_NAME)
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

    pub fn serve_named(
        pipe: &str,
        version: &str,
        on_quit: impl Fn() -> bool + Send + 'static,
    ) -> bool {
        let name = wide(pipe);
        let Some(first) = create_instance(&name) else {
            return false;
        };
        // Owned by the thread, and only there: the version is asked for once per
        // request, so a `&str` captured by the closure would not outlive this
        // call. Cloned once, before the spawn, rather than per request.
        let version = version.to_string();
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
                    if BufReader::new(&file).read_line(&mut request).is_ok() {
                        // Two asks, and only two. `version` is what a second launch
                        // sends *first*, before it has decided anything, so the
                        // answer has to be written by a server that is not going to
                        // act on the request — unlike `quit`, which is a decision
                        // and must never be answered by a process that is not the
                        // session.
                        match request.trim() {
                            "quit" => {
                                let accepted = on_quit();
                                let _ = file.write_all(if accepted { b"ok\n" } else { b"err\n" });
                                let _ = file.flush();
                            }
                            "version" => {
                                let _ = file.write_all(format!("version {version}\n").as_bytes());
                                let _ = file.flush();
                            }
                            _ => {}
                        }
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

    /// One exchange with the request named, no timeout of its own — the caller
    /// bounds it. `expect` maps the answer onto `Ok`/`Err`, and the two asks have
    /// two different answers, so it is the closure rather than a shared match
    /// that has to know which ask this was.
    fn exchange(pipe: &str, request: &[u8], expect: impl Fn(&str) -> Result<(), String>) -> Result<(), String> {
        let mut file = open_channel(pipe)?;
        file.write_all(request)
            .map_err(|e| format!("the request could not be written ({e})"))?;
        let _ = file.flush();
        let mut answer = String::new();
        BufReader::new(&file)
            .read_line(&mut answer)
            .map_err(|e| format!("the answer could not be read ({e})"))?;
        let answer = answer.trim();
        if answer.is_empty() {
            return Err("the instance closed the channel without answering".into());
        }
        expect(answer)
    }

    /// `quit` → `ok` is a session that scheduled its own exit; `err` is a live
    /// instance that could not, and the caller must say so rather than act.
    pub fn request_quit(pipe: &str) -> Result<(), String> {
        bounded(pipe, "quit", |answer| match answer {
            "ok" => Ok(()),
            "err" => Err("the instance refused the request: it could not schedule its exit".into()),
            other => Err(format!("the channel answered '{other}', which is not Quire")),
        })
    }

    /// Wait for the session that accepted a quit to actually be gone.
    ///
    /// The signal is the channel itself: a server that answers `version` is an
    /// instance that still runs, and a durable silence is a process whose
    /// handles — the database's among them — are closed. One silence is not
    /// proof on its own (`open_channel` also answers `Err` in the instant the
    /// server spends one instance and creates the next), so the ask is repeated
    /// and only two failures in a row count. The timeout then says the instance
    /// is not going away, and the caller proceeds into the world it used to
    /// start in — the return value says which of the two happened.
    pub fn wait_for_session_end(pipe: &str, timeout: std::time::Duration) -> bool {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if request_version(pipe).is_err() {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if request_version(pipe).is_err() {
                    return true;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        false
    }

    /// `version` → `version <v>`, and the version is this build's. The returned
    /// `Err` is the *only* signal that nothing is listening, which is why the
    /// caller cannot read a version as "an old one answered" — a server that
    /// answered with something else is an `Err` carrying its own text.
    ///
    /// The parsed version is carried back over its own one-shot channel rather
    /// than captured by the answer closure: `expect` is an `Fn` that outlives
    /// this call (it runs on the exchange thread), so it cannot assign to a
    /// variable this frame owns, and borrowing one across that thread is the
    /// bug the `Option` was standing in for.
    pub fn request_version(pipe: &str) -> Result<String, String> {
        let (tx, rx) = std::sync::mpsc::channel();
        bounded(pipe, "version", move |answer| match answer.strip_prefix("version ") {
            Some(v) if !v.is_empty() => tx
                .send(v.to_string())
                .map_err(|_| "the version could not be carried back".to_string()),
            _ => Err(format!("the channel answered '{answer}', which is not Quire")),
        })?;
        rx.recv()
            .map_err(|_| "the instance answered without naming a version".into())
    }

    /// The one place the answer timeout lives, for both asks.
    ///
    /// The exchange runs on a thread of its own because `ReadFile` on a pipe
    /// whose server has gone away blocks for the full OS pipe timeout, and this
    /// bounds the wait with `ANSWER_TIMEOUT` rather than that.
    fn bounded(
        pipe: &str,
        request: &'static str,
        expect: impl Fn(&str) -> Result<(), String> + Send + 'static,
    ) -> Result<(), String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let name = pipe.to_string();
        // The trailing newline is part of the wire format, not of the ask's name:
        // the server reads a *line*, so a request without it never completes the
        // server's `read_line` and the answer that would follow it is never sent.
        let mut bytes = request.as_bytes().to_vec();
        bytes.push(b'\n');
        std::thread::spawn(move || {
            let _ = tx.send(exchange(&name, &bytes, expect));
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
    pub fn serve_named(
        _pipe: &str,
        _version: &str,
        _on_quit: impl Fn() -> bool + Send + 'static,
    ) -> bool {
        false
    }

    pub fn request_quit(_pipe: &str) -> Result<(), String> {
        Err("the quit channel is Windows-only".into())
    }

    pub fn request_version(_pipe: &str) -> Result<String, String> {
        Err("the instance channel is Windows-only".into())
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
            imp::serve_named(&name, "0.1.12", move || {
                hit.fetch_add(1, Ordering::SeqCst);
                true
            }),
            "the first server for this name must get it"
        );
        imp::request_quit(&name).expect("the server must answer ok");
        assert_eq!(asked.load(Ordering::SeqCst), 1, "the server must have run");
        // a quit that failed to schedule leaves the app running, so the channel
        // has to outlive the request that carried it
        imp::request_quit(&name).expect("the channel must serve a second request");
        assert_eq!(asked.load(Ordering::SeqCst), 2);
    }

    /// A version request must be answered **without** running the quit handler,
    /// and must say the build it is running. A server that acted on `version`
    /// would make a second launch end the session it was only trying to
    /// identify — the single-instance rule's first question killing the app it
    /// was asked about.
    #[test]
    fn a_version_request_is_answered_without_ending_the_session() {
        let asked = Arc::new(AtomicUsize::new(0));
        let hit = asked.clone();
        let name = format!(r"\\.\pipe\quire-desktop-quit-test-ver-{}", std::process::id());
        assert!(imp::serve_named(&name, "0.1.12", move || {
            hit.fetch_add(1, Ordering::SeqCst);
            true
        }));
        assert_eq!(imp::request_version(&name).unwrap(), "0.1.12");
        assert_eq!(
            asked.load(Ordering::SeqCst),
            0,
            "asking a version must not schedule the quit"
        );
        // and the channel is still there afterwards, for the `quit` that follows
        // a newer build asking the old one to leave
        imp::request_quit(&name).expect("the channel must survive a version ask");
        assert_eq!(asked.load(Ordering::SeqCst), 1);
    }

    /// A refusal is told apart from an answer, because a caller acting on
    /// "stopped" that was really "still running" locks the file it overwrites.
    #[test]
    fn a_refused_request_is_not_an_answer() {
        let name = format!(
            r"\\.\pipe\quire-desktop-quit-test-refuse-{}",
            std::process::id()
        );
        assert!(imp::serve_named(&name, "0.1.12", || false));
        let err = imp::request_quit(&name).expect_err("a refused request is an error");
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
        let err = imp::request_quit(&name).expect_err("nothing is listening");
        assert!(err.contains("no instance answered"), "{err}");
    }

    /// The three-way comparison the single-instance rule is made of. Pinned as a
    /// table because the direction is the whole rule and each direction is a
    /// different user-visible outcome: a prompt, a replacement, or a refusal.
    #[test]
    fn the_version_comparison_decides_each_direction() {
        // same → not newer, and `claim` short-circuits on string equality first
        assert!(!is_newer("0.1.12", "0.1.12"));
        // launching newer → replace
        assert!(is_newer("0.1.13", "0.1.12"));
        assert!(is_newer("0.2.0", "0.1.99"));
        assert!(is_newer("1.0.0", "0.9.9"));
        // launching older → the running one wins
        assert!(!is_newer("0.1.12", "0.1.13"));
        assert!(!is_newer("0.1.9", "0.1.12"));
        // a missing component compares as 0 rather than as absent, so 0.1.12 is
        // newer than 0.1 — which is what a trailing `.0` means
        assert!(is_newer("0.1.12", "0.1"));
        assert!(!is_newer("0.1", "0.1.12"));
        // a non-numeric part degrades to 0 instead of panicking on a launch
        assert!(!is_newer("0.1.x", "0.1.0"));
    }
}
