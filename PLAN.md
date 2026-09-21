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

## Track A round 5 — ⋮⋮ menu completion + Callout (2026-09-19, `m8-hardening`)
- [x] block menu = Notion's set minus collab/AI: Copy link to block
      (`quire://block/<id>` via clip.exe), Move to (cross-page subtree
      move, one undo step), Text/Background color (swatch palettes with a
      live current-pick check); quire://block & quire://page links resolve
      in-app (page jump + block focus)
- [x] Callout block (kind 10): tinted box + emoji + text; slash menu /
      Turn-into gain it; exports as a quote
- [x] block color in the contract: ColorKind pair on Block, SetBlockColor
      command, BlockColorSet change, schema v4 (conditional column add),
      theme-aware palette in Colors.slint, rendering across text kinds +
      the editing input
- [x] menu popup anchor clamp now follows the live row count; MenuRow
      gained swatch/check fields; link + arrow-right + palette icons
- [x] tests: move-to-page subtree/undo, color undo, colors+moves storage
      round-trip; suite green; scenes block-menu / move-to / text-color /
      bg-color / block-colors / dark-block-colors rendered and reviewed
- [x] markdown-shortcut curation (ADR-0022) re-verified: H1-3 / bullet /
      numbered / to-do / quote stay out of both menus
- deferred (user-visible gaps, next rounds): Page & Link-to-page blocks
  (need a child-page column + lifecycle), Table/Toggle (v1 exclusions),
  Comment/Suggest edits/Ask AI (collab/AI stay out of scope)

## Track A round 6 — popup dismissal + the "+" insert menu (2026-09-19, `m8-hardening`)

User-reported: none of the three menus (sidebar page menu, block ⋮⋮ menu,
slash menu) close when clicking outside, and the "+" handle only inserted
an empty line instead of offering block choices.

- [x] Root cause: Slint 1.18 dispatches *no event into the window content*
      while a popup is open — clicks outside a `no-auto-close` popup are
      swallowed by the engine (i-slint-core `window.rs` dispatch loop), so
      the AppShell scrim never fired and the old comment about it was
      wrong. All popups were `no-auto-close`.
- [x] Menus switch to `close-on-click-outside` (outside click AND Escape):
      sidebar/TopBar context menu, slash popup, block ⋮⋮ popup, command
      palette, search panel. Dialogs (link, confirm, settings) keep
      `no-auto-close` deliberately — they are modals with explicit
      buttons, and outside clicks stay inert.
- [x] State resync: the engine flips the popup's `is-open` on auto-dismiss;
      AppWindow mirrors each popup's `is-open` parent-side (reading a
      popup's own `is-open` is the 1.18 const-prop crash) and folds it
      back into UIState (`menu-open`, `slash-open`, `block-menu-open-id`,
      `palette-open`, `search-open`). Without this the AppShell scrim
      would stay enabled and the window would look frozen.
- [x] The closing press consumes the click (engine semantics), so a grip
      toggle can't re-open; side effect, documented in CHANGELOG: moving
      directly from one open menu to another costs two clicks.
- [x] "+" handle = Notion's insert menu: `block-plus(id, row-bottom, x)`
      inserts the empty paragraph below the clicked block, focuses it, and
      opens the slash popup in a new insert mode (slash-insert) anchored
      below that line. INSERT_ITEMS (state.rs) carries the full list; the
      curated "/" menu is untouched (ADR-0022 holds there).
- [x] Insert mode: typing filters on the whole line (no "/" prefix), the
      anchor stays put, applying discards the typed filter text, and every
      close path clears the mode in one place (`changed slash-open`).
      Markdown shortcuts still win over the menu, matching the typing flow.
- [x] Notion-parity placeholders: Page, Toggle list, and the database views
      (Table, Board, Gallery, List, Calendar, Timeline) render muted with
      "· later" hints, are skipped by arrow-key navigation, and applying to
      one is a no-op — v1 exclusions stay visible without pretending to
      work (PLAN "out of scope" list unchanged).
- [x] quire-shot grew behavioral probes: `--click x,y` / `--key escape`
      dispatch real input through the engine and print before/after popup
      flags; `--probe-blocks` prints the id:kind projection; scene "plus"
      opens the insert menu headlessly.
- [x] Probe evidence (software renderer): menu / slash / block-menu /
      palette / search all open (before) and dismiss on outside click and
      Escape (after); plus-menu apply converts the new block
      (Paragraph→H2 observed); clicking the disabled Table row keeps the
      menu open and the block untouched.
- deferred (unchanged): real Page / Toggle / database blocks stay out of
  scope for v1; the placeholder rows only surface the roadmap.

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

## Track A round 7 (2026-09-20, `m8-hardening`)

- [x] round 6 (popup dismissal + "+" insert menu) committed as `3f56148`
      after the full suite went green
- [x] M8_FEEDBACK #1 closed: the contract gains `SettingDelete`/`MetaDelete`;
      `repository.rs` applies them as plain DELETEs inside the change
      transaction; `settings_store` retires the empty-value tombstone (the
      read-side filter stays for legacy rows and the next save vacuums one);
      storage round-trip + settings-diff tests added, the backup suite's
      tombstone expectation updated to the new contract
- [x] M8_FEEDBACK #9 wired in `AppState::new`: session.meta entries are
      copied into the `metadata` table in one transaction and consumed with
      `MetaDelete`s; an abort summary becomes the notice bar's first line
      (`db_notice` is a queue now, so it composes with the restore and
      library-move notices from `main.rs`)
- [x] last-open page restore fixed: `current-page` was parsed at startup but
      never used, so every restart landed on Getting Started; the recorded
      page now wins unless it was deleted
- [x] `lan_server.rs` test-only imports moved into `mod tests` (last lib
      warning); `main.rs`'s dead `library_moved` initializer dropped

### D13 · clean-exit marker ✅ (Track B agent + Track A wiring, 2026-09-20)

- [x] `main.rs` notes `logging::END_RECORD` after the final flush (normal
      exit path only); `Logger::start()` reads its absence — with no panic
      report — as "did not shut down cleanly (killed, crashed natively, or
      lost power)" and reports it exactly like a panic. The tail check
      follows the rotation family (an end-record shifted into `.1` still
      reads clean), an empty family is a first run, panic reports take
      precedence, a torn last line aborts
- [x] five in-module logging tests (end-record clean, kill incl.
      start-line-only and rotated variants, panic precedence, empty log) —
      lib suite at 102; verified end to end on a scratch database: clean
      `--auto-exit` leaves the record, deleting it or hard-killing the
      process makes the next start log the Warn + meta entry
- [x] the abort summary reaches the user through #9's consumer: notice bar
      first line + a queryable `metadata` row; ADR-0018 carries the update,
      including the accepted false-positive sources (second instance,
      externally killed bench run)
- [x] full suite green on the merged tree (197 pass / 4 ignored by design);
      M8 hardening feedback #1/#9/#10 all resolved — branch merged to
      `master`

## Track A round 8 — Page block (2026-09-20, branch `m8-page-block`)

The round-5 deferral ("Page & Link-to-page blocks need a child-page
column + lifecycle") lands, on `master` after the D13 merge.

- [x] data layer (`08a2e1f`): BlockKind::Page ("page"), `Block.page_ref:
      Option<PageId>`, schema v5 (`blocks.page_ref`, conditional column
      add like v4), `Change::BlockRefSet`; repository insert/load/apply
      carry the column; export renders a Page block as
      `[title](quire://page/<id>)` (re-imports as a clickable link mark)
- [x] lifecycle (`b869afa`): "+" insert menu's Page row is real — one
      batch creates the child page (under the current page) and converts
      the row; the row shows the child's live title (rename path
      reprojections), click opens (block-activate routes kind 11 to
      open_page; the row is never editable), delete takes the child page,
      duplicate deep-copies the child and retargets the copy, paste lands
      the title as plain text (no shared targets)
- [x] document.apply gained the BlockRefSet arm — its absence was caught
      by the headless scene (row rendered "(deleted page)" while the
      sidebar had the child): the scene-first workflow paying off
- [x] tests: storage round-trip (ref set/cleared survive), markdown
      export shape (link + dangling fallback); suite green (199 pass /
      4 ignored); scene `page-block` rendered and reviewed (icon + live
      title + sidebar child, light theme)
- deferred (follow-ups): turning a Page block into another kind keeps the
  child in the tree; a duplicated page's embedded page blocks still share
  the original's child references (recursive copy is v2)

## Track A round 9 — B-package (2026-09-20, on `master`, parallel with Track B's A1–A5)

First round under the split-brief protocol (`docs/AGENT_BRIEF_M8_TAIL.md`):
Track B owns storage/packaging/bench files, Track A owns contract/app/UI;
both commit per task on `master` with surgical staging. No collisions.

- [x] B1 · Link-to-page block (`9f0f496`): BlockKind::Link ("link_to_page",
      UI kind 12) reuses `blocks.page_ref` — no schema change, no new
      Change variant — and points at an EXISTING page it does not own.
      The "+" menu's "Link to page" row flips the slash popup into a page
      picker (`slash-pick-page` mode: every page in tree order, title
      filter, breadcrumb hints, applying converts the line, Escape keeps
      it). Delete leaves the target page alone; duplicate/paste share the
      ref freely (unowned); rendering shares the page-row path with a link
      icon; export matches Page. Scene `link-block` seeded + reviewed.
- [x] B2 · drag-and-drop file import — deferred with a finding: Slint
      1.18.0 handles zero external file-drop events (no
      `DroppedFile`/`HoveredFile` anywhere in the vendored winit backend or
      core), so Explorer drops never reach `DropArea`. Revisit on a Slint
      upgrade or via a Win32 `IDropTarget` hook in `platform/` (needs a COM
      dependency — cost/benefit against §二十七's post-MVP status).
- [x] B3 · rich paste (`b235829` + `cb4334e`): Ctrl+V reads the clipboard;
      text with block structure lands as blocks (empty row converts in
      place, further rows insert after, marks replay as sequential
      ToggleMark commands), plain paragraphs fall through to the native
      caret paste. The first cut read the clipboard through a
      `Get-Clipboard` subprocess — measured 7–10 s on this desktop — so
      the read is direct Win32 FFI (`OpenClipboard`/`CF_UNICODETEXT`/
      `GlobalLock`, microseconds, no new crate), verified end to end with
      a clip.exe round trip.
- [x] gate tests: `parse_if_block_structure` admits multi-block and
      non-paragraph lines, rejects plain paragraphs (marks included);
      suite green throughout (201 pass / 4 ignored) alongside Track B's
      in-flight A1/A3 edits in the shared tree.
- known costs: an N-block paste is several undo steps (inserts chain on
  ids the command planner cannot know upfront); Ctrl+V in a non-empty
  block inserts after it rather than splitting at the caret (Notion does
  the split; v1 keeps the simpler shape). CJK round-trips through the
  FFI read (any source app's CF_UNICODETEXT); the app's own
  `copy_to_clipboard` write path remains ASCII-by-design (clip.exe).

## Track A round 10 — Move page (2026-09-20, on `master`)

While Track B runs its A-package, the B-side found a real SPEC gap:
`PageMoved` existed in the contract and storage since M3, but nothing in
the app ever constructed it — SPEC §十七's "Move page" never shipped.

- [x] workspace: `move_page` (detach + attach, refuses moving a page into
      its own subtree via the ancestor chain, expands the target parent on
      arrival) and `swap_with_neighbor` (sibling ±1 with edge refusal)
- [x] state: `move_page` / `move_page_by` mirror the tree edits into the
      `page_order` map and record `PageMoved` changes (one for a reparent,
      two for a swap); sidebar rebuilds
- [x] sidebar page menu: Move up / Move down / Move to — the second-level
      submenu lists Top level plus every page outside the moved subtree
      (indented tree walk; cycles impossible by construction), keeps the
      popup open like the block menu's mover, and re-anchors the taller
      list so it stays on the window
- [x] small closes riding along: Turn-into away from Page/Link drops the
      block's reference (a Page's child survives, unowned); ⋮⋮ Copy-link
      on a Page/Link row copies `quire://page/<ref>` so the link opens the
      target everywhere (the in-app resolver jumps straight to it)
- [x] test `move_page_reparents_refuses_cycles_and_swaps_siblings` through
      the public API (reparent/refuse-cycle/swap/edge/back-to-root); scene
      `page-move-to` rendered and reviewed (Back / Top level / indented
      targets, the moved subtree correctly absent); suite green (202 pass /
      4 ignored)
- known limit: a Move-to list taller than the window still overflows
  (the clamp moves the anchor, cannot shrink a menu) — same class as the
  block menu's mover; a scrollable menu is a later polish item

## Track A round 12 — sidebar page-tree drag (2026-09-20, on `master`)

The last interaction in SPEC §八's Sidebar list: drag a page row onto
another page to nest it (one PageMoved), onto the Workspace header for
the top level. Same DragArea/DropArea rails as the block handle; the
landing row tints; favorites/recents/new-page rows are not targets;
workspace::can_move_page (extracted, read-only) refuses cycles per hover
frame. Payload MIME 'slint-notion/page:' keeps page drags distinct from
block drags. Test covers nest / refused cycle / non-target rejection /
top-level return; default scene render unchanged (`13a011a`).




## Track A round 13 — Copy Page as Markdown (2026-09-20, on `master`)

While Track B reviews the A4 sweep (Track A holds ui/**; this round is
ui-free), ADR-0025's promised write-path upgrade shipped: palette command
"Copy Page as Markdown" (CMD_COPY_MD) runs the open page through
export_page and puts it on the clipboard — `copy_to_clipboard` swapped its
clip.exe internals for `SetClipboardData`/`GMEM_MOVEABLE` FFI (callers
unchanged, no crate, clip.exe retired entirely). Round-trip test pins CJK
through write → read (`clipboard_write_and_read_round_trip_unicode`);
notice-bar feedback for empty pages and clipboard failures.

## Track B A-package progress (2026-09-20, dispatched via docs/AGENT_BRIEF_M8_TAIL.md)

- [x] A1 · feedback #13 closed (`87ec1dc`): --portable is a real
      LaunchArgs flag, one resolve per start, OpenReport::migrated_from
- [x] A3 · release profile audit (`32f6d48`): four profiles compared on
      scenes A/D/E — all in one noise band; fat LTO and cgu16 rejected on
      build-time/size, panic=abort rejected on behavior (no unwind = the
      panic hook never runs, the crash-recovery chain goes blind);
      profile_bench.ps1 added; the shipped profile wins. Decision pinned
      as ADR-0024; the audit caught and documented a
      parallel-build-polluted measurement batch
- [x] Track A round 11 riding along: settings STORAGE row (data folder +
      open-folder + back-up-now, hidden for memory-only sessions,
      `643bd7d`) and the duplicate-page id-range reservation fix closing
      feedback #2 (`c405a29`)
- [x] A5 · ROADMAP refresh (`2ba401c`): status column current through
      Track A round 11; menu-overflow listed as an explicit M8 remainder
      (kept there — CHANGELOG Known limitations mirrors it; no ADR needed
      for a polish item)
- [x] A2 · first-paint measurement: startup_ms (window-up) and
      first_paint_ms (first frame) are separate measures now, documented
      side by side in PERFORMANCE.md (≈3.9×/5.6× apart) — the M0-era
      "startup_ms is not first paint" gap is closed; terminology note
      recorded so CHANGELOG/PLAN never blend the two. Follow-ups (real_main
      stage timing, skia comparison) live in PERFORMANCE.md
- [x] A6 · installer end-to-end (`2fd633b`, verify-installer.ps1): silent
      install with the .md association task → installed exe launches and
      exits 0 on a scratch db → extracted icon pixel-matches quire.ico,
      version resources correct → Quire.Markdown registered without taking
      the default handler, and the verb's --open path proven live (pages
      14→15) → silent uninstall leaves zero residue (progid, .md candidate,
      shortcuts, Add/Remove); the real per-user library untouched
      (timestamps checked)
- [x] A6 finding folded: install/quire.png ships for nobody — Slint 1.18
      has no Window::set_icon, nothing references the file, and shell
      identity comes from build.rs's embedded IDI_MAIN resource. Track A
      decision: drop the 7.4 KB asset from the installer (the M8_FEEDBACK
      #4 @image-url plan stays retired)
- [x] A4 · visual sweep (`7e6f07b` harness + `2ec7c17` verdict, re-swept as
      sweep2 after Track A's fixes): 13/34 clean at first pass; the three
      HIGHs and most MEDIUMs fixed by Track A (`e4878f1`, `6c115b5`) —
      D1's "overprint" was the marks SCENE's hardcoded offsets (fixed by
      word-derived offsets; the wrap limitation stays documented as
      accepted design debt, not a regression). Remaining: 4 LOW-ish items
      (notice-bar layout, link-dialog gap, swatch contrast, snippet elide)
- [x] A1 follow-up: `install/verify-portable.ps1` — 24/24 over the
      portable layout, log following, migration suppression, --db
      precedence, legacy migration, %APPDATA% hash untouched
- [x] A2 follow-ups: real_main phase stamps (the pre-paint gap split);
      scaling answer LINEAR (state_new ≈ 4 + 13.1 ms per 1000 blocks,
      max residual 1.5 ms); repo_open flat ≈51 ms across 0–10k blocks
      (re-pins ADR-0015's floor); first-paint floor ≈370 ms flat over a
      20× document range; same-batch-only comparison rule recorded
- [x] `benchmarks/scripts/audit_results.ps1`: recomputes every stored
      summary from raw lines, exit 1 on mismatch — tables are renewable
- [x] M8 verification snapshot at `6c115b5`: 209 passed / 0 failed /
      4 ignored, release build clean, installer E2E re-run 4/4 on the new
      exe (7.94 MB, --open 14→15, zero uninstall residue)

### CRITICAL fix — palette dispatch shadowing (Track A, round 13's own bug)

`CMD_COPY_MD` was missing from controller.rs's import list, so the match
arm became a catch-all binding: every palette command with id ≥ 9
(Export/Import/Copy Markdown and every Jump-to-page) ran
copy_current_page_markdown. Caught by Track B's audit — the tests never
walk this dispatch, and the rustc warnings went unheeded by Track A.
Fixed by the import plus converting ALL const match patterns in both
dispatch closures to fully qualified paths, which the compiler rejects
instead of silently binding.

### Pending decisions/actions

- quire.png: CONFIRMED dropped from the installer (nothing references it;
  shell identity = build.rs IDI_MAIN). Track B updates quire.iss +
  verify-installer.ps1's payload assertion together.
- Shortcuts carry no explicit WorkingDirectory — only relevant if a
  --portable installer option ever ships (recorded, no action).
- Track A next: skia comparison is blocked by the shared target lock
  (needs a CARGO_TARGET_DIR policy decision); renderer_name() duplication
  in main.rs vs controller.rs is Track A's to consolidate.

## Track A round 14 — Go Back / Go Forward, and the IME pass closes (2026-09-20)

- **M4 signed off**: the user walked `docs/IME_CHECKLIST.md` by hand and all
  17 items are checked. Recorded in the file itself as a verbal attestation,
  not a measurement — an item found wrong later gets un-checked there.
  That was M4's last open item and M8's last non-code gate.
- **M9 parked** by explicit decision ("安卓先不做"); ROADMAP says so and the
  measured evaluation in `.scratch/m9/report.md` stands as the record.
- **SPEC §十六 Go Back / Go Forward implemented** — the last palette item
  SPEC names that had no code behind it.
  - `NavHistory` (`src/app/state.rs`, next to `AppState`): two stacks, newest
    last, `NAV_MAX` 50 entries, browser semantics (a new navigation drops the
    forward branch). Deliberately a plain struct with no Slint and no
    database in it, so the stepping rules — which are the whole feature —
    are unit-testable; 5 tests in `state.rs::tests` cover retrace/rewind,
    the forward-drop, the bounded stack, the first open, and one row that
    asserts both palette rows exist with their ids (the id *is* the dispatch
    key, which is what the round-13 CRITICAL shadowed).
  - Deleted pages are skipped rather than opened: `nav_step` takes a
    liveness predicate and pops until it finds a page still in the workspace.
  - Recording happens in the controller's `open()` funnel and in
    `create_page` (creating navigates, so Back returns where you were).
    `navigate()` must not re-record — `nav_step` already parked the page
    being left on the opposite stack.
  - Alt+← / Alt+→ (`AppWindow.slint` KeyBinding → two `*-requested`
    callbacks, the convention the rest of the shortcuts use) and the two
    palette rows in a `Navigate` section, with a new `arrow-left` icon
    mirroring `arrow-right`. Settings ▸ SHORTCUTS lists them.
  - **Found on the way**: the typing flush is debounced 300 ms and resolves
    against `state.open_page`, so navigating inside that window dropped the
    last keystrokes — `flush_pending_edit` ran against a page that had
    already changed. `open()` and `navigate()` now flush first. This fixed
    every existing navigation path too (sidebar click, palette jump, search
    result, Page block), not just the new one.
  - **Behavior change worth knowing**: clicking a Page block now goes
    through `open()`, so the top bar title and the sidebar highlight follow
    it the way they do for every other navigation. Previously it called
    `state.open_page()` directly and left the old title on screen.
- **No page-management limitation existed**: a stale doc fragment claimed
  CHANGELOG listed "create, rename, move, favorite pages" as missing. It
  does not — grep finds no such line anywhere. Recorded so nobody re-adds it.
- **Two audit errors of mine, corrected against the files**:
  `.vscode/extensions.json` recommends `Slint.slint` and `settings.json`
  configures its language server (I had it backwards, and reported it to the
  user that way); `renderer_name()` exists once (`controller.rs:171`), so the
  "Track A next" line above about consolidating it is stale. What §四 task 6
  genuinely still lacks: rust-analyzer settings and a verified live preview.
- **SPEC audit, still open** (each one re-read from source, not from this
  ledger): §十二 block virtualization is not windowed — `reproject_blocks`
  sets every row and `Editor.slint:237` is a plain `for` in a `ListView`, so
  10 000 blocks are 10 000 realized rows each carrying a TextInput; M7's
  "virtualization ✅" overstates it. §廿六 plain text absent (md only, dialog
  filter `&["md"]`). §廿三 memory attribution absent (2 counters total, and
  the checklist ticks "GPU-side memory separate ✓" with no GPU reading ever
  taken). §廿七 tray / native menu / startup options / global shortcut: none,
  and `system-tray` is compiled in unused. §卅一 DPI: 125/150/200% never
  measured. §十三 `composition_state` never named.
- **Visual gate, and what it exposed** (sweep5 vs the sweep4 baseline, hash
  diff so only moved pixels get re-judged): 32 of 34 scenes byte-identical,
  exactly the two that should have changed — `palette.png` (52 px, the scroll
  thumb shortening) and `settings.png`. 52 px is the evidence that the two new
  palette rows are *in the model but below the palette's visible fold* at an
  empty query, so the sweep could not see them at all. Added a `palette-nav`
  scene (`apply_scene_overlay`, query `go`) and it renders both rows with the
  new `arrow-left` icon and the `Alt+Left` / `Alt+Right` hints intact — that
  scene is now the permanent coverage for §十六's rows.
- **Pre-existing defect, not this change's** (pixel-cropped from the baseline,
  not eyeballed): the Settings popup is taller than the 1280x800 window, so
  ▸ SHORTCUTS ▸ ABOUT ▸ the **Done button** have been off-screen since before
  this commit — sweep4's last visible row was already `Ctrl+S / Save now`. The
  new `Alt+Left / Right` row lands exactly on the cut line, visible but the
  last thing on screen. Fixing it means making the popup scroll or clamping its
  height, which is its own item — recorded, not done here.

## M10–M14 · 规格已定，未开工（2026-09-20）

范围由用户点定：除云同步 / 协作 / AI / 插件 / 发布站点 / 评论外，
Notion 的其余能力全部进入排期。规格见 SPEC §三十七–§四十，
里程碑与验收门槛见 SPEC §二十九 与 ROADMAP 的 M10–M14，
决定记录为 ADR-0027。开工前仍按 §三十 的流程走：读架构 → 最小改动 →
编译 → 测试 → 检查 UI → 记录性能影响。

建议起手：M10 批次 A 的 image 块（附件落盘 + 降采样缓存是这批里唯一
会动摇低 RAM 卖点的部分，值得最先测量）。

## M10 批次 B · slice 1 — toggle 折叠块（2026-09-20，on `master`）

§三十七 的第一条完整竖切。没按批次 A 起手，是为了先把「行的生死」这件事
做对：toggle 是这一批里唯一会改变 projection 输出的 kind，而 table / columns
只会重复它。

**改动面**（六处接线，SPEC §三十七 的门槛）：`BlockKind::Toggle`（int 13，
`as_str` = `"toggle"`）；`blocks.folded` 列 = schema v6（条件 ALTER，走
`pragma_table_info` 探测，和晚近几列同一写法）；`Command::ToggleFold` →
`Change::BlockFoldedSet`（可撤销，和侧栏 `PageExpandedSet` 同形，属视图状态
不入正文）；slash / Turn into / ⋮ 菜单各一行；`EditorBlock.slint` 的
chevron（`chevron-down` / `chevron-right`）；scene `toggle` + `toggle-fold`。

**projection 是这次真正的决定**：折叠的子树不产生 row。`project_blocks` 经
`visible_block_indices` 过滤，同一个函数也被新的 `drop_index_for_row` 用——
两条编号（row / model）从此不再等价，§八 的拖拽落点必须换算。这个缝只留
一个，防止两边漂移。`SetBlockType` 离开 Toggle 时会清掉 fold（undo 会还原），
否则子树被藏起来且没有任何出口；`SplitBlock` / `DuplicateBlock` 强制
`folded: false`，因为它们都不复制子树。Markdown 导出把 toggle 降级成 quote 行
（CommonMark 没有折叠语法，和 callout 的降级同一条路），子树仍按 depth 缩进带上，
导入侧不还原折叠——§三十七 要的就是这个不对称。

**验证**：`just check` 全绿（check --all-targets / 119+13+9+35+5+17+16+13 测试
/ release build 3m16s）。新增测试：v6 迁移（先 `DROP COLUMN folded` +
`user_version=5` 造旧库，带 control 断言证明夹具真的缺这列，再跑两遍验幂等）、
fold 往返 + 不存在 id 报错、折叠子树零 row、展开回到源顺序、编号列表不因隐藏
而改号、拖拽落点换算（并证明朴素的 row index 1 会被 `can_move_block_to` 拒）。
三处故意变异（关掉 filter、`can_fold` 恒真、关掉 SetBlockType 的守卫）各自
只打挂预期的测试后回滚——绿得不算数的测试我不留。

**像素**（headless `quire-shot`，`.scratch/sweep6`，基线 sweep5）：35 个旧
scene 里 32 个逐字节相同；只有 `slash` / `plus` / `dark-slash` 动了，原因就是
菜单多了 Toggle 一行。`toggle` vs `nest` 差 1 257 px，全在父块那一行（18 px
高的一条），即 chevron + 22 px gutter 生效且没有波及；`toggle` vs
`toggle-fold` 差 6 006 px，分成四块且每块都有解释：三角形字形、消失的子块
行、其下方整体上移一行、右侧滚动条滑块高了两像素。gutter 只按 kind 给、不按
`can-fold` 给，所以第一个子块出现时文字不会跳。

**未验证**：交互。桌面 UI 不归我点——chevron 的 hover 命中、折叠后继续打字、
⌘F 命中隐藏块、拖进隐藏区这几条要用户手测。另外 `blocks.folded` 目前没有任何
UI 入口能折叠「非 Toggle」的父块，这是刻意的（没有出口的状态不算状态）。

## M10 批次 A · slice 2 — image 块（2026-09-20，on `master`）

回到 §三十七 批次 A。这一条是 M10 里唯一会动摇低 RAM 卖点的 kind，所以
先做它、并且真的去量它，而不是等 table / columns 把媒体层逼到墙角再补。

**改动面**（§三十七 的六处接线，逐条对上）：`BlockKind::Image`（int 14，
`as_str` = `"image"`）；schema v7 = 新表 `attachments` + `blocks.attachment` +
`blocks.img_percent DEFAULT 100`；新的 `services/attachment_store.rs`（落盘、
嗅探格式、降采样）；`Command::InsertImage` / `SetBlockImage` / `SetImageWidth`
三个命令，各自 `Change` 可撤销；slash 与 Turn into 各一行 Image，⋮ 菜单对
图片块多开一个 `Image width` 子菜单（25 / 50 / 100，action id 基址
`IMAGE_WIDTH_BASE = 500_000`）；Markdown 导出 `![name](quire://attachment/<id>)`、
导入侧没有图片形状，整行按字面文本留在段落里（文件名因此不丢）；
`EditorBlock.slint` 的图片行 + `AppWindow.slint` 的点击放大遮罩层；
scene `image` + `image-half`。

**三个决定**：

1. **字节在库里不在库外**——`<library>/attachments/<id>.<ext>`，SQLite 只留一行
   引用。`blocks.attachment` 刻意**不建外键**：文件行丢了，块必须还能载入并画成
   「缺图」，而不是让整本库打不开（用户只拷走 `.db` 是常态）。这条由
   `a_picture_whose_attachment_row_vanished_still_loads` 用裸 SQL 删行钉住。
2. **编辑器永远看不到原图**。超过 `MAX_EDGE = 1280` 的导入会另写一张
   `<id>.cache.png`，`display_path` 只交这张；原始字节一个 bit 都不动，看画
   不该降级用户的文件。扩展名与 MIME 由 `image::guess_format` 嗅字节得出，
   不看文件名——存成 `.jpg` 的 PNG 要按它真的是 PNG 来存。
3. **解码缓存自己管，且有上限**。Slint 的 `i-slint-core` 图片缓存是
   thread-local `CLruCache`，按解码字节加权、上限 **5 MiB**、键是 path+mtime
   ——一张 1280×720 RGBA 就 3.7 MiB，即整个缓存装得下一张照片，滚过图页每帧
   重解码。反过来做一张无上限的自有 map，就是 §二十二 的 RAM 承诺死在第一条
   截图墙上。所以是 `AppState` 里一张按 attachment id 的 LRU，权重 w·h·4，
   上限 `MAX_ATTACHMENT_CACHE_BYTES = 32 MiB`（≈八张全宽帧）。行通过
   `image-for` / `image-aspect` **回调**取图而不是模型字段——Slint 只为它
   realize 的行求值，于是成本跟视口走，不跟文档走。

`Command::InsertImage` 的 apply 是 `AttachmentAdded`（`INSERT OR REPLACE` 的
upsert）+ `BlockInserted`，revert 只删引用、**没有** `AttachmentDeleted`：撤销
一次插入不该删磁盘上的字节，redo 也因此不需要碰盘。Image 块复制粘贴共享同一
个 attachment 指针而不是拥有两份；⋮ 的宽度设回默认 100 时 `plan()` 返回
`None`，菜单点不出空撤销步。

**验证**：`cargo test --workspace` 全绿（129 + 13 + 9 + 36 + 5 + 17 + 19 + 13 =
241 个测试，EXIT=0），`just check` 的 `cargo check --all-targets` 与 release
build（3m11s）在本 slice 的代码收尾处跑过，之后只动过文档。新增测试里值得点名
的：v7 迁移（先造 v6 旧库 + control 断言证明表和列真的不存在，再跑两遍验幂等）、
附件往返、悬空引用仍能载入、`attachment_store` 五个（含 `MAX_EDGE + 1` 卡在
分支边界上的降采样，以及「不是图片的字节被拒绝且不落盘」）、markdown 的
`quire://attachment/` 往返、command 层四个撤销用例。缓存那条
`the_picture_cache_spends_its_budget_and_drops_the_stalest_first` 先证明预算
**真的被花掉**（`spent > MAX/2`，否则断言在空缓存上恒真），再证明淘汰的是
最久没被 realize 的那张而不是最早插入的那张（把一张已淘汰的 id 重新 show 出来，
再灌四张新的，它必须还在、它的邻居必须不在）。这条测试抓到过一个真 bug：
`cache_image` 插入后没把新条目的权重计进 running total，于是 11 张全留着、
一个字节都没淘汰——断言把它逼出来了。

**RAM**（release `quire.exe`，`bench.ps1`，每场景 seed 遍 + 测量遍、独立 pinned
`--db`，原始行 `benchmarks/results/2026-09-20-m10-image-ram.jsonl`）：A 空壳
116.5–116.6 MB WS / 88.1–88.9 private；D 10 000 blocks 126.4–126.5 /
100.9–101.3，四遍的散布 ≤0.4 MB。对 M7 matrix 的同名两行（114.3 / 88.2 与
121.5 / 97.4）是 **1.02× 与 1.04×**，M10 的 ≤1.2× 门槛守住；idle CPU
0.2–0.59%。注意这一遍量的是「加了 v7 列、加了缓存字段、但页面上没有一张图」
的回归基线，它证明的不是图便宜，只是没有把别的东西弄贵。

**像素**（headless `quire-shot`，`.scratch/sweep7`，基线 sweep6）：37 个旧
scene 里 34 个逐字节相同；`slash` / `plus` / `dark-slash` 动了，裁出来就是
菜单尾部（Callout / Code / Divider 整体下移一行）和引用块，原因就是菜单多了
Image 一行。两张新 scene 是宽度档位的几何证据，不是眼看：同一张 640×400 的
fixture，`image` 画到 **760×474**、`image-half` 画到 **380×237**，左边和上边
都还在 x=390 / y 同一条线上——横竖都精确减半，比例没变形；两图差 339 118 px，
`image-half` 里图片下方已经回流进「Why a local-first editor」整段，而 `image`
那一屏只有图。行高由 `pic-aspect` 在拿到像素之前算出，所以半档那行是真的矮了，
不是被裁了。

**未验证**：交互。桌面 UI 不归我点——文件选择器（`rfd`）挑图、点图放大、
遮罩层点击关闭、⋮ 里 `Image width` 子菜单的命中，这几条要用户手测。RAM 也
只量了无图的场景；「一万块 + 一屏图」滚动时的 RSS 需要一个 bench 场景，
目前没有，32 MiB 上限是构造性保证 + 单测证明，不是测量结果。原地替换磁盘上
的同名文件要重启才生效（我们的键是 id，Slint 的键才带 mtime）。粘贴剪贴板
位图（`CF_DIBV5`，`platform/`）与 file / PDF 缩略图仍在本批次里没做，它们复用
同一个 store。

下一步仍是 §三十七 批次 A：file + PDF 缩略图（复用 attachment store），
然后批次 B 的 table + columns。

## M10 批次 A · slice 3 — file 附件块（2026-09-20，on `master`）

批次 A 的第二条。用户 2026-09-20 指定「pdf 附件的先跳过，做其他的」，所以
这一条交付 file 本体，PDF 首页缩略图留在原地。

**改动面**（§三十七 的六处接线）：`BlockKind::File`（int 15，`as_str` =
`"file"`）；**schema 不动**，`user_version` 停在 7——v7 为图片开的 `attachments`
行与 `blocks.attachment` 列描述一个 zip 和描述一张 png 一模一样；
`attachment_store.rs` 加 `import_any_file`（`fs::copy` 流式落盘）、`export_to`、
`save_name`、`format_size`、`create_file_fixture`；`Command::InsertFile` /
`SetBlockFile`，并且图片那两条的 `plan()` 助手被泛化成 `insert_attachment` /
`set_block_attachment`（只有 `kind` 一个参数不同），`AppState` 侧四个方法同样
收成两个带 kind 的 `insert_attachment` / `set_block_attachment`；slash、insert
（+）、Turn into 三个菜单各加一行 File，⋮ 的 `Image width` 子菜单**仍然只对图片
开**；Markdown 导出 `[name](quire://attachment/<id>)`；`Types.slint` 三个回调
`attachment-size`（pure）/ `attachment-opened` / `attachment-saved`，
`EditorBlock.slint` 的 kind 15 行；`platform::open_with_default`；scene `file`。
页脚字数把 File 和 Image 一起跳过——附件的 text 是文件名，不是散文。

**三个决定**：

1. **不加 schema**。这不是省事，是 v7 当初就按「一个块指向一坨存起来的字节」
   设计的，图片只是它的第一种用法。于是这一条 slice 没有迁移、没有幂等测试、
   没有旧库升级路径要验，`PRAGMA user_version` 那条 v7 迁移测试原样通过。
2. **字节不进进程**。`import_any_file` 用 `fs::copy` 把源文件直接抄进
   `attachments` 目录，只记长度：不解码、不调 `image`、**刻意不设体积上限**。
   图片那条路靠降采样守 §二十二，文件这条路靠「根本不看」守——一张 2 GB 的
   附件和一个 2 KB 的附件在编辑器里花一样的钱，直到用户去按按钮。
   `fs::copy` 返回的字节数与 `metadata().len()` 不符就删掉半成品并报错，
   所以不存在「半个附件」留在目录里。
3. **打开是显式按钮，不是点行**。一行点击就调用系统默认程序启动任意可执行文件
   的块，是不敢在里面选文字的块；所以行本身照旧走 `block-activate`（可选、可
   复制），右侧两个 26 px 按钮才做事。两个按钮**常画**而不是 hover 才出现：
   一个看上去没东西可按的文件块，读起来就是坏掉的附件，那是这类块唯一不能给的
   印象。`ShellExecuteW` 而不是 `spawn("explorer.exe", path)`——explorer 成功时
   也返回 0x1（它的设计），子进程分不出「拉起了应用」和「扩展名被拦」，而这个
   FFI 只在真拉起来时答 `> 32`；一个 extern 声明，`windows` crate 不进依赖树，
   跟 ADR-0025 读剪贴板同一条规矩。

**这一条里改过的两个设计**：文件的名字**保留扩展名**（`import_any_file` 取
`file_name()` 而不是 `file_stem()`），图片仍只留词干——一张图的内容就是它的
类型说明，一个 `quarterly-report` 什么都不说。这带来 `save_name` 的一个坑：
存进去的 `file` 列扩展名被小写过（`22.pdf`），`name` 保留用户大小写
（`Report.PDF`），拼回去时必须忽略大小写比较，否则给文件名加一遍后缀。
另一个是 fixture 的确定性：`create_file_fixture` 第一版把 `std::process::id()`
拼进文件名，而文件名就是行上那个 label，于是 scene 每跑一遍哈希就变一次；
pid 移到临时**目录**名上，文件名回到干净。这条是在看截图时发现的，不是靠眼看
判缺陷。

**验证**：`cargo check --all-targets` 干净；`cargo test` 全绿
**249 passed / 0 failed / 4 ignored**（11 个 target：lib 136、13、9、37、5、
17、19、13），`cargo build --release` exit 0。本 slice 新增 8 个测试：
`command.rs` 三个（文件块把文件名当 text、撤销只丢引用且 redo 不碰盘、
Turn into 换掉文字并还原、一张图和一个文件可以指向同一行 attachment）、
`attachment_store.rs` 四个（任意文件逐字节抄进去且从不解码、copy 失败不留半成品
含无扩展名回落到 `.bin`、`save_name` 的两种拼法、`format_size` 的读数）、
`markdown_test.rs` 一个（导出成链接且导入侧真的把 `quire://attachment/12` 带回来
——这是 file 与 image 在导出上唯一的方向性差别）。`attachment_store` 那条
`format_size` 测试当场抓到一个真缺陷：`bytes` 为负时 `< 1024` 分支打印的是原始
值而不是钳过的值，于是返回 `"-5 B"`；断言逼出来了。

**RAM**（release `quire.exe`，`bench.ps1`，原始行
`benchmarks/results/2026-09-20-m10-file-ram.jsonl`）：D 10 000 blocks
126.5–126.6 MB WS / 101.1–101.3 private，与 image 那遍的 126.4–126.5 /
100.9–101.3 差 ≤0.2 MB——**1.04×**，门槛 ≤1.2× 守住。A 空壳 119.2–120.7 /
91.7–92.5，比 image 那遍的 116.5–116.6 / 88.1–88.9 高 2.6–4.1 MB。**这个差
没有归因**：两遍 A 场景里都没有任何附件，不可能是文件行的钱，而且这一遍两趟
自己就散 1.5 MB（上一遍散 0.1 MB），说明空壳读数本身没那么可复现。写下来而不
是圆过去。第一趟 A（`m10file-A1`）因为 `-PinnedDb` 被 bash 吞成 `:TEMP\...`
而废掉，那一臂无法自证开了哪个库，已从 jsonl 里剔除。

**像素**（headless `quire-shot`，`.scratch/sweep8`，基线 `.scratch/sweep7` 的
39 个 scene）：36 个逐字节相同，`slash` / `plus` / `dark-slash` 动了，裁出来
就是菜单里多出 File 一行、下面整体下移，`slash.png` 全图能看到
Text / Toggle list / Image / **File — Attach a file of any type** / Callout 的
顺序。`file` 是新 scene，量出来的几何：盒子 x 390..1149（760 宽）、
y 265..312（48 高，正是 `size-body × line-body + 2 × spacing-md`），左边
`page` 图标 + 文件名，右侧右对齐 "1.8 MiB"，两个 26 px 按钮落在 x 1086..1112
与 1116..1142——按钮无 hover 时透明，所以列占用图上只有两处图标笔画（x≈1099、
x≈1135），这与「常画但只在 hover 才有底色」是两件事，别混。**新基线是
`.scratch/sweep8`（40 scene）**。

**未验证**：全部交互。文件选择器（`rfd`，All files 过滤器）、Open 按钮真的把
字节交给系统、Save-as 对话框给的文件名、两个按钮的 hover 命中、⋮ 菜单、
撤销/重做、Turn into 成 File、以及把窗口拉窄时 `width: parent.width - self.x -
140px` 那条 elide 会不会吃掉文件名——都要用户手测。另外两个已知没做的：
PDF 首页缩略图（按指示推迟，渲染路线未定），以及**附件永不回收**——没有外键、
没有级联，删掉最后一个指向它的块，字节还在 `attachments` 目录里；这是刻意的
（撤销要能不碰盘地找回引用），但对用户就是一条看不见的磁盘泄漏。

**下一步**：批次 A 收尾的仍是那张「一万块 + 一屏媒体」的滚动 bench 场景
（image 欠的，file 让它欠了两次），然后批次 B 的 table + columns。

## M10 批次 A · 收尾 1+2 — 临时目录卫生，和一条并不是死代码的 GIF 分支（2026-09-20，on `master`）

file 竖切之后那份盘点里的头两项，用户点定「先做 1+2，然后 commit，再往下走
table」。两项都不是功能，但第二条在做的过程中翻出一个前提错误，所以也记一笔。

**1. 测试在漏 `%TEMP%`。** 症状是 1648 个 `quire-log-*`；查下来是仓库级的问题，
不是一处：`logging.rs`、`storage/database.rs`、`storage/data_location.rs`、
`services/attachment_store.rs`、四个 `tests/integration/*`，加上三个 bench 脚本，
一共十几个前缀、3639 个条目。每个 helper 都建一个「pid + 计数器 + 时钟」的独
立目录并且注释里郑重解释为什么要唯一——**唯一性之所以必要，恰恰因为从来没人
删**。所以收法不是给每个 helper 补一句 `remove_dir_all`，而是加一个会自己删除
的守卫：`src/testing.rs` 的 `ScratchDir`（`Drop` 删树，`Deref<Target = Path>`
让 `dir.join(..)` / `&dir` 原样可用）。它必须是 `pub` 而不是 `#[cfg(test)]`：
集成测试是另一个 crate，编译 lib 时不带那个 flag，`tests/integration/**` 看不
到 `#[cfg(test)]` 的东西。

换下来的写法里有三处值得记：

* `scratch_logger` 原来返回 `Arc<Logger>`，现在返回一个把 logger 和守卫一起
  装进去的 `ScratchLogger`，靠 deref 让测试体一个字不用改。返回 `(Logger,
  ScratchDir)` 也行，但那会让 12 个调用点都变成解构。
* `Logger::new(scratch("clean")) 这种一行式必须拆开绑成变量：临时守卫在语句结
  束时就 drop，目录会在 logger 还在往里写的时候被删掉——不报错，只是下一行又
  把它建回来，于是白改。
* `backup_test.rs` 的 14 个测试每个末尾都有一句 `remove_dir_all(&dir).unwrap()`，
  它只在绿的时候执行；`persistence_test.rs` 里那段「先把上一轮的 `.db.bak<N>`
  删干净」的手扫，在目录名必然全新的前提下是死代码，一起删了。
* `data_location.rs` 的 `per_user_dir()` 原本返回 `PathBuf`，而它派生自的那个根
  目录才是守卫；两者拆开会留下悬空路径，所以并成一个 `Root { _appdata, per_user }`。

**量具**：`cargo test` 全绿（139 + 13 + 9 + 37 + 5 + 17 + 19 + 13 = **252**，
EXIT=0），跑完后 `%TEMP%` 里 `quire-*` 的总数从 3639 变成 **3639**，按 mtime
筛 10 分钟内新建的只剩 `quire-attachments/` 一条——那是 `for_db(None)` 的兜底目
录，固定路径、内容确定（`1.png` / `1.pdf`），不随运行增长，所以这次不动它。
被删掉的守卫自己也有测试：`testing::tests` 两条，一条证明绑定时目录在、离开作用
域后不在（断言写在 `Drop` 之外，否则自己给自己打分），一条证明同名两次调用不会
撞车。

**PS1 那一半**：`bench.ps1 -Typing` 和 `startup_bench.ps1` 原来只在跑之前清
`.db` / `-wal` / `-shm`，从不清跑之后。证据是对 `%TEMP%` 里 91 个
`quire-firstpaint-*` 分类：71 个 `.db.bak1` + 10 个 `.db.bak2` + 10 个
`.db.bak3`，全是应用自己写的首开快照，正好落在旧清理清单的缝里。现在两处都改成
`Get-ChildItem "$db*" | Remove-Item -Force`（整个文件族），`bench.ps1` 顺手删自
己的报告 json。`bench_matrix.ps1` / `profile_bench.ps1` 不动：它们的 scratch 是
单个固定目录（`quire-matrix` / `quire-profile`），有界且本来就是复用。

**2. GIF 分支不是死代码——我原来的判断是错的。** 盘点时我说 `ext_of` /
`mime_of` 的 `ImageFormat::Gif` 两条到不了，理由是 `Cargo.toml` 只要了
`png`/`jpeg`/`bmp`。查 `cargo tree -e features -i image`：`image feature "gif"`
**是开着的**，路径是 `slint "image-default-formats"` → `i-slint-core` →
`image "default-formats"` → `gif`（顺带 avif/exr/tiff/webp 全都在）。也即
`load_from_memory` 今天真能解 GIF，那两条分支活得好好的；按原计划删掉它们，反而
会把一张真收进来的 GIF 命名成 `.img` + `application/octet-stream`。

所以修法是反向的：把 `gif` 显式写进我们自己的 features（不额外编译任何东西，
但行的格式从此是我们声明的，不是依赖的选择），选择器加上 `gif`（否则那条分支
在 UI 里到不了，等于白留），并补一条测试 `a_gif_is_stored_as_a_still_gif`——它
用**同一个 crate 编码**一张 GIF 再导入，编不出来就直接 fail，而不是在一条走不到
的分支上恒真。gif 在这里是静图：解码器给首帧，App 里没有动画时钟，SPEC §三十七
新增的那条格式条目把这个边界写死了（webp / tiff 能解但不进选择器，因为它们的行
扩展名只能落到 `.img` 兜底）。

**docs**：SPEC §三十七 批次 A `image` 加格式条目；CHANGELOG 新增 `### Build &
test` 节（`just check`、`ScratchDir`、脚本清理）并在 Image 条里补格式；
PERFORMANCE scene E 脚注说明 harness 现在会删自己的库和快照。PLAN 就是本节。

**顺手清掉的历史遗留**：`%TEMP%` 里那 3639 个 `quire-*` 条目是这些泄漏的账本，
测试已经不再生产它们，所以按前缀列出来核对（1648 log / 621 test / 599 location /
447 persist / 124 search / 91 firstpaint / 71 typing / 10 backup，尾上是
matrix / dbg / imgbench / integrity / ab / lan / d9 / attachments 这些一次性探测）
后全部删除，现在 `ls $TEMP/quire-*` 是 0 条。没有 quire 进程在跑，所以删的都是
已完结会话的遗留。

**视觉**：这一轮没有改动任何会被画出来的东西，但 `create_file_fixture` 换了临时
目录的形状（pid 从名字里挪进目录名），而 `file` scene 的行标签正是从那次导入得出
的，所以复扫一遍 40 个 scene 对照 `.scratch/sweep8`：**changed 0 / identical 40**。
基线不变，`gif` 那条特性与选择器条目也就此确认没有把任何一张图挪动过。

## M10 批次 B · slice 4 — table 网格（2026-09-20，on `master`，ADR-0031）

批次 B 的第一条。SPEC §三十七 写死它是「简单表格，不是 Database」，所以这条 slice
的全部设计压力都落在一个点上：**怎么让网格不变成第二套内容模型**。

**改动面**（§三十七 的六处接线）：`BlockKind::Table` + `BlockKind::TableCell`
（`as_str` = `"table"` / `"table_cell"`）；schema **v8**，`blocks.columns INTEGER
NOT NULL DEFAULT 0`，`add_columns_column` 一步；`Command` 四条
`TableAddRow` / `TableAddColumn` / `TableDeleteRow` / `TableDeleteColumn`，但它们在
`plan()` 里全部摊成一串普通的 `BlockInserted` / `BlockDeleted` / `BlockTextSet`
外加唯一一个新 `Change`——`BlockColumnsSet`；`Types.slint` 加 `TableCell` struct，
`BlockRow` 复用 v8 那个 `columns: int`（kind 16 = table、18 = columns 的栏数）并加
`table-cells`；新组件 `TableBlock.slint`（画整张网格并
宿主那一个活的 `TextInput`）与 `TableEdge.slint`（hover 边缘条，Add row / Add
column / Delete row / Delete column）；`UIState.table-cell-move` 走 Tab /
Shift-Tab；slash、insert（+）、Turn into 三个菜单各一行；Markdown 导出 GFM 表格；
scene `table` 与 `table-edit`。

**四个决定**：

1. **格子是子块，不是 payload**。三条路里选的最窄的一条：JSON blob 会让格子不再是
   块，于是 inline marks、undo 粒度、§三十九 的 relation/rollup 全得在模型外面重造
   一遍；真·每行一条记录的 schema 是 §三十九 的活，而 SPEC 明说这个 kind **不是**
   数据库视图。子块只多花一列整数，其余全部复用。代价是 `rows` 不能存，只能派生
   （`cells / columns`），所以行主序的 `order` 是这套结构唯一的责任人。
2. **整张网格占一个编辑器行**。`visible_block_indices` 把格子从 rows 里删掉——
   与 ADR-0028 藏折叠子树同一个过滤器、同一个位置。这张表因此不论多大都只花一条
   delegate，而 §十二 的虚拟化前提不被破坏。
3. **加列是这轮唯一真正的难点**：它要在 `rows` 个不同的缝里同时插 `rows` 个块。
   `OrderKey::STRIDE` 定成 `1 << 16`、`renumber_page` 改用同一个 stride 铺开，就是
   为了这个批量——每插一个就取一次中点会把缝对半砍掉，缝很快就没了。新助手
   `keys_between` / `keys_in_gaps` 一次算完整批。
4. **残缺网格只读**。`grid()` 见到 `cells % columns != 0` 直接 `None`，`plan()`
   于是拒绝一切编辑——与其猜哪一行缺了，不如不动。`DuplicateBlock` 对这种表也拒绝
   复制，因为没有格子的网格一打开就是坏的。

**这一轮翻出来的一个老缺陷**：`DocumentRow.head` 绑了 `height` 却没绑 `y`，而
Slint 对这种子元素做的是**在父元素里垂直居中**。M1 起每个 scene 的页面标题就一直
画在它绑定位置的下面 ~34 px，只是以前的首块全是一行高，居中量看不出来；table 是
第一个「高的首块」，于是网格直接压在标题上（~60 px）。显式写 `x: 0px; y: 0px;`
修掉。同一条 scene 复扫时又发现 `absolute-position` 在 ListView delegate 里不可信
（row 1 报出自己的 head 在自己的 body 下面，而两者 `y` 都是 0），真实几何只能用
`parent.y + head.height`，也就是 `row-y` 已经在往下传的那个值。两条都记进
`docs/UI_ARCHITECTURE.md` 的「Slint geometry traps」。

**验证**：`cargo check --all-targets` 干净、`cargo test` 全绿、`cargo build --release`
exit 0。本 slice 新增的代表性测试：`command.rs` 侧 `a_line_becomes_a_grid_that_holds_its_words`、
`adding_a_column_puts_one_cell_in_every_row`、`cells_key_into_one_gap_as_a_batch`、
`a_ragged_grid_is_not_editable`、`every_row_index_consumer_shares_the_one_visible_list`、
`flattening_a_grid_gives_the_words_back_as_paragraphs`、`undoing_a_table_delete_brings_the_grid_back`；
`storage_test.rs` 的 `a_grid_and_its_cells_round_trip_through_storage` 与
`the_v8_step_adds_columns_to_a_v7_database`；`markdown_test.rs` 四条（GFM 带表头、
格子里的 marks 与转义、空格子导出成空、以及 `|a|b|` **导入回段落**这条反直觉的边界）。

**RAM**（release `quire.exe`，原始行 `benchmarks/results/2026-09-20-m10-table-ram.jsonl`）：
D 10 000 blocks 130.8–131.0 / 104.5–105.7 = **1.08×**，A 空壳 124.8–125.3 / 96.7–98.3。
门槛 ≤1.2× 守住。两个值得记的点：一是与上一批比 A +4.1…6.1、D +4.2…4.5，**两臂同向
同幅**，所以那笔移动不是表格的钱（空壳里一个网格都没有），记为 session drift；
二是这批里唯一可归因的 A/B——投影里 `table_cells` 不加 `kind == Table` 保护时，
每行都扫一遍页面并分配一个空 `VecModel`，D 高 ≈2 MB WS / ≈2.5 MB private，A 在自己
的散布内不动（空壳只有 24 行，付 24 次）。启动时间两臂都没动（475–525 vs 498–552 ms），
因为 `startup_ms` 停在窗口句柄那里，而投影发生在那之后——保护留着是因为内存读数加
渐近线，不是因为某个数字动了。

**像素**：`--hover x,y` 是这条 slice 给 `quire-shot` 加的能力（只发 `PointerMoved`
不按下），因为边缘条只活在 hover 上，这是唯一的 headless 路子。`table` /
`table-edit` 进 sweep，22 px 那条边缘条另有手工 `--hover` 一张。标题居中的修复
**重基线了 42 个 scene 里的 37 个**（`.scratch/sweep10`），判法是新工具
`benchmarks/scripts/diffbbox.ps1 -OldDir A -NewDir B`：逐 scene 打印变化像素的
bounding box，37 个框全部落在标题带里，于是一个结论管住整套，而不是看 37 张图。
投影保护那次复扫（`.scratch/sweep11`）**42/42 逐字节相同**，所以它是内存改动不是
视觉改动。

**未验证**：格子内 Ctrl+L（链接）没接；Enter 不拆格、退格不并格、上下键不跨行，
只有 Tab / Shift-Tab 走格；带 marks 的一行 Turn into 成表格时字留下、marks 丢掉；
hover 时边缘条作为一行挤进布局，所以表格下方内容在指针停留时下移 22 px。这些全写
进 CHANGELOG 的 Known limitations 与 SPEC 批次 B 的边界条目，不当缺陷处理。

**下一步**：批次 B 剩下的 columns。

## M10 批次 B · slice 5 — columns 分栏（2026-09-20，on `master`，ADR-0032）

批次 B 收尾。**这一条没有新迁移**：栏数就存在上一条为网格开的那个
`blocks.columns` 里。理由与 ADR-0030 对 v7 的说法同一个——「这个容器有几个栏」和
「这个网格有几列」是同一个事实，为一个改名付一次迁移不值。

**改动面**：`BlockKind::Columns` + `BlockKind::Column`；`Command` 三条
`ColumnsAddColumn` / `ColumnsDeleteColumn` / `ColumnsAddBlock`，同样摊成子块的
增删改 + 一个 `BlockColumnsSet`；`Types.slint` 两个 struct——`ColumnItem`（一个摊平
的 `[ColumnItem]`，带 `column` / `depth` / `kind` / `text` / `runs` / `checked` /
`number` / `folded` / `color` / `bg` / `attachment`）与 `ColumnBox`（`id` /
`column` / `first` / `size`，形状表）；新组件 `ColumnsBlock.slint` 与
`ColumnItemRow.slint`；`UIState.column-item-move`（Tab / Shift-Tab 在栏间走字）与
`column-fill`（空栏的点击）；slash / insert / Turn into 三处；Markdown 导出摊平成
页面级段落；scene `columns` 与 `columns-3`。

**三个决定**：

1. **layout 的内容画在 layout 自己那一行里**。Slint 没有递归组件，一个栏不可能装
   一个 `EditorBlock` 而后者又装栏，所以唯一能画的地方就是 layout 那一条 delegate
   ——这恰好就是 §三十七 那句「分栏只在可见窗口内展开」要的，也是 §十二 虚拟化前提
   要的。`visible_block_indices` 把栏和栏里的行一起藏掉。
2. **平铺交给 `FlexboxLayout`**（单行、`flex-wrap: no-wrap`、`alignment: stretch`、
   每格 `horizontal-stretch: 1`），不自己写宽度。SPEC 批次 B 本来就点名要这条。
3. **空栏必须能被点开**。一页上只有这一种东西是指针能命中而光标进不去的，所以它
   写「Empty column」，点它 = `ColumnsAddBlock` 给它第一个段落，而不是让点击静默。

**两条 QOML 的坑**（都进 UI_ARCHITECTURE.md）：`for … : if … : Component` 是解析
错误，所以「过滤后的重复」只能做成形状表让 delegate 自己切片（`ColumnBox.first` /
`size`）；组件实例化里 property 右侧的标识符是在**外层**组件解析的，所以子组件拿不到
自己父母的 `root.x`——layout 只能用普通名字往下传 `line-y` / `line-x` /
`each-width`。另外 ADR-0031 那套「指针认领」协议在这里跨了一次组件边界，变成
`in property pointer` + `callback pointer-claimed(int)`，因为 N 个子组件写两条
双向绑定指向父组件同一条 property，Slint 不让写。

**顺手修掉的一个真 bug**：`InsertBlockAfter` 以前把 `parent` 强制成 `None`，并且以
容器自己那一行作为落点。于是一个「+」或者「Paste below」可以把一个顶层块塞进容器
的子树里——而一页的块是按 `order` 排的一片平铺，**子树不连续的容器就不再是一个容器**：
`subtree()` 会走过去，所有按索引读的地方跟着一起错。现在它按锚点的 kind 分岔：容器
之后插在整棵子树之后，槽位（`TableCell` / `Column`）直接拒绝，其余地方继承锚点的
parent。这条同时把网格上同一个洞补了。另一个是「/」菜单的锚定写死了 350 px 的窗口
预留，这批加了一种块就溢出窗口下沿，改成按「+」菜单已有的办法量自己的高度。

**验证**：`cargo check --all-targets` 干净；`cargo test` 全绿 **288 passed / 0 failed /
4 ignored**；`cargo build --release` exit 0 且**零警告**。本 slice 新增的代表性测试：
`a_line_becomes_a_layout_that_holds_its_words`、
`adding_a_box_gives_it_a_line_and_deleting_one_reflows_its_words`、
`an_empty_box_takes_the_click_as_a_request_for_a_line`、
`editing_inside_a_box_stays_inside_it`、
`a_layout_copy_carries_its_own_boxes`、`undoing_a_layout_delete_brings_its_boxes_back`、
`a_box_is_not_a_prose_block`；`markdown_test.rs` 四条（按阅读顺序摊平、嵌套也摊平、
空 layout 导出成空、往返只丢形状不丢字）；`storage_test.rs` 的
`a_columns_layout_and_its_boxes_round_trip_through_storage`。

**RAM**（原始行 `benchmarks/results/2026-09-20-m10-columns-ram.jsonl`）：D 10 000
135.5–137.0 / 109.9–110.7 = **1.13×**，A 126.5–127.0 / 98.4–99.6 = 1.11×。门槛内，
但这是这批离门槛最远的一次。归因写得很清楚：投影只给 `kind == Columns` 的行建那两条
模型，其余行拿 `ModelRc::default()`——Slint 1.18 里它是 `ModelRc(None)`，
`i-slint-core-1.18.0/model.rs:891` 查过，**不分配**——所以一页没有 layout 时每行多
的只是两个 8 字节指针，一万行 ≈160 KB，而 D − A 涨了 ≈3.5 MB。算术到不了读数，
所以读数记成 drift 不认领。**而这已经是连续第四批同向漂移**（A 116.5 → 127.0，
+10.7 MB），于是门槛本身的方法成了结论：拿另一个 session 的矩阵当分母，比值里有一半
是 session。PERFORMANCE.md 里写下的是修法而不是放宽阈值——**下一条 kind 欠一个同
session 的 control build**（把 slice 前一个提交单独编到自己的 target dir，两臂一次测完），
ADR-0031 的投影保护就是这么定下来的。

**像素**：sweep 长到 44 个 scene（`.scratch/sweep12`）。`columns` 与 `columns-3`
是这对平铺场景，1 111 个差异像素全部落在 layout 自己那条带子里；另外三个菜单 scene
动了，因为列表多了一行。判法依旧是 `diffbbox.ps1` 的框，不是眼看 44 张图。

**未验证**：栏宽不能拖（Notion 的 resize handle 没做）、三栏以上不做、栏里再套 layout
不做；栏内 ↑/↓ 只动光标不出 layout，出栏靠点击。Markdown 导入侧本来就没有分栏语法。

**下一步**：批次 A 欠的三样（PDF 首页缩略图按用户指示仍推迟、剪贴板位图粘贴要在
`platform/` 里读 `CF_DIBV5`、孤儿附件的 GC），以及那条欠了两批的「一万块 + 一屏媒体」
滚动场景；批次 B 本身已收完。

## M8 tail · A4 收尾批 — 空页面、调色板分发、模态遮罩（2026-09-21，on `master`，ADR-0033 / ADR-0034）

M10 批次 B 收完之后，回到 M8 那条 A4 缺陷清单。这一批把里面**能靠代码关掉的四条**
关掉了，其中一条比它的标题严重得多。

**改动面**：`Command::AppendBlock { kind, text }`（`plan()` 里拒绝容器 kind，落点是
`page_blocks(page).last()` 的 order 之后）；`AppState::start_page()`；`UIState`
回调 `empty-page-started`；`Editor.slint` 空状态面板加 `TouchArea` + 换文案；
`on_title_commit` 里零行页面提交标题后顺手起一行；`PaletteAction` 枚举 +
`palette_action(id)` + controller 里那条**没有 wildcard arm** 的 `match`；
`AppShell.slint` 的 `modal-open` 与 `Colors.scrim`；Toggle Sidebar 从
`Control+B` 改 `Control+BackSlash`（`AppWindow.slint` 的 `KeyBinding`、Settings 的
`ShortcutRow`、palette 的 hint 三处一起改）；demo 页占位段落那句
「SQLite persistence lands with the next milestone」改成现在真实的行为。

**四条决定**：

1. **空页面自己造第一块，而不是在 create/open 时塞一块**。清单上写的是「文案过期」，
   实际是死路：每条插入命令都要一个锚点 `BlockId`，而 `create_page` 和 `open_page`
   都不播种子块，所以新建的页面**根本打不了字**。修法有三种——建页时塞一块、开页时
   塞一块、页面被明确要求写时自己长出来。前两种会让每次取消掉的「New page」在库里留
   一个空块，第二种还会让空状态永远不可达（并撞掉那条断言新页面投影 0 行的
   workspace 测试）。选了第三种：`AppendBlock` 是这条路的正门，一次撤销就能回到空。
2. **调色板的 id 空间在 Rust 里解码**。原来 controller 是 `match id { 1 => … }`，
   常量没 import 就在 pattern 位置变成 catch-all binding——`aaa3763` 之前 id ≥ 9 的
   每一行都在跑 `copy_current_page_markdown`，测试全绿，因为没有任何东西走这条分发。
   ROADMAP 里那句「durable repair 还没写」要的是一个覆盖命令注册表的测试，而测试要
   能存在，映射必须先是**一个函数**：`palette_action()` 返回枚举，`match` 不留
   wildcard，加一条命令忘了加一个 arm 就是编译错误。
3. **遮罩只给模态**。dialog / Settings / Add link 是模态，压 `Colors.scrim`；slash、
   「+」、⋮⋮、Move to、palette、search 是**锚定 popup**，故意不压——一个让你给某一行
   选类型的菜单，把那一行藏起来没有道理。锚定 popup 本来就靠 click-outside 自己关，
   全窗口那块 Rectangle 留着当 `UIState` 失同步的兜底，非模态时它是 transparent。
   规则写进 `docs/UI_ARCHITECTURE.md` §"Overlay layering"。
4. **Ctrl+B 只属于 bold**。同一个和弦在两处标着两件事，是清单上的 LOW；挪到 Ctrl+\
   而不是挪 bold，因为 bold 是别的编辑器都这么写的，而侧栏开关本来就少人记。

**验证**：`cargo check --all-targets` 干净；`cargo test --all-targets` 全绿 **293
passed / 0 failed / 4 ignored**；`cargo build --release` 零警告。本批新增测试：
`an_empty_page_takes_one_paragraph_and_undo_empties_it_again`、
`appending_lands_below_a_containers_whole_subtree`、
`a_grid_or_a_layout_is_not_a_bare_append`、
`every_palette_row_resolves_to_its_own_action`（走 `mock_commands()`，断言每行都
解析到**自己那条**动作、非页面动作互不重复、没有一行落到 `None`）、
`a_jump_row_carries_its_page_id_and_an_unknown_id_does_nothing`（0 / 14 / 9 999 / −1）；
`workspace_test.rs` 里 `create_then_open_page_lands_empty` 现在接着断言
`start_page()` 恰好给出一行、且 id 与返回值一致。

**像素**：`.scratch/sweep13` 对 `.scratch/sweep12`（44 scene）——38 张字节相同，
6 张动了，每张的 bbox 都能被这条改动解释：`empty` 724 个采样像素全在
x 506..1032 / y 456..466（只有文案那一行）、`palette` 7 个像素（hint 的和弦）、
`link` / `dialog` / `settings` / `dark-link` 整窗（遮罩；`settings` 另外因为多一条
`ShortcutRow` 长了一截）。行为不看像素，看那条注册表测试——**这次修的东西本身只动了
7 个像素**，正好是「绿的视觉门只证明画法、不证明行为」的现场教材。
`--scene empty --click 640,458` 单独一张（`.scratch/a4fix/empty-clicked.png`）证明
点击真的建出了带光标的聚焦首行。改完 demo 文案后再扫一遍（`.scratch/sweep14` 对
sweep13）：**0 / 44 动**，所以 ADR 里那句「这段字在任何 scene 里都不出现」是读数而不
是断言。

**RAM**：本批不欠 bench——没有新 kind、没有新 schema 列、没有新的每行状态，
`AppendBlock` 只在零行页面上跑。批次 B 欠的那个**同 session control build** 仍然欠着，
记在下一条 kind 头上。

**未验证**：`--click` 那张图没有走真实键盘，输入第一句话之后的换行/撤销链没测；
Enter-in-title 起行只测了「零行页面」这一支，非空页面提交标题不动块列表这件事靠的是
`row_count() == 0` 这个条件本身；遮罩在 dark 下几乎看不出来（背景本来就暗），好不好看
要人眼看一次；A4 剩下的 1 HIGH（带 inline mark 的段落不折行）、1 MEDIUM（find 在失焦
块上算得 16 次却一个都不画）、LOW（最暗提示行、最弱配色对）与 `renderer_name()` 那条
编译期限制，都不在这批。

**下一步**：先请用户手测一次（新建页面 → 点空面板 → 打字 → Ctrl+Z；Ctrl+\ 开合侧栏；
打开 Settings 看遮罩），然后进批次 C 的第一条。批次 C 的两条硬限制在开工前已经查清：
Slint 1.18 没有 `Text.rich-text`，语法高亮只能走已有的单行 runs 通道；TOC 的点击跳转
没有可靠机制（没有 focus 自动滚动，`ListView.bring-into-view` 假设等高行）。

## M10 批次 A · 剪贴板里的截图能粘进 Quire 了（2026-09-21，on `master`，ADR-0035）

批次 A 收尾。image 那一栏只剩一条写在 SPEC §三十七 和 CHANGELOG 里的空缺：
**从剪贴板粘贴**。这一批把它补上，顺带把 §二十七「clipboard rich content」的位图
那一半也交了。

**改动面**：新文件 `src/platform/dib.rs`（DIB → RGBA → PNG，里面没有一行 Win32）；
`src/platform/mod.rs` 加 `read_clipboard_image()`（`CF_DIBV5` 优先、`CF_DIB` 兜底，
和文字读取一样的五次 `OpenClipboard` 重试，按 `GlobalSize` 取字节）；
`AppState::paste_image()`；`on_rich_paste` 里「没有文字才轮到图片」那一支。没有新
依赖：`image` 的 png 编码器本来就是附件存储在用，clipboard 依旧手写 FFI（ADR-0025
的后半，不是推翻它）。

**四条决定**：

1. **解码与 FFI 分成两个文件**。这不是洁癖：DIB 的头、位掩码、调色板、行对齐、
   bottom-up 全是别的进程说了算，每一条都是解码器的坑；而这一切都不需要剪贴板。
   分开之后八个断言测试在任意机器上跑自己造的 DIB，既不碰用户真正的剪贴板，也不
   违反这台的规矩（不脚本点桌面 UI：不伪造按键、不抢前台）。唯一需要真剪贴板的那条
   是 `#[ignore]` 的。
2. **文字优先于图片**。富文本应用的「复制」会同时往剪贴板放 `CF_UNICODETEXT` 和
   位图，这时候粘出来的是段落还是段落的一张照片，是行为决定而不是实现顺序。段落变
   成图片就把可编辑的东西弄丢了，所以图片只在「根本没有文字」时才上场。
3. **截图的 alpha 是全零的垃圾字节**。32 位 `BI_RGB` 位图没有 alpha 通道，第四个
   字节通常是 0，而 PNG 会保留透明度——照抄过去就是一张**看不见的图片**，用户读到
   的是「粘贴失败」。修法只在「声明了 alpha 且整个 alpha 平面全零」时把整张涂成不
   透明，只要有一个字节非零就原样保留真实的透明。两条测试各钉一边。
4. **落点跟着光标，绝不覆盖已写的字**。空块直接*变成*图片，有字的块在下方得到图片，
   一次撤销一步——和「/」「+」那两扇门的形状一致，粘贴永远不在页面上留一行空壳。

**验证**：`cargo check --all-targets` 干净；`cargo test --all-targets` **302 passed /
0 failed / 7 ignored**（较 24a3432 的 293 增 9 条断言测试，ignored 从 4 到 7 是三条
打印型测量 + 一条读真剪贴板）；`cargo build --release` 增量 3m21s、零警告。新测试：
`dib.rs` 八条（bottom-up 翻转、行补齐、全零 alpha、真实 alpha、565 与 555 的掩码
区别、`biClrUsed`=2 与 256 色表、1 bit MSB 优先、截断与 RLE 载荷直接拒绝）、
`state.rs` 一条（在 ScratchDir 里开**真库**，断言 kind 与 id、附件名/MIME/字节数、
`attachments/` 下确有文件、下方段落文字仍在、撤销后引用没了字节还在盘上）、
`platform` 一条 `#[ignore]` 读用户剪贴板——它是唯一能证明 FFI 读到的是**别的进程**
写的字节的证据，本机跑出来 `clipboard picture: 1280x800 from 134168 PNG bytes`。

**性能**：这批欠的不是 RAM 臂（没有新 kind、没有新列、没有新的每行状态），而是一次
按键的延迟，所以量的是延迟，写进 `docs/PERFORMANCE.md`：DIB→RGBA 在 1080p / 4K 上
19 ms / 74 ms，存储那条腿（PNG 解码 + 降采样 + 缓存编码 + 两次写盘）在刻意不可压缩
的位图上 59 ms / 196 ms。两头都标了是地板还是天花板：单色图形的 PNG 编码只有 1 /
6 ms，真实屏幕内容比它贵；随机噪声比任何真实截图都贵。

**像素**：`.scratch/sweep15` 对 `.scratch/sweep14`，**44 张全部字节相同**。这批一行
UI 都没改，这条 0/44 就是那句话的读数而不是断言。行为不靠像素——那八个解码测试和
那条 state 测试才是门。

**未验证**：真人在 Quire 里 Ctrl+V 一张截图（Snip-and-Sketch 或 PrtSc）还没跑过，
这是本机规矩要请用户手测的一条；`GlobalLock` 那次拷贝本身没计时；4K 之外、以及
`CF_DIBV5` 带真实透明通道的应用（Photoshop 一层透明 PNG）粘进来长什么样，只有单元
测试的合成位图覆盖，没有真应用验证过。

**批次 A 的账现在只剩两条**：PDF 首页缩略图（2026-09-20 用户指示推迟，渲染路线待定）
和孤立附件的回收（没有外键也没有级联，删掉最后一个指向某段字节的块，字节就永久留在
`attachments/` 里）。另外「一页图片被滚动时」的实测从 image 那批一直欠到 columns 那
批，现在第四批仍然欠着，而且批次 B 欠的**同 session control build** 还没跑。下一步按
这个顺序走：先补媒体滚动的 bench scene（把那笔三次点名的欠账变成读数），再收孤立附
件回收，然后进批次 C——批次 C 六条里只有 math 不撞平台墙（高亮要单行 runs 通道、TOC
要跳转、bookmark / embed 要有 TLS 客户端），开工前再确认一次。

## M10 · 媒体 bench scene，和那条从来没滚动的 scene F（2026-09-21，on `master`，ADR-0036）

这批交付的是**量具**，不是功能。写它之前，从 image 那批起的每一批结尾都留了同一句话：
一页图片被滚动时的代价没测，32 MiB 解码上限只是「构造加上
一条单测」而不是读数。这批把它变成读数——然后发现那个读数本来就不成立。

**改动面**：`HandleArgs` 多一个 `pictures`，`AppState::new` 在**空库**时用
`create_fixture` 播 200 张 1280×720 的 PNG 进 `attachments/`，`bench_pictures` 把 bench
页每 `stride` 行变成 image 块；`attachment_cache_peak` + `attachment_cache_report()`
让应用自己在 `--dump-state` 时往 stderr 打一行 JSON（`bytes` / `peak_bytes` /
`entries` / `budget` / `scroll_y`），`bench.ps1` 把它内联成 jsonl 的
`attachment_cache` 字段；`main.rs` 多 `--pictures` 与 `--scroll-step`，
`quire_shot` 多 `--scroll-y`；`bench_matrix.ps1` 加四个 D+F·P scene，跑前清掉
`attachments`。零 UI 改动，零 schema 改动。

**四条决定**：

1. **缓存由应用自己报，不由进程计数器猜**。工作集看得见 Slint 的路径缓存和纹理上传，
   看不见我们那个以「一张栅格」为单位键的 LRU；一个看不见的上限不可能被读数否定。
2. **fixture 只在库是空的时候写**。否则 measured pass 计的是 PNG 编码而不是图片。
   这条不是靠注释保证的：`the_pictures_scene_seeds_its_pool_once_and_then_only_loads_it`
   在两次 `AppState::new` 之间**删掉一个文件**，如果第二次把它写回来，那条断言就是为
   错误的原因通过——所以它测的是加载路径而不是计数。
3. **池上限 200，不是每张一个文件**。5 000 张各不相同的 1280×720 PNG 会让 seed pass
   变成磁盘与预热的人质，而缓存分辨不出区别——它按字节权重淘汰，不按身份。图片行 k
   取 fixture `k % 200`，相邻图片行仍是不同栅格，这正是缓存要按之定容的东西。
4. **stride 和 pool 只有一个来源**（`bench_picture_plan`），建行的和解种的都是问它，
   两处不会漂。

**真正的发现：scene F 从 M2 起就没滚过。**先不当它是坏仪器、也不当它是阴性读数——
前两批跑出「5 000 张图的 10 000 行页，8 秒后缓存没动、`scroll_y` +840」，把步长从
8 调到 200 反而**每秒移动得更少**，一个慢渲染器不会这样。于是报告里加 `scroll_y`，
`quire-shot` 里加 `--scroll-y`，用像素把它钉死：`--scroll-y 0` 和 `--scroll-y 2000`
产出**逐字节相同**的 PNG，只有 `-2000` 不同。Slint 的列表 content offset 向下为负
（它自己按 `round(-content-y / item-height)` 算首个可见行），而 M2 装的定时器一直在
**加**。这就是为什么之前两批 36 行里凡是 F 的行都不作数：删掉、在修好的二进制上整批
重跑成 27 行；污染的那份留在 `%TEMP%\media-ram-pre-fix.jsonl`，作为「错的是仪器不是
应用」的记录。修法除符号之外还要一个 stall 计数——ListView 只知道自己已经 realized
的行有多高，所以 clamp 比滚动慢一帧，一个卡住的 tick 不等于到底了。

**读数**（`benchmarks/results/2026-09-21-m10-media-ram.jsonl`，九个 scene 一个 sitting，
详见 `docs/PERFORMANCE.md` 末节）：32 MiB 预算**按字节权重停在 9 张栅格**，
33 177 600 / 33 554 432 = 98.9 %，而且 500 张、5 000 张、1 000 行、10 000 行四个形状
停在同一个数字上——上限在视口能显示第十张之前就已经用完，所以它不需要抬；一页你没滚
过去的图片和没有图片无法区分（0 次解码，D+500 = D）；而**屏幕上**一张图片约值 9 MB
进程内存，不是它栅格的 3.7 MB——所以约束照片页的是视口不是缓存，这条进了 §二十二
该带的算术。gate 本身同一 session 重跑：D 1.12× / A 1.10×，媒体臂**故意**留在 gate 外。

**验证**：`cargo check --all-targets` 干净；`cargo test --all-targets` **305 passed /
0 failed / 7 ignored**（比上批 302 多三条）；`cargo build --release --all-targets`
零警告。新测试：plan 的四点（0 行、`--pictures 0`、两个真实 scene 的 stride、要的比行
数还多不能凭空造行）、250 张图落在 1 000 行上的形状（stride 不是 off-by-one、图片行
是唯一带附件的行、栅格互不相同、池耗尽才 wrap、图片行没有文字可排）、上面第 2 条那条
种子/加载。全程 headless：真进程 bench + `quire-shot`，没有伪造按键、没有抢前台。

**像素**：`.scratch/sweep16` 对 `.scratch/sweep15`，**44 张全部字节相同**——这批一行
UI 都没动。但这批真正的像素证据是 `--scroll-y` 那两张：同一份代码、两个都「合法」的
偏移、一张图都不同才叫滚动。绿门只证明没画坏，不证明量具在量。

**同日跟进 · 把 verdict 欠的那半句也量了**：修好的 scene F 不能只有一边有数。
`benchmarks/scripts/scroll_ab.ps1` 一次 sitting 里跑两个二进制（`target\release` 与
`--no-default-features --features skia-opengl --target-dir target-skia` 的那一个），
scene 在外、arm 在内，让机器漂移同时移动一对里的两边；开工前**两臂各自自证身份**——
读自己 `first_paint` 行里的 `renderer`，不对就整批停，因为一个忘了关默认的 skia 构建
会渲染成 femtovg，把 A/B 变成同一个二进制跟自己比。结果（`benchmarks/results/`
`2026-09-21-m10-scroll-skia.jsonl`，每臂一次 seed + 两次 measured）：方向留着，尺寸
缩水——skia/GL 滚起来确实便宜，滚轮 ≈18 %、flick ≈21 %，不是 M7 那行声称的三分之一
（那两个数都是在页面没动的情况下量的），代价是 +34…43 MB 工作集，而它的 private
字节在 flick 臂上反而比 femtovg 低 6 MB（惩罚在共享的 GPU 映射页，不在堆上）。所以
**femtovg 仍然是默认**，理由跟以前一样，只是「scroll 重负载可以换 skia」从继承来的
句子变成了证过的、更小的报价。另一条意外：skia 臂报出的仍是 **9 张栅格 /
33 177 600 peak**，和 femtovg 一模一样——LRU 在 `AppState` 里，在任何渲染器之上，
所以 §三十七 的媒体上限不随渲染器动。控制点：本批 femtovg 臂（150.4–152.0 MB /
图片页 78.0–78.5 %）复现了当天早上那一批独立跑出的（150.0–150.8 MB / 77.4–78.6 %）。

**量具的第二个 bug，是在补 scene 时撞出来的**：把媒体批次里两条手跑的 arm 加回
`bench_matrix.ps1` 之后，用 `-Only A,B` 去只跑它们，脚本**什么都没写、退出码 0**。原因
是 `powershell -File` 把逗号列表当成**一个字符串**塞给 `[string[]]` 参数，于是一个 scene
都没匹配上。现在它自己切 token，并且在「一条都没匹配」时 throw——过滤器匹配不到东西不是
一次测量。顺手把两条 scene 补齐（文本页的 flick、1 000 行的图片页），发布过的每一行现在
都能从脚本重跑；两条都验过：156.3 MB / 85.78 % 与 184.2 MB / 9 张栅格，和早上那两条独立
跑的对上。

**未验证 / 欠账**：真实照片的熵
（fixture 是渐变，而且 `create_fixture` 不随 id 变像素，所以 200 个名字后面是同一张
图——这里的解码数字全是地板）、HiDPI、图片夹在段落之间而不是等距排布；孤立附件回收仍
欠；批次 B 欠的**同 session control build**——这批在同一 sitting 里对照了 A/D/媒体三个
形状、也对照了两个渲染器，但那不是「切片前一版编译出来对照」，仍然欠到下一条 kind 头
上。skia 的 typing WS 漂移（M7 那条 320 MB vs 128 MB）没解释也没重跑，默认是 femtovg
期间它不咬人。

**下一步**：请用户手测两件事——截图后 Ctrl+V 进 Quire，以及现在这台机器上真实滚动的
顺不顺（scene F 修好之后，长页滚动第一次有了诚实的 CPU 数：8 px/帧约 37…43 %，
200 px/帧约 86…87 % 单核，两个渲染器都是这个量级。CPU 数不是流畅度，harness 看不见
掉帧，所以这一条只能由人眼定）。之后按 docs 上剩的账走：孤立附件回收，或进批次 C 的
math——批次 C 六条里只有 math 不撞平台墙。

