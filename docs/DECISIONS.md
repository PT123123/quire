# Architecture Decision Records

Format: decision → context → consequences. Newest first.

## ADR-0016 · Inline marks are the Markdown interchange format, spans in and
spans out
Decision: `import_service::parse_markdown` reads `**bold**`, `*italic*`,
`` `code` ``, `~~strike~~` and `[text](url)` into `core::types::Mark` spans
(byte offsets on char boundaries, the same sorted shape `Command::ToggleMark`
maintains), and `export_service::export_page` writes those spans back as
markers. Neither side builds an AST: the importer recurses into the span it
just matched, the exporter walks the mark *boundaries* and opens/closes markers
there, so nesting is expressed by the ranges — which is the shape the renderer
already consumes. Two shapes have no CommonMark spelling, because its inline
tree is strictly nested: marks of different kinds that only partially overlap,
and styling inside a code span (whose content is literal). The exporter cuts
the first at the crossing — every character keeps its text and each piece
keeps its kind — and drops the second. A literal marker in text is written as
an escape (`\*`), and a code span whose content starts or ends with a backtick,
or is itself padded with spaces, takes the space wrapper that CommonMark
strips back off.
Why: the document model is a flat span list, so anything that parsed a real
CommonMark tree would have to flatten it straight away; keeping the flat list
on both sides makes the pair testable by round trip (`export ∘ import` and
`import ∘ export` land on the same blocks) instead of by a conformance suite
this app cannot afford. Degrading an unrepresentable span into pieces, rather
than dropping it silently, keeps the text byte-exact — the property users
notice when they re-import their own notes.
Consequences: the round trip is a fixpoint from the second pass, not
byte-identical on the first for the crossing case (it comes back as two bold
and two italic pieces). Block-level ambiguity is out of scope and stays
literal: the exporter escapes inline markers but not a line-initial `#`, `-`,
`>` or `1.`, so a paragraph that *starts* with list syntax does not survive
re-import as a paragraph. Tables, images, setext headings and footnotes are
imported as text for the same reason.

## ADR-0015 · Crash recovery: rotating `VACUUM INTO` snapshots, restore at
open; settings and metadata as a diffed key/value layer
Decision: the durability story from M3 stands and is now on the record as
measured: `journal_mode=WAL` is stored in the file header (a later open reads
back `wal`), `synchronous=FULL` (=2), `locking_mode=normal`, `page_size=4096`,
`wal_autocheckpoint=1000` pages ≈ 4 MB, so SQLite folds the WAL back into the
main file by itself during a long session and a clean close checkpoints and
removes `-wal`. On top of that, `src/storage/backup.rs` gives SPEC §二十五 a
backup policy: every successful open shifts `<path>.bak1 → .bak2 → .bak3`
(dropping the oldest) and writes a fresh `.bak1`, and `SqliteRepository::open`
goes through `backup::open_with_recovery` — a main file that fails the startup
`integrity_check` is repaired before the app ever sees an error. Recovery
walks `.bak1 … .bak3`, validates each candidate by really opening it
(migrations + `PRAGMA integrity_check`), moves the unreadable main file aside
as `<path>.corrupt` and deletes its `-wal`/`-shm`, then *moves* (not copies)
the good snapshot into the main path and opens that. `Database::open` itself is
unchanged, so the existing "corruption is reported, not hidden" behavior is
still reachable (and still asserted by `storage::database::tests`).
For the UI's remembered state, `src/services/settings_store.rs` wraps the
frozen contract: `Settings` is a `BTreeMap` with the two keys the panels need
(`theme`, `sidebar.expanded`), and `SettingsStore` over `Arc<dyn Repository>`
offers `load_settings`/`load_meta`, `save_settings`/`save_meta` and the
non-writing `settings_changes`/`meta_changes`, which diff against what the
repository holds and emit only `SettingSet`/`MetaSet` changes — so a burst of
window-resize saves can be queued through `PersistenceService` and stay inside
the debounce window instead of writing per event.
Why `VACUUM INTO ?1` rather than a file copy or rusqlite's `Backup`: a copy of
`workspace.db` misses whatever still lives in `-wal` and can catch a torn page,
while `VACUUM INTO` reads through the live connection (WAL included), writes one
self-contained compacted file with no sidecar, and is a single statement on the
connection the snapshot already locks — so it is one consistent point in the
change stream, not a race. rusqlite's `Backup` offers the same consistency
page-by-page but needs a second destination connection; the statement is
simpler. The copy runs with `synchronous=OFF` and restores `FULL` afterwards,
which is safe because a snapshot is expendable (a torn copy fails its own
integrity check when recovery tries it, and the next open rewrites it) while
`VACUUM INTO` only reads the main database — measured at 2.3 MB: ≈23–49 ms
relaxed vs ≈45–256 ms at `FULL`, same process alternating rounds.
Consequences: startup pays ≈40 ms per 2.3 MB of workspace (PERFORMANCE.md,
M8 addendum) and the folder holds up to 3 extra copies of the database — both
are the price of never opening a blank app after one bad write. The loss
window is by design: `.bak1` is the database *as of the last successful open*,
so a corruption that arrives mid-session costs the edits made since startup;
closing that would mean rewriting the whole file every flush, which §三十三
rules out in spirit — Track A can call `backup::snapshot` from a "save a copy"
menu item if a real case appears. Recovery only answers *structural* damage: an
unknown `blocks.kind` still surfaces as `Corrupt` from `load()` (ADR-0013)
after a clean open, and that path is the app's to handle (M8_FEEDBACK.md). A
snapshot failure is logged and ignored — a read-only or full directory must
never block opening the document — so tests that want the unrecoverable case
have to delete the `.bak<N>` family first. Because the FTS5 mirror lives in the
same file (ADR-0014), a recovered database is searchable immediately, with no
rebuild; and because `Settings` treats an empty value as absent (the contract
has no `SettingDelete`), a stored-but-empty setting is indistinguishable from a
removed one.

Addendum (M8, branch `m8-rc`): that recovery is now *reportable*.
`backup::open_with_recovery` returns `(Database, OpenReport)` with
`recovered_from: Option<PathBuf>` (the snapshot that was moved into the main
path) and `backup_failed: bool` (this session has no snapshot behind it),
reached through the new `SqliteRepository::open_with_report`; `open()` keeps
its old signature and discards the report, so every existing caller and test
stays as it was. `OpenReport::log()` prints the two facts in the `eprintln!`
convention startup already uses, which is what `main.rs` calls now — a UI
warning can be built from the same fields without touching storage again
(closes M8_FEEDBACK #4's "no way to tell the user").

## ADR-0014 · Full-text search: FTS5 mirror inside the apply transaction,
CJK indexed by hand-built segmentation
Decision: search (SPEC §二十) uses two FTS5 virtual tables added by schema
version 2 — `search_pages(rowid = page id, title)` and
`search_blocks(rowid = block id, page_id UNINDEXED, text)` — written by
`src/storage/search_index.rs` from *inside* `SqliteRepository::apply`'s and
`replace_all`'s transaction: `insert_page`/`insert_block` index as they
insert, `PageTitleSet`/`BlockTextSet` re-index by rowid, and one orphan
sweep (`prune`) runs per batch that contained a delete, so the FK cascades
that remove blocks and pages need no per-row bookkeeping. The index can
therefore never drift from the document: an aborted batch rolls the index
back with the rows (ADR-0012). `Repository` and `Change` are untouched —
the query API is `SqliteRepository::search(&SearchRequest)` plus
`src/services/search_service.rs`, which aggregates raw matches into one
ranked `Hit` per page (bm25, block matches beat title matches for the
snippet slot) and offers `search_async` → `PendingSearch::poll` so the UI
thread never waits on SQLite (ARCHITECTURE hard rule 1).
Tokenizer: `unicode61`, which never splits inside a run of Han characters
("写作与中文测试" is one token, so "中文" would never match). Indexing
therefore stores a *segmented* copy — `segment()` gives every CJK character
(Han incl. ext. A/B–E, kana, Hangul) its own token — and queries are
segmented the same way, then issued as a *phrase* (`"中 文"*`) so only
adjacent characters match, reproducing the substring semantics of the M2
in-memory scan. Single words keep a trailing `*` for type-ahead.
Why not FTS5's `trigram` tokenizer (the usual CJK answer), measured with
the `#[ignore]`d `search_index::tests::fts5_capabilities` probe on the
bundled SQLite 3.53.2: trigrams need ≥ 3 characters, so a two-character
Chinese term — "中文", "字体", "行高", by far the common case — matches
nothing (`trigram "中文" -> []`, `"中文测" -> [1]`). It also indexes every
offset, inflating the DB for Latin text. Segmentation costs one extra pass
per write and keeps exact adjacency. The probe further confirms
`bro*` → "brown" (prefix works), `"quick br*"` → ∅ (`*` is only legal
*after* a whole phrase), and that an *unsegmented* Chinese phrase matches
nothing — i.e. query segmentation is mandatory, not cosmetic. FTS5 needed
no new Cargo feature: rusqlite 0.40 `bundled` (libsqlite3-sys 0.38) already
compiles SQLite with `-DSQLITE_ENABLE_FTS5`.
Consequences: a keystroke rewrites exactly one index row by rowid, so the
debounced write cost stays O(edited blocks) rather than O(page size) — the
10 000-block page of SPEC §二十二 keeps typing cheap (numbers in
PERFORMANCE.md "M3 · save latency"); the M2 linear-scan search in
`app/workspace.rs` stays in place until Track A wires the panel (both are
consistent with each other, no behavior change in this branch); migrating a
v1 database runs a one-time `search_index::rebuild` backfill after the
step commits (it needs its own transaction), and `check_schema` now also
requires the two FTS tables; `Arc<SqliteRepository>` must be kept around
by the app layer to build a `SearchService` (unsized coercion gives the
`Arc<dyn Repository>` the persistence pipeline wants — a plain
`Arc<dyn Repository>` cannot be downcast back); index rows for pages that
hold no text are simply absent, so an empty page is unsearchable by body
and by title alike; and the index is derived data — a rebuild is always
safe, which D4's recovery path relies on.

## ADR-0013 · Storage schema: cascade-FK tree + split block_children, one
transaction per contract call
Decision: the SQLite file uses the six SPEC §十八 tables — `workspaces`
(kept as the forward-compatible root, single row for now), `pages`
(self-referencing `parent` FK), `blocks` (identity + payload: page, kind,
text, checked), `block_children` (tree placement: `parent` FK + `ord`),
`metadata`, `settings`. Ids are the `core` u64 newtypes stored as `INTEGER`;
`OrderKey` maps u64→i64 by flipping the sign bit so signed storage keeps
unsigned ordering. Every mutation funnels through `Repository`: `apply`
takes one ordered `Change` list inside a single transaction (SPEC §十八),
deletions are recursive-CTE subtree deletes with `ON DELETE CASCADE` as
backstop, `replace_all` defers FK checks to commit and adds an explicit
acyclicity check (FKs alone cannot see an A→B→A cycle). Durability is
WAL + `synchronous=FULL` with a `PRAGMA integrity_check` at open
(SPEC §二十五); schema upgrades are forward-only steps in
`src/storage/migrations.rs` tracked by `PRAGMA user_version`.
Why: the split matches the SPEC's table list and keeps the hot payload
(text) on its own table for cheap `BlockTextSet` writes; cascade + CTE
makes delete semantics single-statement and testable; FULL is paid for
once per debounced burst (≈2–3 ms measured), not per keystroke (SPEC
§三十三 forbids the latter); cycle validation protects the sidebar tree.
Consequences: unknown `kind` strings or unreadable pages surface as
`StorageError::Corrupt` at startup instead of silent data loss; the
`workspaces` table is a placeholder until a real multi-workspace model
lands (feedback to Track A if M4+ needs it in `PersistedState`); a newer
`user_version` refuses to open rather than downgrade the file.

## ADR-0012 · Persistence contract lives in `core/`, storage implements it
Decision: `src/core/types.rs` defines the persisted model (`PageId`,
`BlockId`, `OrderKey`, `BlockKind`, `Block`, `Page`, `PersistedState`);
`src/core/persistence.rs` defines `Repository` (`load` / `apply(&[Change])
/ `replace_all`) plus the `Change` mutation enum. The M3 storage layer
(`src/storage/`, SQLite) implements the trait; the M4 editor emits
`Change`s from commands. The contract was frozen in one commit before the
two work streams started in parallel.
Why: two agents can then build storage and editor simultaneously without
interface drift; `core` stays free of Slint and of SQL details, and undo
(M4) replays inverse `Change`s rather than re-reading the DB (SPEC §十四).
Consequences: sibling order is a `u64` `OrderKey` with midpoint insertion
(`OrderKey::between`, renumber on exhaustion) — no per-insert row shifts;
deletes cascade in storage (undo replays captured `BlockInserted`s); `text`
is plain UTF-8 until M6 inline spans extend it. Any contract change goes
through one owner only (Track A) with the other side filing a feedback
note, never editing both sides at once.

## ADR-0011 · Headless visual regression via `quire-shot`
Decision: UI screenshots are produced by a second binary
(`src/bin/quire_shot.rs`) that installs a custom `Platform` whose window
adapter is Slint's `MinimalSoftwareWindow` (software renderer), renders the
real `AppWindow` into an RGB buffer, and writes a BMP; `just shot <scene>`
converts to PNG. Scenes (`--scene menu|rename|…`) set `UIState`/controller
state directly — no input injection.
Why: OS foreground policy blocks headless SendKeys, screen capture loses
to overlapping windows, and `PrintWindow` returns black pixels for
GPU-composited (GL) windows. The offscreen path is deterministic,
occlusion-proof, and doubles as the visual-regression harness for M3+.
Consequences: `quire-shot` builds only with `--features software`
(`required-features`), so the shipped binary stays lean; PopupWindow
show/close is exercised through the same code path as production.

## ADR-0010 · Page search = substring over a text blob; palette = commands only
Decision: search (Ctrl+P) scans `Page.search_text` (title + block text,
ASCII case-folding, char-boundary snippets, capped at 20 hits); the
command palette (Ctrl+K) lists commands only, generated from the
workspace. Both share `fuzzy_subsequence`-style matching only where it
helps (palette).
Why: content search must find unopened pages, which requires an inverted
index (SQLite FTS, M3+) or a flat blob; the blob is honest, fast at this
scale, and swappable. Separating palette and search mirrors the Notion
model and keeps command resolution unambiguous.
Consequences: `Workspace::search` is the single seam — M3 replaces its
body with FTS queries without touching UI or controller.

## ADR-0009 · UI event loop runs on an 8 MB-stack thread
Decision: both binaries spawn the Slint event loop / render on a thread
with an 8 MB stack (`std::thread::Builder::stack_size`).
Why: Slint 1.18 evaluates the component tree's initial property and
layout bindings recursively on the C stack; Quire's shell (sidebar tree
delegates + editor + five popup trees) needs slightly over the 1 MB
Windows default in debug builds, and popup open/close adds depth at
runtime. Verified by bisection: any single component removed masks it,
512 MB survives it — finite but deep. 8 MB is the standard Linux default
and costs only address-space reservation.
Consequences: crashes-in-the-field from stack exhaustion are off the
table for the planned M3–M6 growth; if a future Slint flattens binding
evaluation, the wrapper can be dropped in one place.

## ADR-0008 · Popup lifecycle is state-driven, never `is-open`-read
Decision: `CommandPalette` uses `close-policy: no-auto-close`; show/hide is
driven exclusively by `UIState.palette-open` (mirrored in AppWindow with
`show()`/`close()` handlers).
Why: Slint 1.18.0's const-propagation pass panics (`const_propagation.rs:
464`, no diagnostics) whenever a popup component *reads* its own `is-open`.
Verified by bisection with the `QUIRE_PROBE` subset-compile hook in build.rs.
Consequences: closing logic lives in UIState, which also makes the palette
testable from Rust; revisit if a future Slint fixes the crash.

## ADR-0007 · Page chrome rides inside delegates (ListView single-`for`)
Decision: a `ListView` may contain exactly one `for`; the page title and the
bottom spacer therefore live inside the first/last `DocumentRow` delegate
(`first` index check, `BlockRow.tail` flag set by Rust).
Why: 1.18 markup rejects sibling elements of a ListView `for`, and models
expose no `.length` to compute "last row" in the UI.
Consequences: Rust owns the `tail` invariant; re-sorting or appending blocks
must re-flag it (see `with_tail` in state.rs).

## ADR-0006 · M1 UI state via a single `UIState` global
Decision: transient UI state (dark, sidebar-open, palette state, selection
ids) lives in one Slint global; business actions are callbacks on that same
global, wired in `src/app/controller.rs`.
Why: components stay independently restorable; Rust reaches everything
through one generated accessor; there is exactly one place an agent must
read to understand UI state flow.
Consequences: property names are a contract — renaming requires touching
controller.rs and any component in the same change.

## ADR-0005 · Mock content is served through Slint models
Decision: sidebar tree, blocks, and command rows arrive from Rust as
`ModelRc<VecModel<struct>>`, even for M1 mock data.
Why: establishes the node-editor-cpp pattern (backend owns models, UI only
renders) before any real document model exists, so M3/M4 replace data
sources, not plumbing.
Consequences: mock data has the same shape discipline as real data.

## ADR-0004 · Renderer is a per-binary compile-time choice
Decision: `slint` is compiled with `default-features = false`; exactly one
renderer feature is enabled per build (`femtovg` default; `femtovg-wgpu`,
`skia`, `skia-opengl`, `software` selectable). Benchmarks build one binary
per renderer into its own target dir.
Why: comparing GPU stacks at runtime inside one binary would distort
memory numbers; separate binaries keep idle-RAM measurements honest.
Consequences: CI builds at least two renderer configurations.

## ADR-0003 · No TextEdit per block
Decision: static blocks render as `Text` + shapes; only the focused block
will own a real editing surface (M4), plus ListView-based virtualization
from the first commit.
Why: 10 000 interactive widgets is a guaranteed memory/CPU failure mode.
Consequences: caret/selection rendering inside the focused block must be
built deliberately (Slint TextInput + our selection model), not assumed.

## ADR-0002 · Windows IME = Slint + DirectWrite, no custom TSF
Decision: rely on Slint's winit text input (which uses OS IME composition
events); write a platform adapter only if a reproducible Slint/Windows bug
forces it.
Why: the previous input-method project proved TSF integration is the most
expensive kind of platform coupling; this app must stay agent-maintainable.
Consequences: Chinese input quality is a test item at M4, not a feature to
build.

## ADR-0001 · Slint + Rust, single process, no web runtime
Decision: Slint 1.18.x UI in the same process as the Rust core; SQLite for
persistence (M3); no Electron/Tauri/WebView/React/Vue anywhere.
Why: GPU-accelerated native rendering with real low-RAM/low-idle-CPU
behavior, and one language boundary (`.slint` ↔ Rust) that coding agents
can maintain for years.
Consequences: rich text editing and IME polish must be built rather than
inherited from a browser engine; this is accepted deliberately.

## Dependency policy
Every crate must answer: why needed / can std do it / runtime memory cost /
extra threads / build complexity. Current set: slint + slint-build only.
Anything else waits for a milestone that cannot be built without it.
