# M9 Android — pre-research notes (planning only, no code)

Written 2026-09-19 against Slint 1.18. Windows RC comes first (SPEC §二十八);
this page is the homework for the day we start.

## What ports for free

- **`src/core/**`** — the document model, commands, and history are pure
  Rust with no Slint and no I/O. They compile for any target as-is.
- **`src/storage/**`** — rusqlite bundled builds for Android; the DB path
  moves to the app's data dir (jni/ndk-context provides it).
- **The persistence contract** (ADR-0012) is UI-agnostic by design; the
  editor commands (SPEC §十四) carry over unchanged.

## What must be rebuilt

- **All of `ui/`** — the shell assumes a pointer, a 1280×800 window, a
  frameless title bar, and hover affordances. Android needs a
  bottom-bar/touch navigation, larger touch targets (≥44 px), and a
  single-column editor layout.
- **The block renderer's hover model** — BlockHandle menus become
  long-press menus; the find bar moves to a system-style search field.
- **Window size persistence** — replaced by safe-area + fullscreen.

## Known risks (SPEC §二十八)

1. **Slint's Android TextInput** has open issues (history of composition/
   IME gaps). The editor's whole UX rides on it — budget a dedicated
   spike: a minimal IME test app on a real device before committing.
2. **Our own caret model** (`cursor-position-byte-offset` +
   `set-selection-offsets`) is exercised the same way, but composition
   state handling may differ per keyboard (Gboard vs Samsung).
3. **Performance** — FemtoVG on GLES vs the desktop GL path; re-run the
   scene matrix on-device before promising anything.

## Build tooling sketch

- `cargo apk` (android-activity) or a slint-android template; CI needs an
  NDK job. The lib becomes a cdylib; `src/main.rs` splits into a
  platform entry per OS behind a cfg.
- Signing/debugging loop is the slow part — prototype on a physical
  device early.

## Suggested first milestone (M9.0)

A read-only Android viewer: load the SQLite file, list pages, render
blocks (no editing). It proves storage + renderer + build pipeline with
none of the IME risk, and gives the team a device target for the editor
spike that follows.
