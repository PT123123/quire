# Architecture Decision Records

Format: decision → context → consequences. Newest first.

## ADR-0037 · Orphaned attachments get one reclamation path, and it cannot outrun undo

Decision: `reclaim_attachments()` in `AppState`, reachable as a **Reclaim**
button on the settings dialog's STORAGE row, deletes every `attachments` row
nothing can reach any more and, once the row is gone, the files beside it. The
reachable set is deliberately wider than "what is on screen": every block of
every page in `Document` (not just the open one), every attachment id named
inside any `Entry` on any page's undo **or** redo stack, and the block sitting in
the internal clipboard. It is the only emitter of the new
`Change::AttachmentDeleted`, and it reports through the same `db-notice` toast as
"Back up". The ordering rules are the substance: flush the debounced queue first,
write the row deletions synchronously, then delete bytes.

Why: orphans have three producers and no consumer. Undo removes a *reference*
and never the file, which `Change::AttachmentAdded` has said out loud since
ADR-0029 ("an orphaned picture is recoverable, a deleted one is not");
`delete_page` is not an undo step, so its pictures are unreachable the moment
the page goes; and `replace_all` leaves the whole table alone because it cannot
see the incoming references — the comment there called the cost "orphans, which
no user action can see". A local-first app with no cloud quota can live with
that for a while, but not forever, and the file it cannot see is the one it
cannot explain. The scan errs generous because the two failure directions are
not symmetric: leaving an orphan costs disk, deleting a picture Ctrl+Z was about
to restore costs the user's bytes.

Consequences:
- **The undo contract is unchanged, and now has to be proved.** A step still on
  the stack protects its ids from the sweep, in both directions — redo is one
  keystroke from putting a block back exactly like undo is. `History` grew the
  enumeration API it never had (`referenced_attachments()`), because until now
  nothing outside the module needed to know what the stacks hold.
- **The cap is the boundary of the promise.** 100 steps per page is what a
  picture is protected for, not "forever". `both_stacks_protect_and_the_cap_ends_
  the_protection` pins that boundary, and the settings row says in one line what
  the button deletes, because a toast is not a warning the user reads first.
- **One reader for "does this change name an attachment".**
  `core::persistence::attachment_ids_in` matches the arms that carry an id
  (`BlockInserted`, `BlockAttachmentSet`, `AttachmentAdded`) and nothing else, so
  the undo-shape tests and the reclaim cannot disagree about which arms count.
  `BlockDeleted` is not one of them: it is the change that *drops* a reference,
  and its undo carries the `BlockInserted` that names the id again.
- **Rows before bytes, queue before either.** Applying a DELETE out of order
  ahead of a still-queued `AttachmentAdded` would re-create the row after the
  sweep deleted the files it points at — a permanent dangling reference. So the
  reclaim flushes first and refuses to sweep if that write fails. A failure in
  the other direction only leaves bytes no row claims, which the next sweep
  removes.
- **What it does not reclaim: a file with no row.** A directory sweep looks
  tempting and is the one way to lose a library: if `load_attachments` failed
  this session, the in-memory book is empty, every file on disk looks
  unreferenced, and "I cannot see the references" would be answered by deleting
  them. The reclaim therefore reads the book and never the folder, so a broken
  load removes nothing. Unrowed bytes stay a known limitation.
- The decode cache gives its weight back for a deleted row (`evict_image`), so
  the 32 MiB ceiling ADR-0036 measured keeps accounting for rasters that can
  still be asked for.
- A reclaimed id can be **minted again** after a restart: `next_attachment_id`
  is `max(row ids)+1` at load, so reclaiming the highest row lets the next
  session reuse that number and its `<id>.png` name. Inside one session it
  cannot happen, and the session's own cache entry is evicted, so no stale
  raster survives the reuse.
- **It runs on the UI thread, so its cost is a frozen window and it was
  measured.** 1 000 orphans sweep in 549.9 / 556.7 / 567.6 ms (raw rows
  `benchmarks/results/2026-09-21-m10-reclaim-timing.jsonl`; an independent
  earlier batch read 530.6 / 542.3 / 573.0), and the same call with an empty
  book reads 0.0 ms — so the reachability scan is not on the clock and ≈0.55 ms
  per orphan is the `DELETE` transaction plus the `remove_file` calls,
  unattributed between them. That is a fraction of a second to a couple of
  seconds for a library a user actually accumulates, which is why there is no
  progress UI: the notice bar is the feedback, and a second click on a sweep
  that already ran deletes nothing new. Numbers in `docs/PERFORMANCE.md`.
- **The sweep cannot show the new button, and showed the row's other half
  anyway.** `storage-available` is false for the headless shot tool (it builds
  `AppState::new(&args, None)`), so no scene renders Reclaim — the settings
  dialog with a real database still needs a human window. What the re-sweep did
  catch is that a `visible: false` item keeps its slot in a Slint layout: the
  two hidden buttons were already costing the label width (it elided two words
  early) and the third made it worse, while the hidden caption below them cost a
  band of empty height. Both are visible in sweep17's `settings.png`; the row's
  actions are conditional children now, and sweep18 is the baseline.

## ADR-0036 · A picture page is measured by scrolling it, and the scroll had the wrong sign

Decision: give the media benchmark a real scene instead of another deferred
bullet. `--pictures N` turns every `rows/N`-th row of the bench page into an
`image` block; the fixtures behind them are `create_fixture`-generated 1280×720
gradient PNGs, written **only when the library is fresh**, so the measured passes
load a page that already exists rather than re-encoding one. `bench_picture_plan`
is the single source of the stride and the 200-file pool, which is why the row
builder and the seeder cannot drift apart. The app reports its own decode-cache
state — `attachment_cache_report()` prints one JSON line to stderr, and `--dump-state`
is what makes the bench harness read it — because a process working-set counter
cannot see a cache the size of a raster. `docs/PERFORMANCE.md` carries the nine
scenes.

Why: four consecutive batches ended with the same sentence — *a page of pictures
being scrolled is unmeasured, and the 32 MiB ceiling is a construction bound plus
a unit test, not a reading* — and a unit test is exactly the wrong instrument for
a claim about a ListView, a frame clock and a GPU texture upload. The seeding rule
is the honest half of the design: if the fixtures were built inside the timed
window, the arm would measure PNG encoding. The pool is capped at 200 rather than
one file per picture row because 5 000 rasters of distinct 1280×720 PNGs would be
a disk and warm-up artefact, not a memory measurement — and the cache cannot tell
the difference anyway, since it bounds bytes, not identities.

Consequences:
- **Scene F had been scrolling nowhere since M2.** `ListView`'s `viewport-y` is
  negative downward; the timer did `cur + 8.0`, so every 16 ms tick wrote a value
  the clamp immediately rounded back to 0, and the "continuous scroll" arm measured
  a stationary page. This is not a reading to reinterpret — it is every scroll
  number in this document taken before today. The fix is `cur - step`, plus a stall
  counter that only wraps when a position the app actually *put* there is the one
  that comes back.
- **A null reading was the bug, and pixels were the only proof.** The batch's first
  two runs printed an unchanged cache and the same CPU as the static scene. Rather
  than conclude "images are cheap", `--scroll-y` was added to `quire-shot` as a
  pixel control: at `0` and `+2000` it wrote **byte-identical PNGs**, and only
  `−2000` differed. That is the harness defect, photographed. It also means the
  contaminated rows were deleted and the whole batch re-run on the fixed binary;
  the pre-fix file is kept out of the repo (`%TEMP%\media-ram-pre-fix.jsonl`) as
  the record that the instrument, not the app, was wrong.
- **Scroll tests now need a direction test.** "The property changed" is not
  evidence of motion. `--scroll-step` exists so a flick can be measured separately
  from a wheel tick (83–88 % vs 36–38 % of a core), and `--scroll-y` stays as the
  control any future scroll arm should run before quoting its CPU.
- **The readings, briefly.** The cache stops at 9 rasters / 33 177 600 of
  33 554 432 bytes (98.9 %) in every scrolled media arm, at 500 or 5 000 pictures
  and on 1 000 or 10 000 rows — the ceiling is reached before the viewport could
  show a tenth frame, so it needs no raising. 500 pictures on a page you never
  scroll to is indistinguishable from no pictures. And an on-screen picture costs
  ≈9 MB of process memory, not the 3.7 MB of its raster: **the viewport, not the
  32 MiB cache, is what constrains a photo page**, which is the number §二十二's
  arithmetic now has to carry.
- `MUST NOT` the media arms into the §三十七 ≤1.2× gate. They are reported outside
  it deliberately — that gate's job is to catch a per-row projection regression on
  an ordinary page, and a scene that adds GPU textures would fail it for the wrong
  reason. ADOPT the same-session control for the batch's own arms (A, D and the
  media shapes all ran in one sitting), which is what the layout batch asked for.
- **Invalidating a number obliges re-running it, in both arms.** `scroll_ab.ps1`
  measures the fixed scene against femtovg and skia/GL in one sitting, each
  binary pre-flighted against its own reported renderer so a skia build that kept
  the femtovg default cannot pass for an A/B. The result: the verdict's direction
  survives — skia is ≈18 % cheaper at a wheel tick and ≈21 % at a flick — but the
  size halves (the M7 row read 18.2 % vs 27.7 % on a page that was not moving),
  and the price is ≈+34…43 MB of working set while skia's *private* bytes are a
  wash or lower. femtovg therefore stays the default on the same grounds as
  before, with "skia for scroll-heavy use" now a proven and smaller offer. The
  media ceiling turned out to be renderer-independent — skia reports the same 9
  rasters and the same 33 177 600 peak — because the LRU lives in `AppState`,
  above whichever renderer is painting.
- **A filter that matches nothing is not a measurement, and it used to exit 0.**
  Reproducing the new matrix scenes through `-Only A,B` found that
  `powershell -File` hands a comma list to a `[string[]]` parameter as **one
  string**, so a multi-token filter matched no scene, wrote no rows and reported
  success — the silent-probe shape this harness has been bitten by before.
  `bench_matrix.ps1` now splits the tokens and throws when nothing matched. The
  header comment has advertised the two-token form since scene E's follow-ups, so
  any older batch that claims a `-Only A,B` re-run is worth checking for rows
  before its numbers get quoted.
- `create_fixture` writes a gradient, which compresses well and is therefore not a
  photo; the RAM arms above are unaffected (weight is `w·h·4` regardless) but the
  disk footprint and any future decode-time arm are optimistic. Recorded as a
  limitation, not fixed here.

## ADR-0035 · A screenshot decodes in a file that has never seen the clipboard

Decision: 批次 A's last open code item — paste a picture from the clipboard with
Ctrl+V (SPEC §三十七, §二十七's "clipboard rich content") — is split into three
parts that can each be tested without the others. `platform::read_clipboard_image()`
touches Win32 only: it picks `CF_DIBV5` over `CF_DIB` with
`IsClipboardFormatAvailable`, retries `OpenClipboard` five times like the text
reader does, and copies `GlobalSize` bytes out. `platform/dib.rs` turns those
bytes into PNG and contains no Win32 at all. `AppState::paste_image()` decides
which block receives the picture. In `on_rich_paste` **text outranks image**: the
picture branch runs only when `read_clipboard()` returned nothing. No clipboard
crate, no new `image` feature — the PNG encoder the paste needs was already
compiled in for the store.

Why: ADR-0025 settled that clipboard access is hand-declared FFI, and this is
that decision's second half rather than a revision of it. The split is what makes
the risky half testable: a DIB is a header plus a bottom-up pixel array whose
masks, palette, row padding and bit widths are all chosen by whatever process
wrote the clipboard, and every one of those is a decoder bug — while none of it
needs a clipboard, so eight asserting tests build DIBs by hand and run on any
machine without touching the user's real one (the standing rule here is no
scripted desktop UI: no synthetic key events, no foreground stealing).
`CF_DIBV5` is
tried first because it is the only one of the two that can carry a real alpha
channel; `CF_DIB` is what everything actually writes.

Consequences:
- **The zeroed-alpha trap.** A 32-bit `BI_RGB` screenshot has no alpha, so its
  fourth byte is whatever the source had — usually all zero — and PNG *keeps*
  transparency: pasted through unchanged it renders an invisible picture, which
  the user reads as a failed paste. `decode` therefore paints the raster opaque
  when an alpha mask is declared and *every* alpha byte is zero, and leaves a
  picture with one non-zero alpha byte alone. Two tests pin both sides.
- **A hostile header stops being our problem.** `parse` accepts header sizes
  40/56/108/124, bit counts 1/4/8/16/24/32, and only `BI_RGB`/`BI_BITFIELDS`;
  it refuses `width > 16 384`, `height > 16 384` or `width × height > 64 M`, and
  checks the pixel array is actually there before allocating it. Without those,
  one buggy or malicious clipboard write — a header claiming 30 000×30 000 —
  would be an out-of-memory exit on the paste path, i.e. §二十二's low-RAM
  promise broken by a Ctrl+V.
- **Text wins because pasting a paragraph as a picture of itself loses the
  paragraph.** A "Copy" from a rich app puts both formats on the clipboard, so
  the order is a behaviour decision, not an accident of implementation.
- The paste reuses `AttachmentStore::import_bytes`, which is how a picked file is
  stored, so a screenshot gets the same `MAX_EDGE = 1280` downscaled display
  copy, the same content-sniffed extension and MIME (always `.png` here), and the
  same undo contract (ADR-0029: undo drops the reference, never the bytes). It
  stores under the name "Pasted image". The cost of the reuse is two decodes for
  one picture — DIB→RGBA→PNG, then the store's PNG decode — timed in
  `docs/PERFORMANCE.md` rather than assumed: 19 ms / 74 ms for the first leg at
  1080p / 4K, and 59 ms / 196 ms for the store leg at the same sizes with
  deliberately incompressible pixels. Both ends of that range are labelled as a
  floor and a ceiling rather than passed off as the real number.
- Where the picture lands follows the caret, so a paste never leaves a stray
  empty line: an empty block *becomes* the image, a block with text gets the
  image below it — the same shape the "/" and "+" doors have.
- Windows-only, like `read_clipboard`: other targets return `None`.
- Evidence: 8 decoder tests on hand-built DIBs (bottom-up flip, row padding,
  zeroed and real alpha, 5-6-5 vs 5-5-5 masks, palette with 2 and with 256
  entries, 1-bit MSB-first unpacking, truncated and RLE payloads refused), one
  state test with a real SQLite library in a scratch folder (block kind,
  attachment name/MIME/size, bytes on disk under `attachments/`, text preserved
  below, one paste = one undo step, undo keeps the bytes), and one `#[ignore]`d
  test that reads the *user's* clipboard — the only proof the FFI sees bytes
  another process wrote; on this machine it printed `clipboard picture: 1280x800
  from 134168 PNG bytes`. A green sweep proves none of this: the slice changes
  zero pixels (`.scratch/sweep15` vs `.scratch/sweep14`: 0 of 44 moved).

## ADR-0033 · An empty page writes its own first block

Decision: the M8 A4 item filed as "the empty state still says the block editor
arrives in a later milestone" was a functional dead end, not stale copy, and it
is closed with a new command. `Command::AppendBlock { kind, text }` puts one
block at the end of the page with no anchor; `AppState::start_page()` calls it
with an empty paragraph and hands back the new id so the controller can focus
it. Two doors use it: the empty panel is now a `TouchArea` (`empty-page-started`),
and committing the page title on a page with zero rows starts the first block
under it. Both are one undo step, and `plan()` refuses a container kind, the
same rule `InsertBlockAfter` carries.

Why: every existing insert command takes an anchor `BlockId` — after a block,
into a table cell, into a column box — and a page with no blocks has nothing to
anchor to, so `create_page` produced a page that could not be typed into at all
(`open_page` seeds nothing either). The alternative fixes were worse: seeding a
paragraph at `create_page` would litter the library with empty blocks on every
cancelled "New page", and doing it at `open_page` would make the empty state
unreachable and break the workspace test that asserts a fresh page projects zero
rows. Making the page produce its own first row on the user's explicit
interrupt keeps the document honest — a page is empty until someone means to
write in it.

Consequences: the empty state's copy is now an instruction ("Click here to
write, or press Enter in the title above.") and the panel must stay a click
target, so it cannot be turned back into a passive placeholder. Chasing that
one string surfaced a second stale promise of the same family in the demo
pages' placeholder paragraph — "changes live in memory for now; SQLite
persistence lands with the next milestone", false since M3 — now rewritten as
what actually happens; it is fixture text in the memory-only session the shot
harness runs, so it moves no scene pixels and no sweep can confirm it.
`AppendBlock` is deliberately
page-scoped and always appends — it is not a general "insert at position" and
does not become one; the 10 000-block page's "+" and Enter paths still go
through the anchored commands. Verified headlessly: 3 command tests (empty page
takes one paragraph and undo empties it again, an append lands below a
container's whole subtree, a grid or a layout is refused), the workspace test
now asserts `start_page()` yields exactly one row with the returned id, and the
click is proven on pixels — `quire-shot --scene empty --click 640,458` renders
the title plus one focused empty row with a caret where the panel used to be.
The sweep's `empty` scene moved 724 sampled pixels, all inside x 506..1032 /
y 456..466, i.e. only the copy line.

## ADR-0034 · Palette ids resolve through a Rust function the tests can walk

Decision: the command palette's integer id space is now decoded by
`palette_action(id) -> PaletteAction` in `src/app/state.rs`, and the
controller's dispatch is a `match` over that enum **with no wildcard arm**. The
`OpenPage(i32)` case carries the page id; an id that is neither a declared
command nor ≥ `CMD_PAGE_BASE` maps to `PaletteAction::None`. Two tests guard it:
one walks `mock_commands()` and asserts every row resolves to its own distinct
action (and no row to `None`), the other pins the boundaries (0, 14, 9 999, −1).

Why: the palette rows arrive in Rust as a bare `i32`, and the dispatch used to
be `match id { 1 => …, 2 => … }` with the named constants written as bare
identifiers. A constant that is not imported silently becomes a *catch-all
binding* in pattern position, so `aaa3763` shipped with every id ≥ 9 running
`copy_current_page_markdown` — no page could be opened from the palette — while
`cargo test` stayed green because nothing walked the dispatch. rustc warned
(unreachable pattern) and the warning was the only correct signal in the
pipeline. The repair ROADMAP.md:81 named as "still not written" is a test over
the registry, and a test can only exist if the mapping is a function: an enum
arm that cannot be shadowed by a missing `use`, and a match the compiler forces
to stay exhaustive when a variant is added.

Consequences: adding a palette command is now three edits in one file (a
constant, a `PaletteAction` variant, an arm) plus the action's body in the
controller, and forgetting the middle one is a compile error rather than a
silent hijack. The id constants stay in `state.rs` because that is where
`mock_commands` builds them. This closes the class for the palette only — any
other callback that crosses the Slint boundary as a bare `i32` has the same
exposure, so a future one should ride an enum the same way. Proven by the
registry test rather than by pixels: this fix changes 7 sampled pixels in the
`palette` scene (the sidebar hint now reads `Ctrl+\`), which is exactly the
evidence that a green visual sweep says nothing about behaviour.

## ADR-0032 · A layout is two levels of ordinary blocks, and it draws itself inside its own row

Decision: `columns` (SPEC §三十七 批次 B, M10's fifth slice) stores a layout as
**two levels of child blocks** and adds **no schema**. The `columns` block carries
its box count in the very same `blocks.columns` column v8 introduced for a grid;
each box is a `BlockKind::Column` child of it; each line inside a box is an
ordinary child of that box. One `Change` still carries the shape
(`BlockColumnsSet`) and every other edit is `BlockInserted` / `BlockDeleted` /
`BlockMoved` / `BlockTextSet` / `BlockKindSet`, so undo, redo and §十八 keep
working without knowing what a layout is. The UI projects the whole layout into
the layout's **single** delegate as a flat `[ColumnItem]` plus a `[ColumnBox]`
shape table (`id`, `column`, `first`, `size`) that slices it, and
`visible_block_indices` hides both the boxes and their lines — the same filter
ADR-0028 uses for a folded subtree and ADR-0031 for cells, so a layout is one
row however wide it is. Tiling is Slint 1.18's `FlexboxLayout` (one row,
`flex-wrap: no-wrap`, `alignment: stretch`, `horizontal-stretch: 1` per box), not
hand-written widths. The hover strip reuses `TableEdge` for Add / Delete column,
and `plan()` refuses below two and above three boxes.

Why: Slint has no recursive components, so a box cannot hold an `EditorBlock`
that itself holds boxes — the only place a layout's content can be drawn is the
layout's own row delegate, which is precisely what §三十七's "分栏只在可见窗口内
展开" asks for and what §十二's virtualization premise requires. Reusing v8's
integer is the same argument ADR-0030 made about v7: a box count and a column
count are one fact ("how many boxes does this container hold"), and a schema
version for a rename is not worth a migration. A box always holds at least one
line, and `ColumnsAddBlock` exists because an empty box is the one thing on the
page a click cannot put a caret in: the empty box says so, and the click asks for
its first paragraph.

Consequences: a box has no row of its own, so nothing outside the delegate can
address its lines by row index — Tab walks them through `column-item-move` and
stops dead at either end, ↑/↓ move the caret inside the line rather than leaving
the layout, and leaving a box is a click. Markdown export flattens: the
containers write no marker of their own and their lines come out at page depth in
reading order, which a test pins in both directions (re-import keeps every word
and loses only the shape), and the importer learns no columns syntax — the same
degradation ADR-0031 accepted for a grid. `InsertBlockAfter` had to stop treating
a container's row as its own slot: it now inserts after the container's whole
subtree and inherits the anchor's parent, which closes the identical hole for a
grid — "+" on a table used to be able to wedge a top-level block between the
table and its first cell, and a page's blocks are one flat slice sorted by
`order`, so a container whose subtree is not contiguous stops being one
container: `subtree()` walks off it, and every index-based reader with it. Two QOML findings came out of this slice and both
belong in UI_ARCHITECTURE.md: `for … : if … : Component` is a parse error, so a
filtered repetition has to become a shape table the delegate slices
(`ColumnBox.first` / `size`), and an identifier on the right-hand side of a
property inside a component instantiation resolves in the **enclosing**
component, so a child cannot be handed its own parent's `root.x` — the layout
passes `line-y`, `line-x` and `each-width` by plain names. The pointer-claim
protocol ADR-0031 needed for a grid crosses a component boundary here as
`in property pointer` plus `callback pointer-claimed(int)`, because N two-way
bindings onto one parent property is not something Slint lets you write. And the
"/" popup anchored itself with a hardcoded 350 px window reserve, which this
slice is the first to outgrow — the anchor now measures the real list height the
way the "+" menu already did. Deliberately not built: per-box width control
(Notion's resize handle), more than three boxes, and a layout nested inside a
box.

## ADR-0031 · A table is a block that owns its cells, and the grid is one editor row

Decision: `table` (SPEC §三十七 批次 B, M10's fourth slice) stores the grid as
**child blocks**, not as a payload. `BlockKind::Table` gains one stored number —
`blocks.columns INTEGER NOT NULL DEFAULT 0`, schema **v8** — and each cell is a
`BlockKind::TableCell` child of it, kept in row-major order by its ordinary
`order` key. Row count is therefore *derived* (`cells / columns`), never stored,
so adding a row is inserting `columns` blocks and nothing else moves. One new
`Change` carries the shape (`BlockColumnsSet`); every other table edit is plain
`BlockInserted` / `BlockDeleted` / `BlockTextSet` / `BlockKindSet`, which means
undo, redo and the §十八 storage contract all keep working without knowing
what a table is. The UI projects the whole grid
into the table's single delegate as a flat `[TableCell]` + `columns`, and
`visible_block_indices` filters cells out the same way ADR-0028 filters a folded
subtree — a table is one row, always, however big it gets.
Why: the alternatives are a JSON blob column (a cell would stop being a block,
so inline marks, undo granularity and §三十九's later relation/rollup work all
have to be reinvented outside the model) or a real table-per-row schema (§三十九
owns that, and SPEC is explicit that this kind is *not* a database view). Cells
as blocks cost one integer column and reuse everything. The two rules ADR-0028
added for dynamically-rowed kinds apply here and are honoured: the projection
really drops the hidden rows, and every row-index consumer goes through
`visible_block_indices`. A grid whose cell count is not a multiple of `columns`
is refused by `grid()` rather than repaired — `plan()` says no instead of
guessing which row is short, and `DuplicateBlock` will not copy such a table at
all, because a cell-less grid opens broken.
Consequences: cells never appear in the row list, so no code that reads
`rows[i]` can be handed a cell — `SetBlockType` refuses a cell as both source
and target, and a cell's kind belongs to its grid, not to the Turn-into menu.
Turning a block *into* a table moves its whole line into the top-left cell of a
fresh 3×2 grid rather than dropping it, because a table block draws no text of
its own; flattening it back turns each cell into a paragraph, so the words
survive both directions — the *marks* do not, since `new_cell` starts clean and
a mark range pointing at the old block's text would mean nothing inside one
cell. Adding a column is the one command that must insert
`rows` blocks into `rows` different gaps at once, which is why
`OrderKey::STRIDE` is 1<<16 and why `renumber_page` now spreads with it: one
midpoint per insert halves a gap, so a batch has to be keyed whole. The
projection reads a table's cells by scanning the page, so it is guarded on
`kind == Table` — unguarded it ran that scan for all 10 000 rows of a bench page
and allocated an empty model with each, which measured ≈2 MB of working set on
D and nothing on the 24-row A (`docs/PERFORMANCE.md`). Markdown
export writes GitHub-flavoured tables (first row is the header, `|` escapes to
`\|`, a newline in a cell becomes a space); **import does not read them** — a
`|a|b|` line stays a paragraph, pinned by a test in `markdown_test.rs`, because
the importer's shape is line-at-a-time and a table needs lookahead. Deliberately
not built: Enter splits a cell, backspace merges, Up/Down cross rows, Ctrl+L
marks a link in a cell, and a cell's text width does not feed back into column
widths.
Hover-only UI needed a new shot capability: `quire-shot --hover x,y` dispatches
`PointerMoved` without a button, which is the only headless way to light the
edge toolbar; `table` and `table-edit` are in the sweep but the 22 px strip they
show is verified by a manual `--hover` shot, and hovering a table reflows the
content below it by exactly that much. Two Slint 1.18 geometry findings came out
of this slice and both belong in UI_ARCHITECTURE.md: an element in a non-layout
parent that binds `height` but leaves `y` unbound is rendered **centred** in that
parent, so `DocumentRow.head` had been sitting 34 px below its binding in every
scene ever shipped and the table — the first tall *first* block — finally painted
over the page title; the fix is explicit `x: 0px; y: 0px;`. And `absolute-position`
readings inside a ListView delegate are not trustworthy (row 1 reported its head
below its own body with both at `y: 0`), so the delegate's real geometry is
`parent.y + head.height`, which is what `row-y` already passes down. Verified
headlessly: `cargo test --features software` green, and the 37-of-42 changed
scenes in the new baseline all differ **only** inside the title band, which is
the fix above and not the table; the projection guard was re-swept afterwards
and came back 42-of-42 byte-identical, so it is a memory change and not a
visual one.

## ADR-0030 · A file attachment is stored without ever being looked at

Decision: `file` (SPEC §三十七 批次 A, M10's third slice) adds **no schema**. A
file is a block kind, and the v7 columns ADR-0029 introduced for pictures —
the `attachments` row plus `blocks.attachment` — describe it exactly as well,
so the slice ships with `user_version` unchanged. What is new is that the bytes
never enter the process: `AttachmentStore::import_any_file` `fs::copy`s the
picked file straight to `<library>/attachments/<id>.<ext>` and records its
length, with no decode, no `image` call and deliberately **no size cap** — a
2 GB attachment is stored in 2 GB of kernel-side copying and 0 of working set.
The row paints three things from the database line alone: the name, a
`format_size` label, and two buttons. Open goes to `platform::open_with_default`
(a hand-declared `ShellExecuteW`); Save-as goes to `rfd` + `export_to`, which
copies the stored bytes out under `save_name`. Undo drops the reference and
never the bytes, unchanged from ADR-0029.
Why: the priority list in ROADMAP.md puts low RAM above feature count, and the
§二十二 promise is the one this kind would break first — the obvious
implementation, read the file into a `Vec` and write it back, is a 2 GB working
set for a note. The picture path solves the same problem by downsampling; the
file path solves it by never looking. `ShellExecuteW` rather than
`spawn("explorer.exe", path)` because explorer returns 0x1 **on success** by
design, so a subprocess can never tell a launched app from a blocked file
association, while the FFI answers `> 32` only for a real launch — and one
extern declaration keeps the `windows` crate out, the same rule ADR-0025
follows for the clipboard.
Consequences: the label keeps its extension while a picture's drops it, because
for a picture the content is the truth and for a `.zip` the name is; that split
is why `save_name` exists (it rejoins stem + extension for pictures, and leaves
a file name alone — case-insensitively, since `file` lowercases the extension
and `name` keeps the user's). Open is an explicit button rather than
click-the-row, because a row that launches an arbitrary executable on a stray
click is not a row you can safely select text in; the two buttons stay drawn
rather than hover-revealed, since a file block that shows nothing to press
reads as a broken attachment. The size is a `pure callback attachment-size(int)`
rather than a `BlockRow` field so the string is not rebuilt for every row on
every keystroke — the same viewport-not-document argument ADR-0029 makes for
the rasters. Markdown export writes `[name](quire://attachment/<id>)`, a
*link* rather than the picture's `![](...)`, and that is the one place the two
kinds differ in the exporter: the importer has no picture shape but it does have
a link shape, so a file's reference survives an export/import round trip and a
picture's does not. Still open here when this was written: orphaned bytes (no
FK, no cascade, same gap as pictures — ADR-0037 gives it one reclamation
path), clipboard-bitmap paste (ADR-0035), and the PDF first-page thumbnail,
which the user deferred on 2026-09-20 — until it lands a PDF and a `.zip` look
identical apart from their names. Verified headlessly, not by eye: 36 of 40
scenes byte-identical against the previous sweep, `slash`/`plus`/`dark-slash`
moved only because the menus gained the File row, and `file` is new — a
760×48 px box at x 390..1149 with the label left, "1.8 MiB" right-aligned, and
two 26 px buttons at x 1086..1112 / 1116..1142. The fixture's first draft put
`std::process::id()` in the file name, which made the label different every
run; the id moved to the temp *folder* so the scene is reproducible.

## ADR-0029 · Pictures are files beside the database, and the editor never loads the original raster

Decision: `image` (SPEC §三十七 批次 A, M10's second slice) stores its bytes in
`<library>/attachments/<id>.<ext>` and keeps only a reference in SQLite — a new
`attachments` table plus `blocks.attachment INTEGER` (schema v7). The column
carries **no foreign key on purpose**: a block whose file row has vanished must
still load and render as a missing picture, because a user who copies the `.db`
without its folder owns a library, not a crash. Import downsamples anything
longer than `MAX_EDGE = 1280` px into a second file, `<id>.cache.png`, and
`AttachmentStore::display_path` hands the UI only that copy — the original
stays byte-identical on disk, so viewing never degrades the user's file. The
decoded rasters live in a cache owned by `AppState`, keyed by attachment id and
capped at 32 MiB weighted by RGBA bytes, evicting the least-recently-realized
entry first. Rows reach it through the `image-for` / `image-aspect` **callbacks**
rather than model fields, because Slint invokes a binding only for the rows it
realizes. Undo drops the reference and never the bytes: `Change::AttachmentAdded`
is an upsert and there is deliberately no `AttachmentDeleted`.
Why: Slint's own decode cache is a thread-local `CLruCache` capped at 5 MiB
weighted by decoded bytes and keyed by path + mtime (`i-slint-core-1.18.0`,
`graphics/image/cache.rs`). One 1280×720 RGBA frame is 3.7 MiB, so that budget
is a single photograph — scrolling a page of pictures through it re-decodes on
every frame. The obvious alternative, an uncapped cache of our own, is how a
low-RAM app dies quietly, and §二十二's promise sits on the first slot of the
priority list in ROADMAP.md. Callbacks are what keep the cost proportional to
the viewport instead of to the document: a page with five hundred pictures
holds about ten.
Consequences: the 32 MiB ceiling is ≈eight full-width frames, and it is a
construction bound, tested as one (`the_picture_cache_spends_its_budget_and_
drops_the_stalest_first` also pins LRU over FIFO by re-showing an evicted id
mid-scroll). A failed decode caches as a zero-weight blank so a missing file is
not re-opened per repaint — which also means replacing a file on disk in place
needs a restart to show up, since our key is the id and not the mtime. The
extension and MIME come from sniffing the bytes, so a screenshot saved as
`.jpg` but encoded PNG is stored as the PNG it is. `blocks.img_percent`
(25 / 50 / 100, default 100) is the width tier; setting it back to the default
plans no change, so the menu cannot leave an undo step that does nothing.
Markdown export writes `![name](quire://attachment/<id>)`; the importer has no
picture shape, so the line survives as literal text and the file name is never
lost — the reference goes out, nothing comes back, which is the same asymmetry
§三十七 accepted for toggle folds (ADR-0030 gives `file` the link form, so that
one does round-trip). Of what this kind left open, the clipboard-bitmap paste
has since landed (ADR-0035); the PDF first-page thumbnail reuses this store
unchanged and is what the user deferred on 2026-09-20.

## ADR-0028 · A folded subtree gets zero realized rows, so row indexes stop being model indexes

Decision: `toggle` (SPEC §三十七, M10 批次 B's first slice) persists its fold
as a `blocks.folded` column (schema v6) and hides its subtree by **filtering
it out of the projection** — `project_blocks` builds rows through
`visible_block_indices`, so a hidden block has no `BlockRow`, no delegate and
no height. There is no `visible: false` row. Because that makes the editor's
row numbering independent of the document's, every consumer that treats a row
index as a model index must translate first; the one that exists today (the
§八 drag landing) goes through the new `AppState::drop_index_for_row`, which
shares `visible_block_indices` with the projection.
Why: Slint's `ListView` realizes exactly one `for` child per item, so a
0-height delegate would still cost a component, a binding and a slotmap
lookup per hidden block — on a page whose top section is folded that is the
whole point of folding, paid for and thrown away. Deleting the row is also
what makes the behaviour correct by construction: a hidden block cannot be
tabbed into, dragged, found-by-⌘F or renumbered if it is not in the model.
The cost is that two numbering systems now coexist, and the repo had been
using them interchangeably.
Consequences: fold is undoable view state, not content — `Command::ToggleFold`
emits `Change::BlockFoldedSet`, the same shape `PageExpandedSet` already uses
for the sidebar, so it rides the persistence queue and never bumps a document.
Only a Toggle draws the chevron, so `SetBlockType` away from Toggle clears the
fold (undo restores it) rather than stranding a subtree with no way back;
`SplitBlock` and `DuplicateBlock` force `folded: false`, because neither one
copies a subtree and a fold over nothing is a trap. Markdown export degrades a
toggle to a quote line — CommonMark has no fold syntax, so it degrades exactly
like a callout already did, while the subtree still rides along indented and
import never restores the fold (§三十七 asks for precisely that asymmetry).
And SPEC §三十七's 硬性约束 gained a rule for the next dynamic-row kind
(`table`, `columns`): the projection must really delete rows, and new
row-index consumers must translate. Verified headlessly, not by eye: 32 of 35
baseline scenes are byte-identical after the change, `slash`/`plus`/
`dark-slash` moved only because the menus gained the Toggle row, and
`toggle` vs `toggle-fold` differ by 6 006 px in exactly four places — the
triangle glyph, the vanished child line, the one-row reflow below it, and a
2 px taller scrollbar thumb.

## ADR-0027 · The parked feature set is unscheduled, not cancelled; only six capabilities stay out

Decision: of everything the SPEC had deferred, only sync, real-time
collaboration, cloud, plugin market, AI, publish-to-site and
comments/discussions/reactions remain out of scope (SPEC §三十三, extended
2026-09-20 with the last two). Everything else that §九 and §十七 had
written as "后续再加" is now a scheduled phase with a milestone: §三十七 →
M10/M11 (image, file, PDF, table, toggle, columns, then highlighting,
bookmark, embed, math, TOC), §三十八 → M12 (icon, cover, page font /
full-width / small-text, lock, version history, templates), §四十 → M13
(@-mention, backlinks, synced block), §三十九 → M14 (Database). Ordering
follows "how fast a daily note hits the wall", not Notion's feature
alphabet: media and structure blocks before the database, and the database
only after the reference layer exists, because `relation` and the simple
table would otherwise each get their own ad-hoc version of it.
Why: an audit of SPEC.md against Notion found three different things being
reported as one — items genuinely ruled out (§三十三), items deliberately
deferred with a spec line (§九), and items never written down at all
(page-level properties, backlinks, templates, version history, all views
and property types of the database layer, i.e. the largest single gap).
The third group was the problem: §三十五's "feature count ranks last"
had been read as licence to leave them unwritten, so they could not be
planned, estimated or refused. Recording them costs nothing now and makes
"第一期不做" an explicit statement per item instead of a blanket.
Consequences: §三十五's priority order still binds each of these phases,
so every one carries a measured gate rather than a checkbox — M10 must
keep the 10 000-block scene inside 1.2× the current RAM baseline (image
downsampling is where a low-RAM app normally dies), M14 must do filter and
sort in SQL and never realize 10 000 rows (SPEC §三十九's red lines), and
math/highlighting/embed may not smuggle in a JS, WASM or WebView runtime
that §二 forbids. `person` degrades to a local name list because there is
no account model to point at. New sections were appended as §三十七–§四十
rather than inserted in phase order: existing cross-references (ROADMAP
cites §六/§十六/§三十五, ADR-0026 cites §八) are all by section number, and
renumbering would have silently broken every one of them.

## ADR-0026 · Page and Link-to-page blocks share blocks.page_ref; ownership is a kind-level contract
Decision: both page-bearing block kinds — `Page` (kind 11) and `Link`
(kind 12) — point at a page through the SAME nullable `blocks.page_ref`
column (schema v5) and the same `BlockRefSet` change; no per-kind column or
variant. What differs is **ownership**, decided by the kind:
- `Page` **owns** its child page (created in the same batch as the block).
  Deleting the block deletes the child; duplicating the block deep-copies
  the child and retargets the copy; pasting one lands the title as plain
  text (two blocks must never share an owned page). One `BlockRefSet`
  without a live `PageCreated` is still legal — storage stores the pointer,
  the lifecycle is the caller's composition.
- `Link` **references** an existing page picked in the page picker. Delete,
  duplicate and paste never touch the target; sharing a reference is the
  point. Turning either kind into another kind via the ⋮⋮ menu drops the
  reference (`BlockRefSet` → `None`); a Page's child survives in the tree,
  unowned.
Why: Notion's Page block and Link-to-page differ exactly in lifecycle, not
in data shape — one column keeps schema and export uniform (`[title]
(quire://page/<id>)` for both, the ownership difference deliberately does
not survive Markdown), and the contract stays at one change variant
instead of two. The page picker reuses the slash popup in a
`slash-pick-page` mode rather than a fourth popup, so anchor/keyboard/
close behavior has one implementation.
Consequences: a dangling `page_ref` (child deleted from the sidebar) renders
"(deleted page)" muted, and the block still opens nothing — no cascade from
block to tree outside the explicit delete path. A duplicated page's own
embedded Page blocks still reference the ORIGINAL children (recursive copy
is v2). Keyboard focus-move can land on a page row; the row is
never-editable by kind, so the input simply does not appear.

## ADR-0025 · Clipboard reads are direct Win32 FFI — no clipboard crate, no subprocess
Decision: `platform::read_clipboard` opens the clipboard through
`OpenClipboard` / `GetClipboardData(CF_UNICODETEXT)` / `GlobalLock` declared
in-module (`user32`/`kernel32`, already linked for winit) — writes stay on
`clip.exe` stdin.
Why: rich paste (§二十七) needs the text *any* source app put on the
clipboard. The zero-dependency alternatives both fail: `clip.exe` is
write-only, and a `Get-Clipboard` subprocess measured **7–10 s** on the dev
desktop (PowerShell startup under AV) — no paste can wait for that. A
clipboard crate (arboard) would be the first new runtime dependency since
M3 to buy ~40 lines of FFI the toolchain already links. CF_UNICODETEXT is
the format every text source supplies; UTF-16 → `String` via
`from_utf16_lossy`, and a 5×2 ms retry rides out transient clipboard locks
held by other processes. Writes keep `clip.exe` because the only payloads
the app writes (`quire://` links, ADR-0023) are ASCII, so the
OEM-codepage stdin caveat is moot.
Consequences: reads are microseconds and CJK-safe from any source app;
writing non-ASCII *from the app* would garble through `clip.exe` — the day
a feature needs that ("copy page as markdown"), switch the write path to
the same FFI (`SetClipboardData`) rather than adding a crate.

Update (2026-09-20, same day): the day came with "Copy Page as Markdown" —
the write path now runs through `SetClipboardData`/`GMEM_MOVEABLE` FFI too
(`copy_to_clipboard` swapped its clip.exe internals for the FFI; callers
unchanged, no crate). `clip.exe` is fully retired from the codebase; the
round-trip test (`clipboard_write_and_read_round_trip_unicode`) pins CJK
through write → read.

## ADR-0024 · The release profile stays as shipped — no fat LTO, no panic = abort
Decision: `[profile.release]` keeps thin LTO + `codegen-units = 1` +
`strip = "debuginfo"`. Fat LTO is rejected, `panic = "abort"` is rejected,
`codegen-units = 16` is rejected.
Why: SPEC §二十四 only keeps levers that win on runtime memory / CPU /
startup — exe size ranks last. The 2026-09-20 four-way comparison
(`docs/PERFORMANCE.md` "M8 · release profile audit", Track B A3) measured
idle private bytes 88–90 MB, a 10 000-block page at 97–99 MB, idle CPU
0–0.6 %, typing 24–30 % and warm window-up 85–192 ms across all four
profiles — one noise band. Fat LTO saves 2.5 MB of exe for a ×2.5 build
time; `codegen-units = 16` *adds* 2 MB; `panic = "abort"` is the only
"smaller and faster" option and is vetoed on behavior, not numbers: abort
does not unwind, so `logging::install`'s panic hook never runs,
`panic-report.txt` never lands and `last_session_aborted` is blind exactly
when it matters — the ADR-0018 / SPEC §二十五 crash-recovery chain dies
with it. `strip = "debuginfo"` is the symbol policy: linker debuginfo
stripped, the COFF symbol table kept; "debug artifacts separated" is
`just dist` preserving `target/release` originals, not symbol stripping.
Consequences: none at runtime — this ADR mainly pins what must NOT change.
The conclusion expires when a lever moves idle private bytes or typing CPU
beyond ≈2 MB / ≈3 pp of the measured noise floor; re-run
`benchmarks/scripts/profile_bench.ps1` per profile before believing any
single-run delta (the audit caught a parallel-build-polluted batch that
looked like a 20 % win and was not).

## ADR-0023 · The ⋮⋮ menu gets Notion's remaining items; block color crosses the persistence contract
Decision: the block handle menu carries Copy link to block, Move to, and
Text/Background color (Comment / Suggest edits / Ask AI stay out with the
rest of collab+AI). "Copy link to block" puts `quire://block/<id>` on the
system clipboard via `clip.exe` (ASCII-only payload, so no clipboard crate
— dependency policy holds); `open-link` now resolves `quire://block/` and
`quire://page/` in-app first (jump to the page, focus the block) and only
shells out for foreign URLs, which also makes Ctrl+L links to internal
anchors clickable. "Move to" lists every other page depth-indented and
commits `Command::MoveBlockToPage`: the root lands appended to the target
page's top level, the subtree travels as one `Change::BlockMovedToPage`
per block (children keep their parent pointers and orders), one undo step
end to end. Color is a block-level pair (`ColorKind` text + background,
10 slots, Default = theme), one `SetBlockColor` command, one
`BlockColorSet` change, and schema v4 (two TEXT columns on `blocks`,
'' = default; the ALTER runs conditionally in code because SQLite has no
`ADD COLUMN IF NOT EXISTS` and the migration test legitimately re-runs v4
on a hand-downgraded file). A Callout block joins the kind set — the last
Basic kind that neither a symbol shortcut nor the v1 exclusions cover;
it renders a tinted rounded box with an emoji and exports as a quote.
Why: the user asked for the ⋮⋮ menu to match Notion's, minus what typing
symbols already reaches (ADR-0022 curation unchanged and now spanning the
new kinds).
Consequences: colors are cosmetic by design — no Markdown representation,
so export/import round trips drop them (callouts degrade to quotes);
`clip.exe` means copy-link is a Windows-only nicety until a clipboard
crate earns its place; menu popups size their anchor clamp from the live
row count, so the tall root menu and the 11-row color palettes stay
on-screen; the "current color" check is drawn as an icon because the
software renderer has no font fallback (a ✓ glyph rendered as nothing in
headless captures).

## ADR-0022 · Markdown line-shortcuts; the menus list only what symbols can't reach
Decision: typing a trigger at the block start converts the block live —
`# `/`## `/`### ` → Heading 1/2/3, `- `/`* ` → Bullet, `12. ` → Numbered,
`[] `/`[ ] ` → To-do, `[x] ` → checked To-do, `> ` → Quote, `---` →
Divider, ` ``` ` → Code — as one undo step (`exec_all`, which now skips
no-op parts of a compound command instead of failing wholesale). The slash
menu and the ⋮⋮ "Turn into" list carry only Text/Code/Divider: every other
kind has a symbol path. Notion's Comment / Suggest edits / Ask AI / Color /
Copy-link / Move-to stay out (collab+AI are v1 out-of-scope; no block
anchors or cross-page moves yet).
Why: the user wants the pickers short — entries reachable by blind typing
are noise; the symbol is the only path to the removed kinds, which keeps
the menus honest.
Consequences: code/divider blocks are exempt from conversion (their text
legitimately starts with these characters); converting back among symbol
kinds works by typing the other symbol at the block start. Fixed while
wiring: the ⋮⋮ popup was driven by a `block-menu-open` bool only the bench
scene ever set — a real grip click raised just `block-menu-open-id`, so the
menu never appeared and had no coordinates. The popup now follows the id
and the bool is gone; menu geometry anchors beside the handle from
delegate-reported viewport coordinates (DocumentRow layout-y + handle
offset), which also fixes the slash menu anchoring every block at the first
row's offset.

## ADR-0021 · Drag-reorder rides Slint's built-in `DragArea`/`DropArea`
Decision: the grip handle wraps its TouchArea in a 1.18 `DragArea`
(`allow-move`, payload = plain-text `slint-notion/block:<id>` built in the
controller); every editor row is a `DropArea` whose `can-drop` validates the
landing through a read-only `can_move_block_to` check and parks the accent
drop line in `UIState.drop-line-row`; the drop commits one new
`Command::MoveBlockTo` (insert-above flat index), so one undo step and one
persistence entry cover a multi-row move.
Why: SPEC §1564 says to prefer Slint's built-ins over hand-rolling; the
in-window drag path lives in core (press-filter → threshold → DragMove/Drop
routing), so it works with every renderer on Windows without OS DnD, and a
click below the threshold still reaches the menu TouchArea. Reordering on
drop (not per-hover swap) sidesteps the ListView delegate-reuse trap, where
mutating the model mid-drag would swap the data under the dragging delegate.
Consequences: the landing must not split a subtree (top-level inserts may
only sit above another top-level block; nested items stay adjacent to their
sibling run) and nested-swap `MoveBlock` now re-sets the same parent instead
of flattening children to top level (pre-existing bug found while planning).
Dragging across a virtualized viewport does not auto-scroll the ListView
yet; revisit when long-document drag matters.

## ADR-0020 · The library moves to `%APPDATA%\Quire`; `--db` and `--portable`
stay
Decision: `storage::data_location` is the single answer to "where is the
library". Opening the pre-D12 default — `appdata/quire.db`, relative to the
working directory — is redirected to `%APPDATA%\Quire\quire.db`, and the first
such redirect carries the old library across whole: the main file, the
`.bak<N>` family, the `-wal`/`-shm`/`.corrupt` sidecars a crashed session left,
and the `quire.log` family that shares the folder. A caller that names a file
of its own (including `--db <path>`) is taken at its word and never rerouted;
`--portable` asks for the old behavior out loud, which is what a stick install
wants and what the benchmark harness uses to keep out of real notes. Each file
moves as copy → open-the-copy → one rename → delete the source, staged through
a `<destination>.migrating-<pid>` name. The idempotence key is the destination:
once `%APPDATA%\Quire\quire.db` exists there is nothing to migrate, so a restart
— or a move interrupted after the first file — cannot overwrite a library that
has since been edited.
Why: with the installer in place (ADR-0017) the working directory is whatever
the shortcut's "Start in" happens to say, so launching Quire from two folders
creates — and seeds — two unrelated libraries, and the user finds out when a
note is missing. That same relative path is also what forces the per-user
install; once the data has its own home the premise behind ADR-0017's
"the install folder must be writable" disappears. The copy is opened before the
source is retired because a `fs::copy` of a database another session is writing
can be torn, and a damaged library discovered at the new path has no source left
to fall back to; refusing the move keeps the old folder — snapshots included —
exactly where `backup::open_with_recovery` expects it. Falling back to the
legacy path on any error rather than opening the empty per-user file is the
difference between "it did not move" and "my notes vanished".
Consequences: the log follows the database, since `services::logging::data_dir()`
now asks storage — a `--db` run writes its log beside that file, which also
keeps the benchmark runs out of the checkout. Three things this track could not
finish: `main.rs` still creates `appdata/` beside the working directory before
opening it, `LaunchArgs` has no `portable` field so the two flags are re-read
from the command line here (`scan`), and `OpenReport` has no field to say a move
happened, so the user only ever sees one stderr line. All three are in
`M8_FEEDBACK.md` #13 with the lines to write. `%APPDATA%` roams with the
domain profile, which is what the milestone asked for and is fine at this
database size; if a roaming profile ever turns out to be slow for a live SQLite
file, `roaming_root()` is the one function to change.

## ADR-0019 · Backup retention is two windows; the periodic snapshot rides the
flush tick
Decision: `storage::backup` keeps five generations (ADR-0015 had three) *and*
drops any generation whose modified time is older than `MAX_AGE` = 7 days. Both
are applied by one `prune(path, now)` that `snapshot()` calls after a successful
copy and whose return value is discarded: cleanup is housekeeping, and a locked
or vanished old file must not turn a good snapshot into a reported failure.
`prune` scans `KEEP + 2` slots so a family left behind by a larger `KEEP` cannot
survive unboundedly (`recover` only walks `1..=KEEP`, so anything past it is
unreadable weight). Mid-session insurance is `PersistenceService`'s: a snapshot
hook attached with `with_snapshotter(interval_ms, hook)` — or the app's one-liner
`with_database_snapshots(&repo)` — runs from the flush path that is already
ticking (`flush_if_due`, `force_flush`), gated on two facts: the period
(default `DEFAULT_SNAPSHOT_INTERVAL_MS`, ten minutes) elapsed since the last
snapshot, *and* something was written since then. `SqliteRepository` now records
the path it opened and exposes `snapshot()`, so the hook is a method call on the
`Arc` the app already holds; a failure is stored for the UI to take
(`take_snapshot_error`) and logged, never returned as the flush's error.
Why: the milestone forbids a second resident thread, and the app already arms a
timer per recorded burst — a snapshot that rides that tick costs no new
scheduling and, because of the `pending` gate, no work at all in a session that
changed nothing. Count-only retention keeps a five-week-old `.bak5` alive for a
user who opens Quire once a week; age-only keeps five copies of a scene-D
workspace forever, which is the memory ADR-0015 accepted at three generations
and should not silently triple. Age comes from the file's own `modified()`
because the snapshot's whole life is that one write, and `now` is a parameter for
the same reason the debounce window takes a clock: a test ages a file with
`File::set_times` rather than waiting a week. Gating on the write rather than
running on the wall clock is what keeps a 2.3 MB `VACUUM INTO` from repeating
while the user reads.
Consequences: ADR-0015's "deliberate omission" (a long session had no insurance
until the next open) is closed — the loss window is now "since the last tick",
but only for a session that keeps editing, because the app's timer is
single-shot: an idle window takes no snapshots, and ten minutes is a minimum gap
rather than a cadence. Making it exact is an app-side `TimerMode::Repeated`
(M8_FEEDBACK #12 records the wiring line and this reading). A monthly user can
end up holding one generation instead of five — age wins, which is the point of
the second window. `services/persistence.rs` now names `SqliteRepository`
concretely, the same trade ADR-0014 made for `search`, and `snapshot()` takes the
connection mutex, so a snapshot serialises against writes by construction rather
than by a new lock. Retention covers `.bak<N>` only: the `.corrupt` corpse and
D9's log family still have their own rules.

## ADR-0018 · Logging is one rotating file plus a panic report the next start
reads back
Decision: `services::logging` owns the app's only log file, `quire.log` beside
the database, with a family of three (`quire.log`, `.1`, `.2`) and a 1 MB ceiling
per file — size is checked before each append, so a line may overshoot by its own
length and nothing else does. Records are one physical line: `2026-09-19T08:21:55
.169Z [info] …`, UTC, with newlines in a message escaped to the two-character
`\n`. `init()` is the single line `main` calls; it creates the directory, writes
the startup record, and installs a panic hook that chains whatever hook was
already there. On panic the hook appends a `[panic]` line and writes
`panic-report.txt`; the *next* `start()` reads that file, records it as the
`last_session_aborted` entry of `session.meta`, logs the same fact, and deletes
the report so a restart cannot blame the old crash twice. `session.meta` is a
line-oriented key/value file — the file-backed twin of the `metadata` table — and
`meta_entries()` hands it to whoever holds a repository.
Why: the alternatives all cost more than a crash trace is worth. A `log`/`tracing`
facade would add dependencies to a four-crate tree to gain levels nobody tunes,
and an async writer would need the thread this milestone is explicitly avoiding.
Writing the abort *metadata* on the next launch rather than inside the panic is
the same argument in miniature: a panicking process may die at any moment, so it
gets two plain writes while the read-modify-write of a metadata file runs on a
thread that is known to be healthy. UTC rather than local time because a log
timestamp and a file's modified time must be comparable when the two disagree
about nothing else. The one line in `main.rs` is the ceiling the milestone allows,
which is also why the logger cannot reach the database: it starts before the
repository exists, and `src/app/**` is off-limits (M8_FEEDBACK #9 records the
wiring Track A still owes).
Consequences: a hard crash (access violation, kill, power loss) leaves no report,
so `last_session_aborted` means "a Rust panic", not "the session did not end
cleanly" — closing that gap needs an exit-path call in `main.rs` (one more line)
or a clean-shutdown hook in the app layer. Logs currently land in the working
directory's `appdata/` until D12 moves them into `%APPDATA%\Quire\` with the
database, which also means a portable run keeps its log beside its own database.
The `Logger` is constructed per directory and every write opens the file, so
tests rotate against a 200-byte limit instead of a megabyte.

Update (2026-09-20, D13): the exit-path call landed — `main` notes
`END_RECORD` ("session ended") after the final flush, and `start()` reads its
absence, with no panic report, as an unclean end. The gap this ADR originally
recorded is closed: `last_session_aborted` now means "the session did not end
cleanly", with panic summaries still verbatim ("panicked at …") and the new
shape worded apart ("did not shut down cleanly (killed, crashed natively, or
lost power)"). The tail check follows the rotation family, so an end-record
that shifted into `.1` still reads as clean, and an empty family is a first
run, not a kill. Two residual false-positive sources are accepted: a second
instance reading a live session's tail (no single-instance guard), and any
external kill of a bench/scene run — both *were* unclean ends, the banner
just cannot say who did the killing. The app consumes the session.meta entry
on the next start (M8_FEEDBACK #9 wiring): it lands in the `metadata` table
and in the notice bar, then is deleted so it cannot re-report forever.

## ADR-0017 · Shell identity is a build-time resource; the installer installs
per-user
Decision: the exe's icon and version block come from a resource script that
`build.rs` generates — `IDI_MAIN` pointing at `install/quire.ico` plus a
`VS_VERSION_INFO` whose numbers it reads from `CARGO_PKG_VERSION` — and hands
to `rc.exe` through the `embed-resource` build-dependency. `install/make_icon.ps1`
renders that `.ico` with GDI+ (rounded gradient tile, ring + tail for the Q,
seven PNG-compressed frames from 16 to 256 px), so no image tool enters the
repo. `install/quire.iss` (Inno Setup 6), driven by
`install/build-installer.ps1`, produces `dist/Quire-<version>-windows-x64-setup.exe`
with a Start-menu entry, an optional desktop shortcut, an uninstaller, and an
unchecked task that adds Quire to the "Open with" list for `.md` — never a
default handler.
Why: two crates can embed a resource; `winres` wants a resource compiler
already on `PATH`, while `embed-resource` locates `rc.exe` through `vswhom`
and reports `NotAttempted` on a machine without the SDK instead of failing the
build — a clone that cannot package still compiles. The version stays written
once: `build.rs` reads `Cargo.toml`, and the `.iss` reads it back off the built
exe with `GetFileVersion`. Per-user install (`PrivilegesRequired=lowest`,
`{localappdata}\Programs\Quire`) is forced by the app itself: its default
database path is relative to the working directory (`appdata/quire.db`), so a
Program Files install would start with a non-writable one and fall back to
memory without saying so. (ADR-0020 moved the library to `%APPDATA%\Quire`, so
that premise is gone; the per-user install stays, because it is also what keeps
uninstalling from touching the notes.) Keeping the whole tree in the user
profile also means uninstalling leaves the notes alone — Inno removes only
directories it emptied.
Consequences: the window icon and the `--open <path>` dispatch the `.md`
association invoke both live in files this track may not edit (`ui/**`,
`src/app/**`, `src/main.rs`), so they went to Track A in `M8_FEEDBACK.md` with
the exact lines; until they land, the association only launches the app. An
`rc.exe` rejection now fails the build rather than shipping a plain exe.
`make_icon.ps1` is a manual step: change the palette and the `.ico` does not
follow until it runs again.

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
M8 addendum) and the folder holds up to 3 extra copies of the database (5 since
ADR-0019) — both are the price of never opening a blank app after one bad
write. The loss
window is by design: `.bak1` is the database *as of the last successful open*,
so a corruption that arrives mid-session costs the edits made since startup;
closing that would mean rewriting the whole file every flush, which §三十三
rules out in spirit — Track A can call `backup::snapshot` from a "save a copy"
menu item if a real case appears. (ADR-0019 later closed this from the inside:
the family is five generations deep and `PersistenceService` snapshots on the
flush tick, so the window is "since the last snapshot", not "since startup".)
Recovery only answers *structural* damage: an
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
extra threads / build complexity. Current set: slint + slint-build (M1),
rusqlite with the bundled SQLite (M3 — persistence has no std answer), rfd
(M8 dialogs — native file pickers, no UI toolkit dependency), and
embed-resource as a *build* dependency only (M8 installer — it runs rc.exe and
adds nothing to the binary). Anything else waits for a milestone that cannot be
built without it.
