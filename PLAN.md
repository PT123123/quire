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


## M10 批次 A · 孤立附件回收：Settings → STORAGE → Reclaim（2026-09-21，on `master`，ADR-0037）

批次 A 剩的两条账，这次收的是**孤立附件**那条（PDF 首页缩略图仍按 2026-09-20 的用户指
示推迟）。

**改动面**：`Change::AttachmentDeleted { id }`（`core/persistence.rs`，只有回收会发它，
任何命令计划都不会），以及新的单一读者 `attachment_ids_in()`；`Document::all_blocks()`；
`History::referenced_attachments()`（历史模块第一次对外界交代栈里有什么）；
`SqliteRepository` 的 DELETE 分支；`AttachmentStore::remove()`（`file` 与 `thumb` 都处
理，`NotFound` 不算失败，删不掉的文件名回传给提示条）；`AppState::reclaim_attachments()`
+ `evict_image()`；`controller.rs` 的 `on_reclaim_attachments`；`ui/Types.slint` 一个
callback；`SettingsDialog.slint` 的 STORAGE 行多一个 **Reclaim** 按钮和一行说明。没有迁
移，schema 停在 v8。

**四条决定**：

1. **可达集合故意比屏幕上有的宽**：所有页面的所有块 ∪ 每一页 undo **和 redo** 栈里的每
   一个附件 id ∪ 复制板上那一块。因为两个失败方向不对称——留下孤儿只损失磁盘，删掉一张
   Ctrl+Z 正要还原的图损失的是用户的数据。
2. **顺序就是内容**：先 `force_flush()` 把防抖队列写空，再同步删行，最后删文件。反过来
   做，一条还躺在队列里的 `AttachmentAdded` 会在 DELETE 之后重放，造出一行**指向已经不存
   在的文件**的永久引用。写失败就整体放弃；删文件失败只是留下没有行认领的字节，下次扫描
   还会清掉。
3. **绝不列目录**。这条看着最省事、也最致命：如果这一次 `load_attachments` 失败，内存里
   的账本是空的，磁盘上每个文件都「没人引用」，于是「我看不见引用」被回答成「把它们全删
   了」。回收只读账本，所以加载坏掉时它删不掉任何东西。代价写在 Known limitations：**没
   有行的文件回收不了**。
4. **100 步上限就是承诺的边界**，不写「永远」。`both_stacks_protect_and_the_cap_ends_the_
   protection` 把这条边界钉住（100 步全保，第 101 步松开最早那个 id），而按钮旁边那一行
   字必须先被读到——提示条是一次 toast，不是一句警告。

**验证**：`cargo check --all-targets` 干净；`cargo test --all-targets -- --skip
clipboard_write_and_read_round_trip_unicode` → **312 passed / 0 failed / 8 ignored**（剪
贴板那一条见下；它跑起来的 session 就是 313 passed / 8 ignored，比上批 305 多 8 条断言 +
1 条打印型计时）；`cargo build --release --all-targets` 零警告。**一条与本切片无关的失
败**：`platform::tests::clipboard_write_and_read_round_trip_unicode` 现在这台机器上过不了，
`copy_to_clipboard` 返回 false。原因不在我们的代码里：一段**不经过本仓库任何函数**的 C#
探针（`user32!OpenClipboard`）在这个 shell 里连续 12 秒每次都是 `ERROR_ACCESS_DENIED`
（5）、`GetClipboardOwner()` 为 0，也就是当前有别的进程独占着剪贴板；而 `git diff` 证明
`src/platform/*` 这批一行都没改。所以它是这个 session 的失败，不是构建的失败——Quire 自己
的 Ctrl+C 现在同样会静默失败，这一点值得用户留意（不是这批引入的）。新测试：`history.rs`
1 条（两条栈都保护、id 只数一次、CAP 收尾）；`state.rs` 6 条，
全部在 `ScratchDir` 里开**真库**——撤销能还原的图不动、重启之后新 session 能回收前任删掉
的那张、复制板上的图在它那一页被删后再活下来、同一 session 里删整页即可回收（不用重启）、
行还在文件已经没了的孤儿也把行删掉并且报出那个文件名、内存态 session 直接答「无可回
收」；`storage_test.rs` 1 条（DELETE 幂等，且删行不碰块上那条悬空引用）。控制断言：先证
明这一页**确实带着那一行图**再断言它活着，否则「什么都没删」会因为「什么都没加载」而通
过。

**性能**：这批欠的不是 RAM 臂（没有新 kind、没有新列、没有新的每行状态，bench scene 看
不见它），而是**一次点击在 UI 线程上冻多久**——回收是这台应用里唯一一个在 UI 线程上扫磁盘
的功能。所以量的是那个：`a_reclaim_of_a_thousand_orphans_is_timed`（`#[ignore]`，打一行
JSON）在 `ScratchDir` 里造 1 000 张「有行、有文件、没有任何块指向它」的附件，再计时。存进
`benchmarks/results/2026-09-21-m10-reclaim-timing.jsonl` 的那三个 sitting：**549.9 /
556.7 / 567.6 ms**，每张孤儿约 0.55 ms；另有一批独立先跑的读数 530.6 / 542.3 /
573.0 ms，批间漂移和批内一样大。同一句调用在空账本上是 **0.0 ms**，所以可达性扫描没有在时
钟上，那 0.55 s 全在「1 000 条 DELETE 装一个事务 + 1 000 次 `remove_file`」里，两者谁占多
数没归因。控制断言在数字前面：先证文件夹里确有 1 000 个文件、表里确有 1 000 行，扫完再证
两者都为 0——否则「快」和「这里本来就没东西」无法区分。顺带一个仪表器的教训：第一次跑它用
了 `--exact` 加裸测试名，**一个测试都没匹配、打印 0 passed、退出码 0**，看着像绿了。写进
`docs/PERFORMANCE.md`。

**像素**：`.scratch/sweep16` → `.scratch/sweep18`，44 张里只有 `settings.png` 变，而这一变化正是这次要加的按钮。中间那一版（sweep17）是一次真缺陷，而且是像素抓出来的：同一个
flag 下第三个 `visible:` Button 和被隐藏的说明 Text 都**留在布局里占位**——隐藏的两行文
字吃掉约 40 px 高度，隐藏按钮抢走标签宽度，于是「Running in memory — no database
attached」被截成三个词（bbox `x 440..838 / y 0..798`，10 908 px）。改成条件子元素
（`if UIState.storage-available : …`）后消失，规则写进 `docs/UI_ARCHITECTURE.md` §"Slint
geometry traps"。headless 拍不到新按钮本身：`quire_shot` 建的是 `AppState::new(&args,
None)`，`storage-available` 恒为 false。

**未验证**：①**真人点一次 Reclaim**。headless 拍不到这个按钮——`quire_shot` 建的是
`AppState::new(&args, None)`，`storage-available` 恒为 false，所以带真库的设置对话框只能由
人眼确认（按钮在不在、那一行说明读不读得清、提示条报的数字对不对）。②真实库里孤儿的形
状：计时用的是 1 000 个 24×18 的小 PNG（173 KiB 总），文件名与数量都对，但真实附件的**大
小**分布没有进过这条测量；删文件的代价随数量走、不随大小走，所以这条影响的是磁盘回收的
字节数而不是延迟。③Windows 上「文件被别的程序占用」那条分支（`remove` 返回卡住的文件
名）只有单元测试的合成形状，没有真的锁住一个 `.png` 再扫一次。

**批次 A 的账现在只剩一条**：PDF 首页缩略图（2026-09-20 用户指示推迟，渲染路线待定）。
另外批次 B 欠的**同 session control build** 仍然挂着——回收这一批不改任何 delegate，所以
这次没有把它再欠一遍，但也没有还。下一步按 SPEC §三十七 进批次 C（highlight / bookmark /
embed / math / TOC / synced block），批次 C 六条里只有 math 不撞平台墙（高亮要 inline runs
的单行通道、TOC 要跳转、bookmark / embed 要有 TLS 客户端），开工前再确认一次。

## M11 批次 C · slice 1 — math：公式存源，图片是导出路上算出来的（2026-09-21，on `master`，ADR-0038）

批次 C 六条里那条不撞墙的。SPEC §三十七 要「LaTeX 子集」并把标准写成「渲染优先 Unicode
近似排版」，同时规定**引入排版引擎必须先出 ADR 并附内存数字**——走 Unicode 近似就跨过了
标准而没有触发那道闸：没有引擎，也就没有欠的内存数字。

**改动面**：新文件 `src/core/math.rs`（`to_unicode`，唯一的渲染器）；`core/mod.rs` 挂模块；
`core/types.rs` 的 `BlockKind::Math`（`ALL` 20→21）和 `MarkKind::Math`；`app/state.rs` 的行
号 20、slash / insert 菜单各一行、以及 `build_runs` 在**投影**时把公式 run 换成字形；
`app/controller.rs` 的 `"$$ "` 行首快捷键、`on_math_render`、`on_toggle_mark` 的 arm 4、两
个新 bench scene；`services/import_service.rs` 的 `$$ … $$` 围栏 + 行内 `$…$` + TeX 的
flanking rule；`services/export_service.rs` 的围栏、`dollar_pair_ahead` 转义与 span 排序；
`ui/Types.slint` 一个 `pure callback math-render(string) -> string`；`EditorBlock.slint` 的
公式框；`SettingsDialog.slint` 的快捷键行。**没有迁移**，schema 停在 v8：kind 与 mark kind
在库里都是字符串（ADR-0030 那套论证），没有 CHECK 列表要放宽，而未知 kind 依旧是「读到就
报损坏」，所以旧构建打开一个带公式的库是响亮地失败，不是悄悄丢行。

**七条决定**：

1. **存源不存字形**。`blocks.text` 里是 `\frac{a+b}{2}`，不是 `(a+b)/2`。这让渲染器可替换：
   将来引擎到位是换一个函数，之前写下的每个库都不需要迁移、重导出或重建搜索索引；它也保住了
   `text <=> UIState.editing-text` 双向绑定的诚实（用户编辑的是公式，永远不是它的图片），以及
   Markdown 的往返（而不是一次性渲染）。
2. **渲染器三条契约**，测试钉的就是这三条：输出**永远不丢用户打下的东西**（未知命令原样回
   来，最坏读成「这条没渲染出来」而不是「这条不见了」）；源里的空格**是内容不是语法**（TeX 在
   数学模式丢空格，这里不丢——在单行近似里，用户打的那个空格是他间距唯一的幸存表达，
   `\alpha + \beta` 和 `\alpha+\beta` 故意不同）；**幂等**（所以每行每次绑定求值都能重derive而
   不漂移）。
3. **派生绑定是按元素的成本，不是按 kind 的**。`visible: false` 的 `Text` 照样求值它的绑定，
   所以公式那行读 `is-math ? UIState.math-render(…) : ""`；不加守卫，一页 10 000 行会为了 1
   行公式调用渲染器 10 000 次。行内公式同理，但走的是投影而不是绑定：绑定是每帧重付。
4. **公式 span 是它那段字节上最外层的东西**（`kind_order` / `mark_order` 里 Math = 5）。被它
   包含的 mark 在导出时丢掉，与它**共享边界**的 mark 也丢掉——因为 `$**a**$` 会把两颗星导进公
   式源里；首尾带空格的源没有 `$…$` 写法（importer 要求两侧都是非空格），于是 mark 走、字留。
5. **`$` 的守卫是成对的，两半是同一个谓词**。import 只开 TeX flanking rule 允许的对（紧跟非空
   格、闭合前非空格），所以「costs $5 and $10」是散文；export 只在真的会重新成对时才转义
   （`dollar_pair_ahead`），所以散文里的美元符号不必每个价格前面挂一个 `\$`。`$$ … $$` 围栏像
   代码围栏一样逐字，因为 `\alpha` 必须带着那一个反斜杠回来。
6. **空公式也要看起来是个公式**：无文本时渲染 `$$`，行有高度、读起来是一个槽位而不是一条空白
   带；而 kind 仍在可编辑集合里，所以点它就把源开在活的 TextEdit 中、渲染 Text 自己让位。
7. **21 个 kind 这条事实是核对过的**：`BlockKind::ALL` 从 20 变 21，而 `docs/ROADMAP.md` 里
   那句「16 个 kind」是 columns 批次留下的旧账，一并改到 21。

**顺带抓到一个真缺陷**：`math-inline` 是这一集里第一条**短到在自己框里留下余量**的带 mark 的
行，而那个余量把三处 run 行的布局问题照出来了——`HorizontalLayout` 的默认 `alignment` 是
`stretch`，每个 item 的 `min-width` 钉在 `Text.preferred-width`、`horizontal-stretch: 0`，这套
读起来像「用自然宽度」但**不是**：一处 stretch 都不给，多余宽度照样被摊到 item 之间。结果是三
条 run 之间 155 px 的空隙。控制断言不是假设：改成 `start` 之后，基线里 44 张 scene **一字节都
没动**（`marks`、`table-edit`、两张 `columns` 全部 byte-identical），因为它们全都溢出自己的框、
从来就有余量可浪费。两条 Slint 陷阱（默认 stretch、`visible: false` 不挡求值）都写进
`docs/UI_ARCHITECTURE.md` §"Slint geometry traps"。

**验证**：`cargo check --all-targets` 干净；`cargo test --all-targets -- --skip
clipboard_write_and_read_round_trip_unicode` → **335 passed / 0 failed / 9 ignored**（比上一批
的 312 / 8 正好多这次的 23 条断言 + 1 条打印型计时；跳掉的那条是本 session 环境的剪贴板独占，
见上一批的记录）；`cargo build --release --all-targets` 零警告、4 m 11 s。新测试：`core/math.rs`
15 条（希腊字母 / 关系符 / 分式 / 根式 / 上下标 / 环境 / 未知命令原样 / 幂等 / 空格是内容 / 空
公式…），`markdown_test.rs` 9 条（两种围栏形状、行内往返、含两个 `$` 的散文、导出转义、公式吞
掉别的样式、首尾空格降级、双形状 import）。控制断言在关键处：「散文里两个美元」那条先证明它
**没有**产生 mark，否则一个什么都没解析的测试也会绿。

**性能**：`to_unicode` 五例平均 **658.5 / 625.8 / 608.3 ns**（release，200 000 轮 × 5 例，
`#[ignore]` 打印型）⇒ **≈0.61 µs 一条公式** ⇒ 每行都挂一个公式的 10 000 行页，一次投影的上界
≈6 ms（10 000 × 0.61 µs 的算术，**不是量出来的帧**：仓库里从来没有一条整页投影的读数，最接近
的两个各自都不是它——打字那行的按键处理中位数 47–66 µs 是一个事件，存储那行的去抖 32 变更
`apply` 中位数 3.42 ms 是一次 SQLite 提交）。6 ms 是一句「渲染器最坏能被记多少账」的上界，
也是「不在换行的行内渲染公式」和「转换放在投影里」两个决定的理由：那样一页一秒重画 60 次，放
在绑定里就要把 6 ms 再乘 60。RAM 闸这次**带了自己的 control**（批次 B 欠的方法账，
现在还）：把前一个 commit `2e9de99` 在 `git worktree` 里编出来（`git status --short` 为空、树
里没有 `src/core/math.rs`），两个 exe 在同一 sitting 里交替跑 scene D——control 135.7 / 135.8
WS · 109.8 / 110.7 private，math 136.6 / 136.7 · 111.7 / 112.4，**1.016×**（闸 ≤1.2×）。种子
那次（143.3 / 143.7）不进稳态。两臂各自自证身份（大小 + md5），所以谁也不能冒充谁。那 +1.8 MB
比臂内 0.9 MB 的散布大，所以**记下而不抹平**：exe 本身大了 56 KB，其余推测是两个 `match` 各多
一个 arm 加符号表，未归因。同一 scene 的空闲 CPU 这次读 2.5–4.5 %、媒体批次读 0.0–0.2 %，这也
是闸只按臂间比值讲、从不报绝对值的原因。数据在
`benchmarks/results/2026-09-21-m10-math-ram.jsonl`，论证在 `docs/PERFORMANCE.md`。

**像素**：`.scratch/sweep18` → `.scratch/sweep21`，44 张里动 4 张、新增 2 张（`math`、
`math-inline`），40 张 byte-identical。四个 bbox 全在改动能解释的带内：`slash` 621 px 与
`dark-slash` 2 483 px 同 `x 340..618 / y 532..586`（菜单多了「Math — or type $$」那一行）、
`plus` 10 983 px 覆盖 `x 600..878 / y 0..798`（插入菜单整列高了一行，所以它画的每一行都位移）、
`settings` 77 px 在 `x 548..724 / y 490..500`（快捷键行现在写 Ctrl+B / I / E / M）。46 张是新的
基线。这一批**不是**靠肉眼判的：先算哈希、只重看变化的 6 张。

**未验证**：①**人眼**看一个光标下的 Math block（活 TextEdit 里编辑源）和选中文字按 Ctrl+M——
`quire_shot` 从不聚焦某一行，两张新 scene 因此都是渲染态，编辑态没有像素。②真实 LaTeX 语料：
「常见子集之外一律原样回来」是策略不是验证，`\begin{aligned}` 那类多行环境没有排版。③代码高
亮那条硬要求（长代码块打字延迟不可测）还没测过——批次 C 的下一条若走同一条 runs 通道，会直接
复用这次的 ≈0.61 µs/次的结论。

**批次 C 还剩**：highlight / bookmark / embed / TOC / synced block。上一批结尾那句判断要改一
半：**TOC 的墙已经不在了**——`quire://block` 锚点在应用内跳转从 M8 就能用，缺的只是收集标题
和刷新，跟 §二十五 的快照机制没有冲突；highlight 撞的是 runs 的单行通道（这次没动它，但公式
run 走的就是那条道）；bookmark / embed 仍然要有 TLS 客户端，而 SPEC §二/§三十三 禁 WebView 与
JS 运行时，所以它们需要一条不含网络的形状（embed 至少要能画一张占位卡）。建议顺序：TOC → 代码
高亮 → embed 占位卡 → bookmark → synced block。

## M11 批次 C · slice 2 — TOC：目录不存内容，每次投影从页面上现算（2026-09-21，on `master`，ADR-0039）

批次 C 六条里的第二条。上一条结尾那句判断（「TOC 的墙已经不在了」）这次得到验证：它确实
只是「收集标题 + 在投影里出一份列表」，`user_version` 依旧停在 **8**，一行文档代价是
`blocks.kind` 里的一个字符串 `"toc"`。

**改动面**：`core/types.rs` 加 `BlockKind::Toc`（int 21、库里 `"toc"`、`ALL` 21→22）；
`ui/Types.slint` 加 `struct TocEntry { block, label, level }`、`BlockRow.toc-entries` 一条
列表字段、`callback toc-jump(int)`；`app/state.rs` 的 kind↔int 两个 arm、`BLOCK_TOC`、slash
与 insert 菜单各一行、`project_blocks` 把 `visible_block_indices(blocks)` 提到函数开头（原来
只在一处用）、新增 `fn toc_entries(blocks, shown)`，以及 mock `block()` 补字段；
`app/controller.rs` 注册 `on_toc_jump` 与 `"toc"` bench scene；`EditorBlock.slint` 的
`is-toc`、可编辑集合排除、上边距、`body-height`、指针光标，和那个 `toc-list` 委托；
`services/export_service.rs` / `import_service.rs` 的标记行；`benchmarks/scripts/sweep.ps1`
多一张 scene。**没有迁移**，论证同 math 那批（kind 在库里是字符串，未知 kind 是「读到就报
损坏」，所以旧构建打开带目录的库响亮地失败，不是悄悄少一行）。

**六条决定**：

1. **目录是派生的，一行都不存**。列表每次投影现算，于是改标题就改目录、删标题就少一行、
   折叠起来的标题自动不在目录里——不是「同步」出来的，是同一份数据。反过来若存快照，就要
   发明刷新时机，而那一时刻一到，目录和正文必然有不一致的时候。
2. **可见集合是借来的，不是自己算的**。`toc_entries` 走的是投影自己已经算出来的
   `visible_block_indices`，所以「折叠的标题不进目录」不是加的一条过滤，而是复用同一份
   「这一页现在能显示哪些行」。同一函数顺带把 block-tree 与容器两种隐藏一起解决了。
3. **Markdown 只写一行 `<!-- quire:toc -->`**。把 1 000 条目录行导进文档，等于把派生数据
   连同本库内部的 block id 一起塞进一个跨库交换格式——导到别的工具里是死字，导回来还会
   造出一批指向不存在 block 的链接。
4. **目录行可选、不可编辑；转换进来的那行文字留着不画**。前者因为它没有自己的文字可改，
   点它是选那一块；后者走 divider 的先例，所以 Turn into 来回不丢字。空标题给 `"Untitled"`
   而不给空行：空行没有可点的东西，但那一行仍然是个落点。
5. **点击只移动光标，不滚视口**——这条是先查证再写的。Slint 1.18 的裸 `ListView` 没有
   bring-into-view（只有 `StandardListViewBase` 有，见
   `i-slint-compiler-1.18.0/widgets/common/listview.slint`），而 `i-slint-core` 里没有任何
   代码在焦点变化时挪动 `Flickable` 的 viewport。M8 的 `quire://block/` 锚点早就带着同一个
   限制，所以这是已知边界，不是这里新造的坑；`reveal`（把目标行滚进视口）单独排成一条
   slice，两个入口一起受益。不在这次偷偷塞一个靠估算的滚动条位置。
6. **`toc-jump` 不走 `open-link`**。目录行标的就是本页的 block，id 已经在行数据里是个
   int；套 `quire://block/` 那条链要先拼字符串再解析回来，还顺带过一次页面查找。路径复用
   `flush_pending_edit` → `set_editing_id(-1)`（重建委托，让 TextEdit 接管）→ `focus_block`。

**顺带修的两处 UI 形状**：目录块的 `top-margin` 组里漏了 kind 21（跟在其他非文本块后面补
上），以及 `toc-jump` 之后 `body-height` 得由 `toc-list.preferred-height` 给——一个
`Rectangle` 不能拿子 `Text` 的高度来定尺寸（和 Rectangle 默认 100 % 宽度互为循环），所以每
行高度用的是字体度量加 `overflow: elide`。

**验证**：`cargo check --all-targets` 干净；`cargo test --all-targets -- --skip
clipboard_write_and_read_round_trip_unicode` → **340 passed / 0 failed / 10 ignored**（比上一
批的 335 / 9 正好多这次的 5 条 + 1 条打印型计时；跳掉的那条是本 session 环境的剪贴板独占，
见上一批的记录）；`cargo build --release` 零警告。新测试：`state.rs` 2 条（一页里有 H1/
折叠 H2/H3/空标题，目录读出 `(2,"Top",1) (5,"Second",2) (6,"Untitled",3)`——**控制断言是
那个折叠的 H2 不在里面**，同时每一条非目录行的列表都得是空的，否则派生就漏到了别的块上；
另一条改标题后目录跟着改，证明确实没有拷贝）、`markdown_test.rs` 3 条（导出只有标记行、
标记行不吞邻行、往返之后不带目录的副本）。

**性能**：RAM 闸按上一批立的规矩重跑了一遍——前一个 commit `cc7ccf0` 在 `git worktree` 里
编出来（`git status --short` 为空），两个 exe 同一 sitting 交替跑 scene D，各自先用 md5 +
大小自证身份（control md5 `a81ce6df…` 21 928 960 B，本树 `f6d9a576…` 22 017 024 B）：
control 稳态 135.9 / 137.0 WS · 110.5 / 112.1 private，toc 137.0 / 137.3 · 111.6 / 112.5 ⇒
**1.007×**。而且和 math 那批不同：两臂均值之差 0.75 MB **小于 control 臂内部的 1.6 MB
散布**，所以这次连「小成本」都不能说，只能说量不出来。种子那次（112.7 / 110.0）不进稳态。
exe 大了 88 064 B：一个 enum arm、一个 struct、一个 callback、一个委托。
闸看不见的那半单独量了：**scene D 里没有目录块**，所以 RAM 比值讲的是「多一个 kind」的
账，不是「多一条列表」的账。`cost_of_one_contents_block_on_a_ten_thousand_row_page`
（`#[ignore]`，release）对 10 000 行、每十行一个标题的页面计时 `project_blocks`，行 0 分别
当段落和当目录（1 000 条），三轮 39.12 / 37.40 / 37.17 vs 37.71 / 37.85 / 38.18 ms ⇒
差值 −1.42 / +0.45 / +1.01 ms，**和付账那一臂自己的轮间散布分不开**；能给 1 000 条派生
条目设的上界是 ≈1 ms，基数是同一投影本身的 ≈38 ms。顺带说一句：**≈38 ms 是仓库里第一条
整页投影的读数**——上一批引 ≈6 ms 时明说那是算术不是量出来的，因为从来没人量过公式落进
哪次投影；两个数一起读，问题的形状变了：每条公式的成本是一次投影的百分比，而一次投影不是
一帧。这条是 Rust 侧数字（block 进、`BlockRow` 出，不 realize 委托、不重绘），对「10 000
行画出来要多少」一个字没说，已公布的 UI 数字照旧（按键处理中位数 47–66 µs、32 变更去抖
`apply` 中位数 3.42 ms）。数据 `benchmarks/results/2026-09-21-m11-toc-ram.jsonl`，论证在
`docs/PERFORMANCE.md`。

**像素**：`.scratch/sweep21` → `.scratch/sweep22`，46 张里动 3 张、新增 1 张（`toc`），43
张 byte-identical。三个 bbox 全在改动能解释的带内：`slash` 683 px 与 `dark-slash` 2 574 px
同在 `x 340..618 / y 564..618`（菜单多了「Table of contents」那一行）、`plus` 1 249 px 在
`x 612..864 / y 624..792`（插入菜单多一行）。`settings` 这次没动——快捷键没加。47 张是新
基线。新 scene 走的是**把样例页第一段转换成交集块**，而不是往 `mock_blocks_sample()` 里加
一行：那样 ~30 张默认页的 scene 才会原样不动，而「43 张 byte-identical」就是这句话的控制
断言。这一批同样不是肉眼判的：先算 manifest 哈希、只重看变化的 4 张。

**未验证**：①**人眼**点一次目录行——hover 变色、光标落到目标标题末尾、以及「不滚动」在真实
长页上的观感，`quire_shot` 从不聚焦某一行，所以点击路径一个像素都没有。②`reveal`（把目标
滚进视口）没有实现，这条限制只是被复述、没有被解除。③中文长标题 elide 成一行后的读感。
④一页几百个标题时目录块自己的高度（列表用 `preferred-height`，会顶开整块，但没在真实语料
上看过）。

**批次 C 还剩**：embed / highlight / bookmark / synced block。建议顺序仍是 embed 占位卡 →
代码高亮 → bookmark → synced block：embed 只需要一张不含网络的卡（SPEC §二/§三十三 禁
WebView 与 JS 运行时），而高亮那条硬要求（长代码块打字延迟不可测）大概会复用 math 与这次
的同一份 runs 通道。`reveal` 不属于批次 C 的任何一条，但 TOC 和 `quire://block` 锚点现在都
在等它，谁做谁受益。

## M11 批次 C · slice 3 — embed：卡片只存地址，两行是画的时候现算的（2026-09-21，on `master`，ADR-0040）

批次 C 六条里的第三条，兑现的正是上一批结尾那句「embed 只需要一张不含网络的卡」。SPEC
§二/§三十三 禁 WebView 与 JS 运行时，这次没有绕那道禁令，也没有把它当成降级：一行文档的
代价仍然是 `blocks.kind` 里的一个字符串 `"embed"` 加地址本身，`user_version` 依旧停在
**8**。

**改动面**：`src/core/embed.rs`（新增）、`src/core/mod.rs`、`src/core/types.rs`、
`src/app/state.rs`、`src/app/controller.rs`、`ui/Types.slint`、
`ui/components/EditorBlock.slint`、`src/services/export_service.rs`、
`src/services/import_service.rs`、`tests/integration/markdown_test.rs`、
`benchmarks/scripts/sweep.ps1`。

**六条决定**：

1. **`BlockKind::Embed` 是 int 22**，库里字符串 `"embed"`，`text` 存的就是地址本身，不新增
   列 ⇒ `user_version` 仍是 **8**，这是第五个不需要迁移的 kind。未知 kind 依旧按「加载即
   损坏」处理，所以老版本打开一个带 embed 的库会响亮地失败，而不是悄悄丢一行。
2. **卡片那两行走 `UIState` 的两个 pure callback**（`embed-label` / `embed-url`，Rust 侧
   `core::embed::describe` / `with_scheme`），不加 model 字段——ADR-0038 的教训照搬过来。
   绑定被 `is-embed ? … : ""` 守住，因为 `visible: false` 不阻止 binding 运行。21 个已收录
   站点，google.com 既认子域也认第一段 path；认不出来的域名就以域名本身当标签；没有域名可
   说的三种形状分别是 `Embed`（空）/ `Link`（一句散文）/ `Email`（mailto）。
3. **卡片可以编辑（和 toc 相反）**：点卡片改的就是地址，边打字标题边重算；空卡说
   「Embed / No address yet」而不是一个空盒子。没有 WebView（SPEC §二/§三十三），所以「卡 +
   交给系统打开」就是功能全部，不是降级形态。
4. **Markdown 是一行裸地址**：GFM 本来就把它渲染成链接，尖括号或注释标记只是多一个「文本不
   是合法地址时会写坏」的东西；导入侧 `is_bare_address` 只认「整行一个 token 且带显式
   `http(s)://`」。
5. **导出走 code / divider / math 那条 verbatim 分支**，不过 inline-mark 渲染器——url 里全
   是 `_ * ~ &`。
6. **`open-link` 的 shell 分支前面加了 scheme 白名单** `core::embed::is_openable`
   （http/https/mailto）。**这条要写清楚不是修漏洞**：编译探针证明 std 会给参数加引号，
   `… & echo PWNED` 是作为一个整体字面串到达 `start` 的，注入从来不存在；真正变的是行为
   ——`file:///…`、`\\share\x`、`javascript:…` 和裸 `C:\…\calc.exe` 现在什么都不做，因为
   链接目标是「从文件里来的数据」，而 `start` 运行路径和打开 url 一样乐意。

**自己写的测试抓到两个真 bug**（改的是代码不是断言）：`maps.google.com/?q=x` 读成
"Google"（product 只从 path 读）；`HTTPS://WWW.Site.COM` 没去掉 `www.`（先去前缀再小写）。
另外 `TextInput` 在 Slint 1.18 没有 `placeholder-text` 属性，编译失败后把空卡提示改成由两个
`Text` 的 `visible` 表达。

**验证**：`cargo check --all-targets` 干净；`cargo test --all-targets -- --skip
clipboard_write_and_read_round_trip_unicode` → **352 passed / 0 failed / 10 ignored**（上一批
340 / 0 / 10，这次 +8 条 `core::embed` 单测 +4 条 markdown）；markdown 那一个 target 单独跑
61 passed；`cargo build --release` 零警告。新测试里最值钱的是控制断言：句子里的 url 仍然是
句子（`an_address_in_a_sentence_stays_a_sentence`），以及 `evilyoutube.com` 不算 YouTube
（整标签后缀匹配）。跳掉的那条仍是本 session 环境的剪贴板独占（`OpenClipboard` err=5）。

**性能**：闸按上一批立的规矩第三次跑——control `4fa2b7b` 在 `git worktree` 里编
（`src/core/embed.rs` 不在），两臂同一 sitting 交替跑 scene D，各自先用 md5 + 大小自证身份
（control md5 `9c5e153f…` 22 017 024 B，本树 `19aea07c…` 22 072 320 B）：control 稳态
136.7 / 137.0 WS · 110.9 / 112.1 private，本树 138.2 / 138.9 · 111.8 / 112.3 ⇒ **1.005×**。
两臂均值之差 0.55 MB 小于 control 臂内部 1.2 MB 散布，所以仍是「量不出来」。种子那次
（109.7 / 111.3，startup 807 / 1104 ms）不进稳态。idle CPU 这次是本树更低（3.12–4.09 vs
4.29–5.85），这是同一个「分不出」的另一种写法。exe 大了 55 296 B。**没有做计时**：卡片不像
目录要扫页，它的活里没有「页」这个量——这是构造上界，不是读数。一个方法结论进了
`docs/PERFORMANCE.md`：三批连在 1 MB 内 ⇒ 闸有 ≈1 MB 的下限，低于它的 per-row 成本该写成
「分辨不出」，而不是报一个看着像测量的比值。数据
`benchmarks/results/2026-09-21-m11-embed-ram.jsonl`。

**像素**：`.scratch/sweep22` → `.scratch/sweep23`，47 张里动 3 张、新增 2 张（`embed`、
`embed-empty`）、44 张 byte-identical。bbox 全在改动能解释的带内：`slash` 774 px 与
`dark-slash` 2 528 px 同在 `x 340..618 / y 596..650`（比上一批的带低一档，因为菜单又多了一
行）、`plus` 1 062 px 在 `x 612..860 / y 656..792`；`dark-slash` 的框和 `slash` 逐像素相同，
所以一条判决覆盖一对。两张新 scene 仍然走「把样例页第一段转换成 embed」而不是往 fixture 里
加行——44 张不动就是这句话的控制断言。判图前先看的是 manifest 哈希 diff，不是肉眼。49 张是
新基线。

**未验证**：①**人眼**按一次 Open——浏览器真起来、卡片编辑中（边打边重算标题）、arrow 的
hover、暗色主题的卡、Turn into Embed 再转回 Text，`quire_shot` 不点 TouchArea，所以这条链路
一个像素都没有（headless 点它会真的开浏览器）。②`is_openable` 之后 `file://` 一类链接「点了
没反应」是不是用户要的行为——行为改了，没有测试能证明这被接受。③未收录域名的标签宽度（长
host 在卡里 elide 的读感）。④bookmark 的边界：SPEC §三十七 里它仍然是「抓标题与 favicon」，
这次没有替它做网络那一半，卡片形状已经在那儿了。

**批次 C 还剩**：highlight / bookmark / synced block。建议顺序不变：代码高亮 → bookmark →
synced block；高亮的硬要求（长代码块打字延迟不可测）大概会复用 math 与 TOC 的同一份 runs
通道，bookmark 现在只差一个 TLS 客户端，synced block 等 §四十。`reveal` 仍不属于批次 C 任何
一条，但 TOC 和 `quire://block` 锚点还在等它。


## M8 收尾 · A4 最后一个 HIGH — 带标记的一行现在按词断行（2026-09-21，on `master`，ADR-0041）

批次 C 三条之间插进来的这一刀，还的是 M8 的账：A4 视觉清扫从 `42cb158` 起就挂着一条 HIGH，
「带 inline marks 的段落丢掉换行、在词中间被裁掉，而同样一段文字不加粗时是正常换行的」。它一直被
写成「Slint 平台墙」，于是这一批先去看墙是不是真的——是真的（Slint 1.18 的 `Text` 没有
`TextFormat`、没有 per-run style），但墙后面还剩一条能走的路：**一个 layout 单元不可断行，那就
让单元等于一个词**。

**改动面**：`src/app/state.rs`（`build_runs` + 两条单测 + 一条 `#[ignore]` 计时）、
`ui/components/EditorBlock.slint`、`ui/components/TableBlock.slint`、
`ui/components/ColumnItemRow.slint`、`src/app/controller.rs`（3 个 scene）、
`src/main.rs` / `src/bin/quire_typing.rs` / `src/bin/quire_shot.rs`（`--marks`）、
`benchmarks/scripts/sweep.ps1`、`benchmarks/scripts/bench.ps1`、
`tests/integration/{persistence,workspace}_test.rs`（`HandleArgs` 字面量）。

**六条决定**：

1. **只切未标记的片段**：`build_runs` 在无标记的连续文本里每个词起头切一刀，标记片段一个 cell
   到底。理由写进注释了——下划线、code 底框、链接的点击目标按空格切开，比它断不了的那个粗体短
   语更糟。
2. **空白跟着它前面的词**，所以 cell 拼回去就是原文，逐字节。两条新测试一条钉死期望的 cell 列
   表（`["one ", "two ", "three ", "bold words", " four ", "five"]`），一条覆盖「标记在行首 /
   CJK / 标记在行尾」并断言拼回与无空 cell。
3. **三处画 runs 的 delegate 一起换成 `FlexboxLayout { flex-wrap: wrap }`**：块行、表格单元格、
   分栏里的行。只改第一处等于把同一面墙留在另外两个表面上。
4. **行高权威不动**，仍是旁边那个不可见的 plain `Text`（同字同宽，量的就是行数），runs 容器
   `clip: true` 在那个高度上。先试过把 `body-height` 绑到 flex 自己的 `preferred-height`：**Slint
   编译错误 `Cannot access id 'runs-flex'`**——`if` 里声明的元素在 `if` 外面不可引用；把 `if` 提
   出来让 flex 常驻，代价是 10 000 行 bench 页每行两个 item。所以断行点从 Rust 侧给。
5. **harness 先修再量**：scene D 一个标记都没有，直接跑闸会得到「1.00× 的空气」。于是
   `--marks N`（每 `rows/N` 行把第二个词加粗）进了 main / quire-typing / bench.ps1，
   `--dump-state` 多印一行 `marked=N`，让每条臂报出自己真正建了什么。
6. **不迁移、不加 kind、不加 model 字段**：`user_version` 仍是 8，runs 通道仍是
   `Vec<TextRun>`，存储 / undo / Markdown / LAN 导出全都不知道这次改动。

**墙移到了哪儿，还剩什么**（这句必须写准）：①一个长过一行的标记短语仍然裁；②一行标记文字比同样
词数的未标记文字**需要更多行**时仍然裁（粗体和 mono 更宽，而高度是 plain text 量的）；③没有
ASCII 空格的文本（中文段落）仍是一个 mark 一个 cell，因为切词用的是 `is_ascii_whitespace`。这三
条现在是窄例，不是常态，`docs/EDITOR_ARCHITECTURE.md` §"Platform wall" 逐条写着。

**验证**：`cargo check --all-targets` 干净；`cargo test --all-targets -- --skip
clipboard_write_and_read_round_trip_unicode` → **354 passed / 0 failed / 11 ignored**（HEAD 是
352 / 0 / 10，这次 +2 单测 +1 条 `#[ignore]` 计时）；`cargo build --release` 零警告。跳掉的仍
是本机环境的剪贴板独占（`OpenClipboard` err=5）。计时那条试过搬成独立的 `[[test]]` target（这样
往 control worktree 里复制时不必改被测文件），最后没有搬：ADR-0039 的投影数字已经是 lib 里同一种
`#[ignore]` 测试，多一个 target 换不到一致性，而且复制的只是 `#[cfg(test)]` 代码，不碰被测路径。

**性能**：闸跑了**两批**（原始行 `benchmarks/results/2026-09-21-m8-wordwrap-ram.jsonl`，12
行）。control = `2c5a25f` 在 clean worktree 里编，**只**把 bench 旋钮搬过去，所以它的 runs 还是
一行裁掉的（md5 `cdf6da5a…` 22 074 368 B）。scene D 10 000 行 / 1 000 行带标记，两臂交替，各自
pinned db，每臂种子那次不计：第一批（本树 `f59edb1b…` 22 081 024 B）control 112.7 / 111.4 →
本树 115.2 / 115.9 ⇒ **1.031×**；第二批（表格与分栏 delegate 也改完，`4f9205ca…` 22 087 680 B）
control 114.3 / 111.1 → 本树 114.8 / 115.7 ⇒ **1.023×**。两批都在 ≤1.2× 内，但诚实的读法是：
两臂均值差 2.5…3.5 MB **小于 control 臂自己在第二批的 3.6 MB 散布**，所以这是「贴着闸的分辨率」。
数字有形状：一条 bench 标记行从 3 个 run 变 11 个 cell，一个 cell 就是一个带自己 `Text` 的
item。投影侧另有一条两臂同 sitting 的计时（`#[ignore]`，release，50 轮 `project_blocks`）：
未标记 41.686 / 41.504 ms（两臂差 0.4 %，这就是「同一台机器同一份 fixture」的控制），1 000 行
标记 44.147 (+2.461) / 46.942 (+5.438) ⇒ 切词大约 **+2.8 ms / 万次投影**，即每条标记行
≈2.8 µs，**全页无标记时是 0**——这也是 `block.runs.length > 0` 那道守卫留着的原因。

**像素**：`.scratch/sweep23` → `.scratch/sweep24`，49 → 50 张：动 3、新增 1（`marks-wrap`）、
46 张 byte-identical。`marks` 与 `dark-marks` 1 942 / 1 918 px 落在同一个框
`x 390..1148 / y 196..240`（一段被裁成一行 → 两整行）。`math-inline` 动 360 px 在
`x 420..664 / y 194..210`——这张本来该「完全不动」，所以又补了两个探针：位移探针（找能把差值归零
的 dx/dy）说它不是平移，ink-edge 扫描说它是**最右有墨列从 661 变成 664**，即一行按词测量比按整
串测量多出 ≈3 px 的间隙累积（每个间隙不到 1 px）。`.scratch/sweep24` → `.scratch/sweep25`，
52 张：**已有 50 张动 0 张**，新增 2 张（`table-marks`、`columns-marks`）——那个 0 不是证据，
两张新图才是：grid 的 fixture 每格一个词、layout 的是「Column 1 / Column 2」，两处 delegate 改
完没有任何一张图会失败，于是各造一个「塞进最窄的盒子、句子长到必须换行、中间一个粗体短语」的
scene。`marks-wrap` 同理，而且它的 needle 全改成 `.expect("needle present")`：早先
`unwrap_or(0)` 把「break between cells」拼错成找不到时，静默标到了偏移 0，画面看着仍然像那么回
事。判图前看的仍是 manifest 哈希 diff。52 张是新基线。

**未验证**：①**人眼**——在换行后的标记行里点一下 caret 落在哪、编辑中标记段长高一行、链接现在
只有一个词那么宽（hover 目标与「一个词的 underline 算不算 bug」）、中文标记段落（还是整段一
cell）、表格格与分栏里编辑时的行高。②`--marks` 只喂 bold，italic/code/link 混排的内存形状没
量。③两批比值之差（1.031 / 1.023）本身没有解释，只能记成噪声。

**批次 C 还剩**：highlight / bookmark / synced block。**高亮前面那面墙没了**——per-token 的颜
色 run 现在能断行，这条正是它当初排不进来的理由；bookmark 仍差一个 TLS 客户端（embed 已经把卡
片形状占了，缺的是抓标题与 favicon 那一半网络），synced block 等 §四十。`reveal`（变高
scroll-into-view）不属于批次 C，但 TOC 跳转和 `quire://block` 锚点还在等它。
（写在下条之后的更正：**「墙没了」不是高亮能走 runs 通道的意思**——ADR-0042 最后没走那条路，
理由在下条第 1 条。）

## M11 批次 C · slice 4 — code 高亮：一个颜色是一层，不是一段 run（2026-09-21，on `master`，ADR-0042）

SPEC §三十七 批次 C 的第四条，也是这批里唯一先撞墙再绕过去的那条。上一条结尾说「高亮前面那面
墙没了」，指的是 ADR-0041 让 run 能断行；真去量 token 的时候发现墙其实还在，只是换了个位置：
**token 不是词**。一条字符串字面量可以跨行，一段注释可以，一条模板串也可以，而 ADR-0041 的 cell
按 `is_ascii_whitespace` 切，切出来的单元天生不含换行。所以 per-token 上色走 runs 通道的话，
一个 token 要么占满一整个 layout 单元（跨行时无法断），要么被按空格切开（字符串里的空格就不是
颜色的一部分了）。两条都不要，于是改成一整块字符串叠六层。

**改动面**：`src/core/highlight.rs`（新增，1 005 行，含 16 条单测）、`src/core/types.rs`
（`Lang` + `Block.lang`）、`src/core/{command,history,persistence,mod}.rs`、
`src/storage/migrations.rs`（**v9** `add_lang_column`）、`src/storage/repository.rs`、
`src/app/state.rs`（投影带 `lang`、`code-hl` 的 fixture `bench_code_source` / `bench_code`、
`--code` 的 10 000 行构造）、`src/app/controller.rs`（`on_code_layer`、`SetCodeLang`、两个
scene）、`src/main.rs` / `src/bin/quire_typing.rs`（`--code` + `--dump-state` 的 `coloured=`）、
`src/bin/quire_shot.rs`、`src/services/{import,export}_service.rs`（fence info string）、
`src/services/{settings_store,lan_server}.rs`、`ui/{Colors,Types}.slint`、
`ui/components/Editor.slint`（advance 探针）、`ui/components/EditorBlock.slint`（六层）、
`benchmarks/scripts/{sweep,bench}.ps1`、`tests/integration/*`（`HandleArgs` 字面量 + 新断言）、
docs 六份。

**七条决定**：

1. **一层，不是一个 run**。`highlight::layer(text, lang, frame, advance, kind)` 返回**整个
   block**，把不属于这个颜色的字符换成 `U+00A0`（不可断空格——这是它不用普通空格的原因：空白列
   不能成为断点），源码里的空白原样保留。六串等长、在同一批硬换行处断，所以它们是逐字叠在一起
   的：不需要两个排版引擎互相同意，而**量行高的那一层（`Kind::Plain`）本身就是叠出来的一层**。
2. **换行是派生的，不是存的**。列预算 `columns = floor(frame / advance)`，ASCII 记 1 列、tab
   记 8、其余记 2（非 ASCII 的真实宽度这个模型不知道，见第 6 条）。`advance` 为 0（第一次量到
   之前）就只上色不加换行，退化成未上色块的软换行——那是更朴素的渲染，不是坏掉的渲染。
3. **一次全应用的字体测量**：`Editor.slint` 里一个 `visible: false`、摆在 `-1000px` 的
   `code-probe := Text { text: "0000000000"; changed width => UIState.code-advance =
   self.width / 10 }`。十个数字是因为小数 advance 在长串上舍得更准。没有第二个地方猜字号。
4. **颜色复用块调色板的槽位**：`Colors.code-token(kind)` 把 keyword/comment/string/number/name
   映到 block-text 的 7/1/5/3/6（紫/灰/绿/橙/蓝），暗色是同一组槽位的另一列。不新增颜色常量，
   主题开关不用碰。
5. **上色只在没在编辑时**，而且是**条件元素不是 `visible`**：`if root.is-highlighted &&
   !editing : Rectangle { for kl in 5 : Text {…} }`。`visible: false` 不阻止 binding 运行——
   第一版就是这么写的，结果是页面每行每次重绘跑五次 lexer。改成条件元素后，光标在块里时那五层
   根本没被 realize；这也是「打字延迟不可测」的机制，不是运气。
6. **画序 1,3,4,5,2，注释压最后**。非 ASCII 字符宽度未知 ⇒ 模型不敢把它 blank 掉 ⇒ 它被逐字抄
   进每一层，于是它的颜色由最上面那层决定。散文（中文注释、字符串里的中文）最可能是非 ASCII，
   所以让注释色赢：写在注释里的中文读起来是灰的，这是取舍不是 bug，SPEC 边界里写着。
7. **语言是块存的唯一新东西**：`blocks.lang`（v9，本批第一次也是唯一一次迁移——math/TOC/
   embed 四类都不存新东西）；入口是块自己的 ⋮ → Language（submenu id 14，只在 code 块出现；
   `CODE_LANG_BASE = 600_000 + index`），撤销走 `Change::BlockLangSet`。`Lang::try_from_str`
   折别名（`rs py python3 jsx tsx markdown jsonc sh shell zsh`），认不出的折成 `Plain`——**一
   个这份构建认不出的语言是没有颜色，不是一个坏掉的块**。Markdown 双向带 info string（进来什
   么别名都认，出去写规范名 `rust`，`Plain` 写空 fence），富文本粘贴带 `lang`。

**两个只有截图能抓到的缺陷**（都进 ADR-0042 与 `docs/UI_ARCHITECTURE.md` 的几何陷阱表）：
①**层与量尺的错位**——`code-hl` 第一张图上，五种颜色整块浮在字上方一行。根因是 Slint 的
`Text` 没绑 `width` 就永不换行（它的宽度变成 `preferred-width`），所以五层只在 lexer 自己插入
的硬换行处断，而量高那层在软换行。修法：五层各绑 `width: root.code-width`。**复证不是在
1 280 px 而是 760 px**：fixture 里没有一行长到需要断的时候，这个缺陷在任何宽屏图上都看不见。
②`if cond : { Rectangle {…} }` 编译不过（`expected Identifier`，EditorBlock.slint:607），条件
元素的冒号后面必须有元素名。写文档时还出过第三次自己的错：把「彩色臂 private 更低」解释成
「代码块文字更短」，而 fixture 是 ≈58 字节的 lorem 行换成 330 字节七行——PERFORMANCE 那节已按
实际数据改掉，理由见下。

**验证**：`cargo check --all-targets` 干净；`cargo test --all-targets -- --skip
clipboard_write_and_read_round_trip_unicode` → **374 passed / 0 failed / 11 ignored**（上一条
ADR-0041 是 354 / 0 / 11，这次 +20）。其中 16 条在 `highlight.rs`：五种语言各自的词法（注释里的
`//` 不是注释、`'a` 是 lifetime 不是字符、JSON 的 key 与 value、bash 里词中的 `#`）、层与源串等
长、所有 kind 共享同一批断点（`every_kind_shares_the_breaks_so_the_layers_stack`）、列预算
（`a_wrapped_line_never_exceeds_what_the_row_can_show`）、宽字符只抄不 blank、`U+00A0` 不是断点
而真空格是。别名折叠那条在 `types.rs`（`lang_strings_round_trip_and_aliases_fold`），迁移与读写
在 storage/markdown 两处集成测试里。`cargo build --release` 零警告。跳掉的仍是本机剪贴板独占。

**性能**：三臂两 exe 一个 sitting（原始行
`benchmarks/results/2026-09-21-m11-highlight-ram.jsonl`，15 行）。control = `2b62498` 在 clean
worktree 里编（md5 `bde7faec…` / 22 087 680 B）；candidate 是本树 release（md5 `961ff115…` /
22 194 176 B），并跑 `--code 0` 与 `--code 2000` 两臂——**同一支 exe**，所以「树的账」与「上色
的账」是两个读数。每臂自己的 pinned db、交替、种子那次不计、每条臂用 `coloured=` 自证 fixture
（不带该字段的二进制印不出这行，这就是它的身份证）。稳态 private：control 97.4 / 96.9，
candidate 无彩色 97.5 / 98.4，彩色 2 000 行 90.6 / 90.4 ⇒ **1.008×** 与 **0.924×**。第一个数
是这批欠闸的数：0.8 MB 对 control 臂自己 0.5 MB 的散布，就是「量不出来的成本」。**第二个数不
是成绩**，而且它的第一版解释是错的：彩色臂的行**更长**（330 B 对 58 B）、**更高**（7 行对 1
行），文字量涨、行高涨，private 反而掉 7.5 MB。而且它连符号都不稳：200 行彩色 ⇒ +2.75 KB/行，
2 000 ⇒ −4.4 KB/行，5 000 ⇒ −1.85 KB/行。一个在自己量程内变号的量没在被测量，所以这里发布的
是「**彩色行多 25 倍，进程没有变大**」，不是那 7.5 MB。它是 committed-not-touched：三臂的
working set 全在 132.2–133.4 MB，落掉的 7.5 MB 从来不在内存里，这次坐实的只是一句——不同分配
序列留下的 arena，本闸不定价。idle CPU 稳态 0.2–0.59 %（种子那三次 2.53–5.65 %）。exe 大 106
496 B。SPEC §三十七 真正写下来的是延迟，scene E 答这条：同一支 exe，`--code` 0 对 2000，各两
轮（`benchmarks/results/2026-09-21-m11-highlight-typing.jsonl`）——handler 中位 **89 µs** 对
92–112 µs，p95 154–167 对 154–207，两列都不输，而未彩色臂自己跨轮散布（92→112）比两臂之差还
宽。「长代码块打字延迟不可测」达成。彩色臂打字时 CPU 确实高（29.7–30.8 % 对 24.3–27.1 %），那是
软件渲染器每 tick 重画一行高亮的开销，进程有富余；若哪天把五层挪出 `!editing` 守卫，要重读的
就是它。

**像素**：`.scratch/sweep25` → `.scratch/sweep26`，52 → **54** 张：**已有 52 张动 0 张**，新增
`code-hl` 与 `dark-code-hl`（`sweep.ps1` 的 scene 表在 `embed-empty` 与 `dark-link` 之后各加一
行）。两张新图是这一批唯一的证据，所以不能只看「有颜色」：用 PIL 对 PNG 做了**颜色普查**，五
种 token 色在两张图里都出现（light 5 色 / dark 5 色），而 `default` 与 `dark` 两张基线图里各只
有 4 与 1 个杂点落在这些值上（那是 UI 别处的相近色，不是高亮）。这一步是必要的：一个「paints
nothing」的高亮器不会让其余 52 张里任何一张变化，也不会让上面任何一个内存数字变坏。另外补的
两张对照（`.scratch/hlfix/`，非基线）：1 280 px 的原图与 760 px 的窄图——后者才真的逼出列预算
那条路径，错位缺陷就是在这两张之间从「看不出来」变成「看不对」的。54 张是新基线。

**未验证**：①**人眼**——⋮ → Language 菜单的实际观感（一级菜单里第 14 项、8 个语言项）、编辑中
五层消失 / 退出编辑后出现、中文注释读成灰色的取舍在真实页面上能不能接受、暗色主题下的对比度、
以及 caret 落在高亮块里时的行高。headless 一张图都没有按下 TouchArea。②`Lang::Plain` 的块在一
个已经上色的页面里会不会被误当彩色（守卫是 `block.lang != ""`，代码路径有单测，画面没有）。
③非 ASCII 的列宽模型（第 6 条）只按 2 列估，一条全是中文的「代码」在窄框里会比实际宽，`clip:
true` 兜住不溢出，但没有图证明这读起来是对的。

**批次 C 还剩**：bookmark / synced block。bookmark 仍只差一个 TLS 客户端（embed 已经把卡片形状
占了，缺的是抓标题与 favicon 那一半网络）；synced block 等 §四十。高亮这条不再需要 runs 通道，
所以 ADR-0041 那三条残留（长标记短语、标记行需更多行、无空格文本）与它无关了。`reveal`（变高
scroll-into-view）仍不属于批次 C，但 TOC 跳转和 `quire://block` 锚点还在等它。


## 工具诚实 · quire-shot 不再冒充 FemtoVG（2026-09-21，on `master`）

批次 C 第四条之后顺手还的账：A4 报告里那条「known limitation, not a defect」——sweep 每张图的
侧栏页脚都写着 `FemtoVG · GL`，可 `quire-shot` 装的是 `HeadlessPlatform`（Slint 的软件光栅器），
而 `--features software` 并不会关掉默认的 `femtovg`，于是编译期按 feature 顺序答出来的名字说的是
一个从没跑过的渲染器。这不是观感问题：sweep 是本项目关于「渲染」的证据，它一直在给自己的证据贴
错标签——`scroll_ab.ps1` 早就为 skia 臂记过同一个坑（`--features skia` 不关 femtovg）。

**改动**：`src/bin/quire_shot.rs` 在 `controller::wire()` 之后把 `UIState.renderer-name` 覆写成
`Software · headless`。app 二进制一个字没动——它跑的确实是自己的 feature 选中的后端，页脚写的就是
对的；只有「自己装 Platform」的那个进程需要说真话。

**验证**：`cargo check --all-targets`、`cargo build --release`、`cargo build --release --features
software --bin quire-shot` 三条全零警告（ROADMAP 里那句「shot 构建还剩一个 `unused variable: ui`
警告」已经作废：`render()` 早就不接 `ui` 参数了，这次撤回）。sweep26 → sweep27：54 张**全动**，
而「全动」正是这一刀的预期——`diffbbox` 说 53 张只动了 `x 96..192 / y 778..784` 里 118…125 个采样
点（页脚那一行 caption），`settings` 动 244 px 覆盖 `x 96..722 / y 692..784`（页脚 + 对话框的
Renderer 行）。读这个属性的地方一共两处，变化的框也正好两处，别处一个像素都没有。两张放大裁图
（`.scratch/shotlabel/`）直接读到旧图 `FemtoVG · GL`、新图 `Software · headless`——两臂各自自证
身份。新基线 `.scratch/sweep27`（54 张）。

**顺带清掉的文档债**：ROADMAP 的 M8 verification snapshot 多了一节「Re-run at `63a4081`」，把测试
数（374 / 0 / 11）、三条构建、sweep 结果写清，并明确 installer / portable 两条**这次没重跑**；A4
的 open list 逐条对代码复核后落成——D1（ADR-0041）、D9（ADR-0033，`ui/` 里已无「later milestone」
字样）、模态遮罩、`Ctrl+B` 让位给 `Ctrl+\`、渲染器标签，五条全闭；只剩 D7（MEDIUM）与两条对比度
LOW。D7 这次也拿到了代码证据：`EditorBlock.slint` 里没有任何 find 的引用，所以「caret 不在这一块
就画不出命中」不是坏掉，是**功能不存在**——它要的是行级命中区间，撞的正是 ADR-0042 绕开的那面
「一个 Text 一种颜色」的墙，因此按新 feature 排期，不按 bug 修。

**未验证**：`--no-default-features --features software` 这一支 shot 构建没跑过——现在标签与 feature
无关，所以那个组合不再影响图的正确性，但它的构建时间与产物没量过。
