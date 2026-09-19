# PLAN — living status tracker (update after every milestone)

## M0 · Toolchain & baseline — ✅ (2026-09-18)
- [x] slint-rust-template as starting point; project renamed to `quire`
- [x] Slint 1.18.0 (current stable), MSVC, Debug + Release profiles
- [x] Renderer feature matrix in Cargo.toml (femtovg / femtovg-wgpu / skia / skia-opengl / software)
- [x] Renderer benchmark harness: `benchmarks/scripts/bench.ps1`
- [x] Baseline numbers recorded in docs/PERFORMANCE.md
- [x] Slint LSP config (.vscode)（CI workflow 已按要求移除，本地手动构建）
- [x] Directory structure + ARCHITECTURE.md / DECISIONS.md / PLAN.md / PERFORMANCE.md

## M1 · Design system & product-grade shell — ✅ (2026-09-18)
- [x] Theme.slint: spacing scale, radii, motion tokens, dark flag
- [x] Colors.slint: full light/dark palette (12+ semantic tokens)
- [x] Typography.slint: UI + document type scale, mono stack
- [x] Icons.slint: original vector path icons, theme-aware stroke color
- [x] AppShell / TopBar (custom title bar + window controls) / Sidebar /
      PageTree / Editor placeholder (block-styled mock document) /
      CommandPalette mock / Button / IconButton / BlockHandle
- [x] Mock data served through Slint models from Rust (ADR-0005)
- [x] Light/Dark toggle; one visual polish pass done
- [x] Release idle CPU/RAM re-measured with shell open (see PERFORMANCE.md)

## M0/M1 completion report (2026-09-18)
Delivered:
- Compiling Rust+Slint 1.18 workspace (`quire`), debug + 3 release renderer
  builds (FemtoVG·GL / FemtoVG·wgpu / Skia), all verified rendering real
  pixels via screenshots (light + dark + palette-open, Chinese text OK).
- Full M1 shell: frameless window, custom title bar, sidebar with pinned
  rows + page tree, mock document (headings/paragraphs/lists/todo/quote/
  code/divider), command palette mock with live filter plumbing to Rust.
- Idle: ≈0 % CPU (≤0.4 % of one core), 85–212 MB private by renderer;
  10 000-block page costs ≈3 MB over the empty shell (virtualization works).
- Docs: ARCHITECTURE / DECISIONS (ADR-0001…0008) / PERFORMANCE (baseline
  table + method + scene list) / this PLAN.

Compiler landmines found in Slint 1.18.0 (workarounds in ADR-0007/0008):
- reading a PopupWindow's own `is-open` panics the compiler;
- ListView accepts exactly one `for` child.

Known gaps carried into M2 (not blockers):
- palette keyboard/mouse interaction verified by construction + static
  screenshots, not by scripted key injection (OS foreground policy blocks
  headless SendKeys on this desktop);
- Scene F (continuous scroll) still a manual pass;
- window "startup_ms" measures window-up, not first paint.

## M2 · App shell navigation on mock pages — ✅ (2026-09-19)
- [x] `app/workspace.rs`: pure page-tree model (create/rename/duplicate/
      delete, favorites, recents with cap, expand-ancestors, search over
      title + content blob) — unit-tested without Slint
- [x] Live sidebar: Favorites / Recent / Workspace tree projection,
      collapse/expand round-trip, selection + fallback landing page,
      "+ New page" row
- [x] Page switching: per-page mock content, TopBar breadcrumb
      (Workspace › parent › page), Todo toggle kept
- [x] Context menu (right-click on tree rows, ⋯ on TopBar): new subpage,
      rename (inline), duplicate (deep copy), favorite toggle, delete
      with confirmation dialog (subtree-size aware)
- [x] Search panel Ctrl+P: titles + content, keyboard navigation,
      recents as the empty-query view; palette (Ctrl+K) = commands only
- [x] Settings dialog (appearance + about), delete confirm dialog,
      empty-page state, Ctrl+N new page flow
- [x] lib/bin split: `src/lib.rs` exposes the crate for `tests/`
      integration tests (6) + unit tests (7) — all green
- [x] `quire-shot` headless visual-regression tool (ADR-0011) +
      `just shot <scene>`; 9 M2 scenes rendered and reviewed (9/9 pass)
- [x] Benchmarks: scenes F + G measured for the first time
      (PERFORMANCE.md), scenes A/D re-measured on the M2 build

Delivered in the same pass (former known-gaps from M0/M1):
- Scene F now has a programmatic proxy measurement; scene G delivered
  with M2 as planned. Palette interaction verified by real renders of
  each interactive state (headless), not just static construction.
- Stack-overflow on startup found & fixed (ADR-0009): Slint 1.18's
  recursive binding evaluation exceeded the 1 MB Windows default with
  the full M2 shell; the event loop now runs on an 8 MB-stack thread.
  Also fixed by the visual review pass: page-title descender collision,
  TopBar child-geometry (Slint centers width-only children), zero-length
  icon path segments invisible in the software renderer.

Deferred (not blockers, tracked for M3+):
- window `startup_ms` still measures window-up, not first paint;
- scene F remains a programmatic proxy until wheel-input injection is
  available; real input pass stays manual.

## M3 · Local documents (SQLite) — ✅ (2026-09-19, branch `m3-storage`)
- [x] `storage/`: rusqlite (bundled) + six-table schema per SPEC §十八 in
      `migrations.rs` (`PRAGMA user_version`, forward-only upgrades),
      `database.rs` (WAL + synchronous=FULL + startup `integrity_check`,
      SPEC §二十五), `repository.rs` implementing the `core::Repository`
      contract — `apply` one transaction per change list (SPEC §十八),
      cascade + recursive-CTE subtree deletes, `replace_all` with deferred
      FK checks and an explicit parent-cycle guard
- [x] `services/persistence.rs`: dirty queue → 300 ms debounce (injectable
      `Clock`) → one batched `apply`; `force_flush` for Ctrl+S/shutdown;
      failed writes keep the queue ordered and retry at the next deadline
      (SPEC §十九). Slint- and thread-free: the app layer's timer calls
      `flush_if_due`, any thread may call `force_flush`
- [x] Tests: 22 lib unit + 13 storage integration + 3 persistence
      integration — round-trip (incl. OrderKey extremes and CJK text),
      migration of a v0 file, rollback of a failing batch, corrupt-file and
      unknown-kind reporting, fake-clock debounce behavior, and a
      subprocess kill -9 crash test (committed rows survive, an
      in-flight transaction never surfaces)
- [x] `cargo check --all-targets` + `cargo test` green on this branch;
      save-latency numbers recorded in docs/PERFORMANCE.md; schema
      rationale in ADR-0013

Not in this pass (per track split): controller/UI wiring of
`PersistenceService` and retiring `app/workspace.rs` mocks — Track A
after the M3/M4 merge. DB file location/first-run bootstrap is part of
that wiring decision, not the storage layer.

## M3 integration wiring — ✅ (2026-09-19, Track A, `de43e95`)
- [x] main.rs opens `appdata/quire.db` (migrate + integrity check);
      `--db` override; failed open = memory-only session (never writes
      over an unreadable database); final flush on window close
- [x] fresh DB seeds the session once (PageCreated + BlockInserted batch
      flushed immediately); non-empty DB rebuilds workspace tree + Document
      from PersistedState
- [x] every mutation records contract Changes: page create/rename/
      duplicate/delete/favorite/expand, editor commands, undo/redo;
      600 ms quiet-period flush timer + Ctrl+S
- [x] verified end to end: seed run (14 pages / 24 blocks) -> restart
      loads identical state (`--dump-state`)
- known drift: a duplicated page may sort last among siblings after a
  restart (gap-exhaustion fallback appends); session view keeps it
  adjacent. Bench `--blocks` overrides the loaded page in memory only.
- still mock: recent list is session-local; settings (theme) persistence
  lands with the settings UI work (M8); M4 editor polish continues
  (block-type switching menu in M5, IME acceptance = user pass)

## M4/M5/M6 progress — Track A (2026-09-19, `cda0cd8`+)
- [x] M5 slash menu: Rust-owned descriptors, filter-as-you-type, keyboard
      first (SPEC §十五); applying type+text cleanup is ONE undo step
- [x] M5 block handle menu (+ / ⋮⋮): move up/down, duplicate, copy block,
      paste below, delete; cross-block clipboard (menu-gated)
- [x] M6 inline marks: Mark/MarkKind in the contract (BlockMarksSet),
      ToggleMark command (add/remove/replace semantics), marks table in
      schema v2, Ctrl+B/I/E/Shift+X over the selection; runs render with a
      documented wrap limitation (Slint Text has no inline formatting)
- [x] settings persistence: theme + recents survive restart (settings/
      metadata tables); palette gained Rename/Duplicate/Delete Page
- [x] platform notes: PopupWindow cannot be conditional/repeated → one
      instance per row with changed-driven show; delegate-inline menus get
      painted over by following ListView rows (window-level popups win)
- [ ] user pass: Chinese IME acceptance; 1000-block typing check lands
      with Track B's scene E; drag-to-reorder (mouse) deferred to M7 polish

## M5/M6/M7/M8 wiring — Track A round 2 (2026-09-19)
- [x] merged Track B m8-markdown into m3-storage: schema v2 (FTS search
      index) + v3 (inline marks) coexist; CURRENT_VERSION = 3
- [x] search panel switched to the FTS SearchService (ranked, Chinese-
      capable via segmentation); blob scan remains the no-DB fallback
- [x] import/export wired: palette "Export Page as Markdown…" /
      "Import Markdown…" with native file dialogs (rfd); import creates a
      page from the file and opens it
- [x] M6 link UI: Ctrl+L over a selection opens an Add-link dialog
      (URL input, Apply/Remove); Link marks render accent-colored
- [x] settings persistence: theme + recents survive restart; palette
      gained Rename/Duplicate/Delete Page; Ctrl+Shift+Up/Down moves the
      focused block
- [x] judge review of slash / block-menu / marks / link scenes: 4/4 pass

## Track A round 3 (2026-09-19)
- [x] async FTS search in the GUI: query goes to the worker thread, the
      controller polls on a re-arming 30 ms timer, generation counter drops
      superseded results; headless scenes use a blocking sync path
- [x] link runs open in the browser (shell open, cfg-gated)
- [x] restore notice banner (OpenReport consumption, M8 feedback #4) +
      load-corrupt notice path (feedback #5 UI half)
- [x] mouse-drag reorder evaluated and dropped (PointerEvent carries no
      position; TouchArea coords move with the swapped block) — menu +
      Ctrl+Shift+arrows are the reorder UX
- [x] IME manual checklist (docs/IME_CHECKLIST.md) — USER PASS PENDING
- [x] judge: 14-scene regression sweep 14/14 pass
- [ ] final build/test/render verification rides on Track B's in-flight
      D6 export/import mark parsing (services/export_service.rs WIP)

## Track A round 4 (2026-09-19)
- [x] dark-mode QA of every new surface (slash / block menu / marks /
      find bar / link dialog / title edit): combined dark scenes added,
      all render correctly on dark surfaces
- [x] find-as-you-type (session rebuilds per keystroke)
- [x] README screenshots (docs/screenshots, 6 shots) + window size
      persistence (settings table, restore before first paint, save at
      close) + version footer cleanup
- [x] M8_FEEDBACK all 8 items triaged: 4 resolved, 2 accepted with
      rationale, 2 partially resolved pending Track B's D5-D6 landing
      (now landed and merged)

### D11 · M7 performance matrix ✅ (Track B, 2026-09-19)
- [x] full matrix recorded (benchmarks/results/*.jsonl): femtovg + skia,
      scenes A–G incl. typing at 1k/10k; bench.ps1 gained -PinnedDb
      (idle scenes pinned to a temp DB) and legal JSON output; new
      bench_matrix.ps1 driver
- [x] conclusions page in docs/PERFORMANCE.md (per-scene comparison +
      SPEC §六 checklist + ranked M7 follow-ups)

### D12 · Data location migration ✅ (Track B + Track A wiring, 2026-09-19)
- [x] storage/data_location.rs: Placement (Fixed / Portable / Roaming),
      argv scan, per-user migration carrying db + snapshots + log family
- [x] open_with_report resolves through it (idempotent); logging::data_dir
      follows the database
- [x] Track A wiring: #12 periodic snapshots powered
      (with_database_snapshots on the flush tick, 10-min floor); #13
      main.rs create_dir_all removed (storage owns its dirs); startup
      notice bar reports the library move and backup restores
- [x] verified: full suite green post-merge; LAN smoke re-run on the
      merged tree

## Next: M4 · Block editor MVP (remaining)
- block-type switching menu (BlockHandle affordance), cross-block
  clipboard, 1000-block editing responsiveness check, Chinese IME
  acceptance pass (user), editor visual scenes re-shot.

## Later (unchanged from brief)
M4 block editor MVP (IME = test item, ADR-0002) → M5 slash menu + command
palette real wiring → M6 rich text → M7 virtualization/performance →
M8 Windows RC (packaging, crash recovery, import/export) → M9 Android
(separate IME/editor test plan).

## Explicitly out of scope for v1
Sync, collaboration, cloud, plugin market, AI, multi-process IPC,
custom TSF/IME implementation, image/table/toggle/database blocks.

## M8 hardening · Track B (2026-09-19, branch `m8-hardening`)

Based on `master` at `f2eb855` (after M8 D5–D8 and the merged markdown pair).
Four deliverables, one commit each: D9 logging, D10 backup retention, D11 the
M7 performance matrix, D12 the data-location move.

### D9 · log infrastructure ✅
- `services/logging.rs`: `quire.log` beside the database, family of three
  (`quire.log`, `.1`, `.2`) at ≤1 MB each, one physical line per record with a
  UTC stamp, `Level::{Info,Warn,Error,Panic}`.
- `init()` is the only thing `main.rs` gained (one line). It creates the
  directory, writes the startup record and installs a panic hook that chains the
  previous one, so stderr keeps the usual message.
- A panic appends `[panic] …` and writes `panic-report.txt`; the next `start()`
  turns it into the `last_session_aborted` entry of `session.meta`, logs it once,
  and deletes the report. `meta_entries()` is the seam for Track A to carry it
  into the `metadata` table (M8_FEEDBACK #9).
- Tests (9, in-module because `Cargo.toml` is out of reach — feedback #11):
  rotation caps the family at three files and drops the oldest, generations
  shift the right way, a panic → abort-metadata → consumed-once chain, a clean
  start reports no abort, metadata escaping across a reopen, the hook firing
  through a real `catch_unwind` panic, and UTC stamp arithmetic.
- Checked end to end: a scratch-directory run writes
  `2026-09-19T08:21:55.169Z [info] session started (pid 67564)` to
  `appdata/quire.log` and exits 0.
- Known limit (ADR-0018): a kill or a native crash leaves no report, so
  `last_session_aborted` means "panicked", not "ended badly".

### D10 · backup retention + periodic snapshot ✅
- `storage/backup.rs`: `KEEP` 3 → 5 and a second window, `MAX_AGE` = 7 days.
  New `prune(path, now)` applies both (plus two slots past `KEEP`, so a family
  left by a larger setting cannot linger) and `snapshot()` runs it after a
  successful copy, ignoring its error — cleanup never turns a good snapshot into
  a reported failure. `recover` now walks five generations.
- `services/persistence.rs`: `with_snapshotter(interval_ms, hook)` and the app's
  shortcut `with_database_snapshots(&repo)`, default
  `DEFAULT_SNAPSHOT_INTERVAL_MS` = 10 min. The check runs at the end of
  `flush_if_due` / `force_flush`, so no thread and no second timer; it fires only
  when the period elapsed *and* something was written since the last snapshot.
  A failing snapshot goes to `take_snapshot_error()` + `logging::warn`, never into
  the flush's `Result`, and the period restarts either way (a dead disk is not
  retried per tick).
- `storage/repository.rs`: `SqliteRepository` remembers the path it opened
  (`path()`) and gained `snapshot()`, which takes the same connection mutex every
  write takes.
- Tests: 6 new in `services::persistence` (fake clock — period *and* write gate,
  period restarts from the snapshot not the write, `force_flush` carries it, an
  idle session takes none, a failing hook leaves the data write intact, no hook =
  old behavior); 3 new + 1 generalised in `tests/integration/backup_test.rs`
  (rotation to five generations, age window via `File::set_times`, oversized
  family, retention from inside a real open); 1 new in `persistence_test.rs`
  against a real SQLite file, asserting `.bak1` holds the mid-session edit and
  the startup copy moved to `.bak2`.
- Landed as two commits, because the parallel track's `git add` swept
  `src/storage/backup.rs` into `67b23f1` while D10 was unfinished; the retention
  half is there and the snapshot half is here.
- Wiring owed (M8_FEEDBACK #12): one line in `app/state.rs` where
  `PersistenceService::with_default_clock` is built. Until then the periodic
  snapshot is tested but not armed, exactly like the startup one was before.
- Known reading (ADR-0019): ten minutes is a *minimum* gap on a timer the app
  re-arms per burst, so an idle window takes no snapshot — deliberate, since
  there is nothing new to protect.

