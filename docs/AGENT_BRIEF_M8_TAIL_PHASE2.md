# Agent brief — M8 tail, phase 2 (A2 · A5 · A4 · A6)

You are Track B, continuing the package dispatched in
`docs/AGENT_BRIEF_M8_TAIL.md`. Phase 1 is done and merged: A1 (`87ec1dc`,
feedback #13) and A3 (`32f6d48`, release profile audit, pinned as
ADR-0024). Master is at or past `e5c81ff`; suite green (204 pass / 4
ignored by design).

## Ground rules — unchanged from phase 1, restated

- **File ownership.** You own: `src/main.rs`, `benchmarks/**`,
  `docs/PERFORMANCE.md`, `docs/ROADMAP.md`, `install/**`, `.scratch/**`.
  NEVER touch: `src/core/**`, `src/app/**`, `src/services/**`,
  `src/platform/**`, `src/storage/**`, `ui/**`, `tests/**`, `Cargo.toml`,
  `README.md`, `PLAN.md`, `CHANGELOG.md`, `docs/DECISIONS.md`,
  `docs/M8_FEEDBACK.md`, `docs/SPEC.md`. If a task seems to need one of
  those, stop and report instead.
- **Git.** One commit per task on `master`, staging ONLY the task's file
  list (never `git add -A`). No checkout/switch/reset/stash. If
  `.git/index.lock` is held, Track A is committing — wait and retry.
- **Builds.** The target-dir lock may be held by Track A — wait and retry,
  no separate CARGO_TARGET_DIR. Note: Track A compiles often; measure-only
  tasks (A2, A4) should take their measurements in as few windows as
  possible and re-check for pollution before drawing conclusions (your A3
  methodology — it worked).
- **Docs.** `docs/PERFORMANCE.md` and `docs/ROADMAP.md` are yours.
  Everything else (PLAN/CHANGELOG/DECISIONS/M8_FEEDBACK) goes into your
  report; Track A folds it in with ADR numbering.
- **Scratch data** under `.scratch/`, cleaned up. GUI verification runs use
  `--db .scratch/<dir>/quire.db --auto-exit <secs>`. Never touch the real
  per-user library.
- **Report per task:** files (path + what/why), commands + evidence,
  deviations, notes for Track A to fold into the docs. Honest unfinished
  beats silently improvised.

## A5 · ROADMAP refresh — land what you started

`docs/ROADMAP.md` is modified in your working tree. Finish and commit it as
its own commit: status column current against `PLAN.md` (M0–M7 done with
dates; M8 in progress, naming what actually remains: the user IME
acceptance pass, the menu-overflow polish; A1/A3/A5/A6 and Track A's
rounds 9–11 — link-to-page blocks, rich paste, Move page, the settings
storage row — are landed). Keep the table format, "out of scope", and
"Definition of done" sections as they are. Mention the ADR-0024 pin in the
M7/M8 rows if it fits naturally.

## A2 · First-paint startup measurement

The known gap: `startup_ms` measures window-up, not first paint (deferred
since M0/M1). Deliver a programmatic, flag-gated (e.g. `--measure-startup`)
measurement of "first painted frame" latency recorded like the other bench
numbers (JSONL via a script, plus a method + conclusion line in
`docs/PERFORMANCE.md`). Research what Slint 1.18 exposes (render/frame
callbacks — the vendored crates under `~/.cargo/registry/src/` are the
authoritative reference). If public API cannot see first paint, a
documented proxy is acceptable — state the proxy and its bias. Normal runs
(no flag) must pay nothing: no timer, no thread, no extra frame.

**Files:** `src/main.rs`, `benchmarks/**`, `docs/PERFORMANCE.md`. If the
hook needs `src/app/**` or `ui/**` changes, STOP and report.

## A4 · Full visual regression sweep (SPEC §二十一)

Every scene in `apply_scene`/`apply_scene_overlay` (light as declared,
including `dark-*`; the set grew since phase 1 — `page-block`,
`link-block`, `page-move-to`, and the settings dialog now carries a
STORAGE section). Build the software renderer once, render each scene to
`.scratch/sweep/<scene>.png`, and review each against the SPEC §二十一
checklist: spacing/alignment, typography hierarchy, hover/selected/focus
where shown, popup anchoring (no clipped/off-window menus), empty states,
CJK, light-vs-dark consistency.

**Coordination point:** announce (via the user) when you start A4 — Track
A will hold `ui/**` edits until your sweep snapshot is taken. Note the
master commit you swept at.

**Output:** `.scratch/sweep/report.md` (one line per scene, pass/fail;
every fail = what/where + PNG path + severity) and the top defects in your
final message. Do NOT fix anything — the report is the deliverable.

**Files:** `.scratch/**`, `benchmarks/**` (script tweaks only).

## A6 · Installer end-to-end verification (environment-permitting)

D8's installer (`install/quire.iss`, Inno Setup) has never been verified
end to end. If Inno Setup is installed (check
`%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe`): build a release binary, run
ISCC against `install/quire.iss`, install to a scratch prefix (`/DIR=`
with `/SILENT` or the closest supported mode), verify (a) the installed
exe launches with `--db .scratch/... --auto-exit 3` and exits 0, (b) the
taskbar icon shows the embedded resource, (c) the optional `.md`
"Open with" verb lands in the registry when the task is selected, then
uninstall cleanly and confirm the scratch install directory is gone.
Record what you ran and what you saw; if ISCC is absent, report that as an
environment finding and skip — do not install tooling on your own.

**Files:** `install/**` (only if a fix is required and it stays within the
.iss/ps1), scratch dirs. Commit only if you changed something; otherwise
report.

## Sequencing

A5 (finish + commit) → A2 → A4 (announce start via the user) → A6.
Same report format as phase 1; the user relays each report to Track A for
review, doc fold-in, and merge.
