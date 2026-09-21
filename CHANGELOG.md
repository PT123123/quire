# Changelog

## 0.1.0 — Windows RC (in progress)

First functional release: a local, single-file-database notes workspace.

### Editor
- Block editor: paragraphs, headings 1–3, bullet / numbered / to-do lists,
  quotes, code blocks, dividers
- One focused block owns the live text input; everything else renders
  lightweight (10 000-block pages stay flat in memory and rendering)
- Enter / Backspace merge & split semantics, empty list items leave the
  list on Enter or Backspace
- List nesting: Tab / Shift+Tab (depth 1), drag-free reordering via the
  block menu or Ctrl+Shift+↑/↓, Ctrl+D duplicates a block
- Drag the block handle (⋮⋮) to reorder: an accent line marks the landing
  spot and the drop commits one undo-able move; landings that would split a
  nested list away from its parent are rejected
- Markdown line-shortcuts: type "# ", "## ", "### ", "- ", "* ", "1. ",
  "[] ", "[x] ", "> ", "---", "```" or "$$ " at a block start to convert it as
  you type (one undo step)
- Callout block: a tinted rounded box with an emoji and text (slash menu
  "Callout"; turns into Text/Code/Divider like the other kinds)
- Block menu (⋮⋮) — Notion's set, minus the collab/AI items that stay out
  of v1: Turn into (Text / Callout / Code / Divider), Duplicate, Copy link
  to block (puts a quire://block anchor on the system clipboard), Move to
  (every other page; the whole subtree crosses in one undo step), Text
  color and Background color (live-picking palette with swatches), Move
  up/down, Copy block, Paste below, Delete. Clicking a quire://block or
  quire://page link jumps inside the app. The slash menu and Turn-into
  list only carry kinds without a symbol shortcut (ADR-0022)
- "+" handle opens Notion's insert menu: it creates the empty line below
  and shows the full block list (Text, Page, To-do, Headings, Bulleted /
  Numbered, Quote, Divider, Callout, Code, Toggle list, Image, File, Table,
  Columns, Math, Table of contents, Embed) — picking a
  row converts the new line, clicking away or Escape keeps the empty line,
  typing filters the menu. The database views (Table view, Board, Gallery,
  List, Calendar, Timeline) appear as muted "later" placeholders and
  cannot be picked yet
- Toggle list: a collapsible section. The chevron folds its whole subtree
  out of existence — the hidden blocks get no rows at all, so they cannot
  be tabbed into, dragged or renumbered — and the fold is a view setting,
  not content, so it survives restarts and never bumps the document.
  Turning a Toggle into another kind re-opens it rather than stranding its
  children; Markdown export degrades a toggle to a quote line
- Image block: pick a file from the insert menu, the slash menu or Turn
  into — or press Ctrl+V with a screenshot on the clipboard — and the
  picture lands in the page. The bytes go to an
  `attachments` folder beside the database (the app keeps only a
  reference), oversized pictures get a downscaled display copy while the
  original stays untouched, and the ⋮⋮ menu's "Image width" sets the row
  to 25 / 50 / 100 % of the column. Click a picture to view it full-width
  behind a scrim; clicking anywhere closes it. Undo removes the reference,
  never the file. The picker offers png, jpg, bmp and gif — the four
  formats the decoder is asked for; a gif becomes a still, its first frame
- File block: any file at all, from the same three doors as a picture. The
  row shows the name with its extension, the size, and two buttons — Open
  hands the bytes to whatever the system has registered for that type,
  Save-as copies them out under their original name. The file is streamed
  straight into the `attachments` folder and never read into the app, so a
  2 GB attachment costs the same as a 2 KB one until you press one of those
  buttons. Undo removes the reference, never the file
- Table block: a simple N×M grid, from the insert menu, the slash menu or
  Turn into. Tab moves through the cells and Shift+Tab back; tabbing past
  the last cell adds a row, so the table grows as far as the typing goes.
  Hovering the grid reveals its toolbar — Add row, Add column, Delete row,
  Delete column — where adding goes after the row or column the caret sits
  in and deleting takes it, and a table never shrinks below 1×1. Cells hold
  text with the same inline marks as anywhere else (Ctrl+B / Ctrl+I /
  Ctrl+E / Ctrl+Shift+X / Ctrl+M work inside a cell). Turning a line into a table
  keeps its words in the top-left cell, and turning a table back into text
  gives every cell back as its own paragraph; Markdown export writes a
  GitHub-flavoured table (import still reads those lines as text). This is
  a grid, not a database — schema, filters, sorts and views are a later
  milestone
- Columns block: a line becomes a side-by-side layout, from the insert menu, the
  slash menu or Turn into. Two boxes to start, three at most, and hovering the
  layout reveals its strip — Add column, Delete column — where a box is added at
  the right end with a line inside it and a deleted box's words move into the
  box before. The line's own words open the first box, so nothing is lost by
  converting a line; the layout itself holds no text of its own. Tab walks the
  lines inside the layout and stops at either end (you leave a box by clicking
  out), and an empty box says "Empty column" until you click it, which gives it
  its first line. Layouts are ordinary blocks all the way down — a box is a
  child block and so is every line in it — so undo covers every layout edit and
  no new storage is needed. Markdown export flattens a layout into page-level
  paragraphs, keeping the words and the reading order
- Math block: a formula on its own line, from the insert menu, the slash menu,
  Turn into, or by typing "$$ ". It stores what you type — a LaTeX subset as
  source — and paints the nearest Unicode reading of it: `\frac{a+b}{2}` becomes
  one-line `(a+b)/2`, `\sqrt{ab}` becomes `√(ab)`, `x^2_i` becomes `x²ᵢ` wherever
  the alphabet has the glyph. Nothing is thrown away: a command it does not know
  comes back as its own source, so the worst case reads "that did not render"
  rather than "that vanished". Click the block and the source is what you edit.
  Markdown writes and reads a `$$ … $$` fence, verbatim inside like a code fence
- Contents block: a page's own table of contents, from the insert menu, the slash
  menu or Turn into. The list is not stored — every line is read off the page each
  time the page is projected — so renaming a heading renames its line, deleting one
  removes it, and a heading a folded toggle hides leaves the list until the toggle
  opens again. Lines indent by heading level, an untitled heading reads "Untitled",
  and clicking a line puts the caret at the end of that heading. The block has no
  text of its own; converting a line into one keeps that line's words stored but
  unpainted, the way a divider does, so turning it back gives them back. Markdown
  writes and reads a single `<!-- quire:toc -->` marker line
- Embed block: a link shown as a card, from the insert menu, the slash menu or
  Turn into. The block stores the address and nothing else — the card's headline
  (YouTube, Figma, Google Maps, GitHub, or the host itself for a site the app has
  never heard of) and the address line under it are both read off that text as it
  paints, so there is no second copy of the link to go out of step. The arrow
  hands the address to your browser; nothing is fetched, no page is embedded and
  no favicon is downloaded, which is what keeps a card cheaper than the iframe it
  stands in for. Click the card and the address is what you edit — the headline
  changes as you type it, and an empty card says "Embed / No address yet" rather
  than showing a blank box. Markdown writes the bare address on its own line, so
  the file reads as a link in any other renderer; import turns a line back into a
  card only when it is one address and nothing else, so a sentence that happens to
  contain a url stays a sentence
- Fixed a marked line with room to spare painting its runs apart. A paragraph's
  inline marks render as side-by-side runs, and the row laid them out with
  Slint's default `alignment: stretch`, so any leftover width was divided among
  the runs as gaps — ~155 px between three of them on the first short formula
  line the sweep had ever shot. Every marked line in the then-44-scene baseline
  overflowed its frame instead, so nothing showed it before. The fix is on all
  three run rows — block, table cell, column line — and moved none of them
- A paragraph carrying inline marks now wraps like a plain one. This was the
  documented platform wall: Slint `Text` has no inline formatting, so marks paint
  as separate runs, a run was one whole stretch of text, and a layout cell cannot
  break — a marked paragraph ran off the edge and was clipped mid-word while the
  identical unmarked paragraph wrapped. `build_runs` now cuts every *unmarked*
  stretch to one word per run and the row lays the runs out with a wrapping
  `FlexboxLayout`, so the breaks land where the words do; marked stretches stay
  one cell, because an underline, a code box or a link's click target split at
  every space is worse than the bold phrase it still cannot break (ADR-0041).
  What it costs, measured rather than assumed: ≈2.8 ms per projection of a
  10 000-row page whose every tenth line is marked (and nothing on an unmarked
  page), 1.031× the control's memory on that same page, and ≈3 px of extra line
  spread on a marked line whose words were already fitting
- Fixed the page title sitting below where it is bound. The row's title band
  sets its height but used to leave `y` alone, and Slint centres such a child
  vertically in its parent — harmless while every first block was one line
  tall, visible as soon as the first block was a table, whose grid then
  painted over the title. The offset was ~34 px on a paragraph page and
  ~60 px on a table page; the band now states its `y`
- Page block: embeds a child page (insert menu "Page"). The row shows a
  page icon and the child's live title (renames propagate), clicking it
  opens the child, deleting the block deletes the child page, and
  duplicating copies the child so the two blocks never share a target.
  Exports as a `quire://page` link that re-imports clickable
- Link-to-page block (insert menu "Link to page"): points at any existing
  page through a filterable picker; the target is not owned — deleting
  the block, duplicating, or pasting it never touches the page
- Rich paste: pasting markdown with block structure (headings, lists,
  to-dos, quotes, code) splits it into real blocks with inline marks;
  plain text still pastes natively at the caret
- Ctrl+V pastes a screenshot: when the clipboard carries no text but holds a
  bitmap (`CF_DIBV5`/`CF_DIB` — what Snip-and-Sketch and PrintScreen write),
  it becomes an Image block stored like any other attachment. An empty block
  *becomes* the picture; a block with words in it gets the picture below, so a
  paste never leaves a stray empty line. One undo step. When a copy carries
  both text and a picture, the words win
- Every popup (page menus, ⋮⋮ menu, slash menu, command palette, search)
  dismisses on a click outside it and on Escape; UI state follows so
  nothing stays blocked behind an already-closed menu
- Modals — the delete confirm, Settings, the Add-link card — now dim the window
  behind them. Anchored popups deliberately still do not: a menu that asks you
  to pick a type for one line should not hide that line
- The "/" block menu now measures its own height when it picks where to sit, so
  a long list stays inside the window instead of running off the bottom edge as
  soon as another kind is added to it
- Fixed the handle (+/⋮⋮) being clickable while invisible on the block
  being edited: it now shows whenever the row or the buttons are hovered,
  editing or not
- Inline marks: bold (Ctrl+B), italic (Ctrl+I), inline code (Ctrl+E),
  strikethrough (Ctrl+Shift+X), links (Ctrl+L + dialog; click a link to
  open it — internal quire:// links navigate in-app), inline math (Ctrl+M over
  a selection: the same LaTeX subset as a math block, rendered inside the
  sentence; Markdown writes it as `$…$`, and a sentence that merely mentions
  prices — "costs $5 and $10" — stays prose)
- Toggle the sidebar with Ctrl+\ — it had been Ctrl+B, which is bold, so the
  same chord was labelled two different things in two different places
- A brand-new page can be written in: click the "This page is empty" panel, or
  press Enter in the title, and the page makes its own first paragraph, focused
  and one undo step away from gone (ADR-0033)
- Code blocks can be coloured: the block's own ⋮ → Language menu picks the
  language (Rust / Python / JavaScript / TypeScript / Markdown / JSON / Bash) and
  the block paints five token colours over its own text — keyword, comment,
  string, number, name. Only the language is stored; the colours are derived while
  drawing, so a keystroke never lexes and a block being edited shows plain text
  (ADR-0042). A Markdown fence's info string carries it both ways (`rs` in,
  `rust` out), a rich paste keeps it, and a language this build cannot lex is no
  colour rather than a broken block
- Per-page find bar (Ctrl+F) with hit counter and selection navigation. Every
  match is now painted, not just counted: a hit is one word-run cell with a pale
  fill and an amber border, so the box says where the text is without taking the
  block's own colour below what the palette already allows. A hit that lands in
  the middle of a word splits the word, one that lands inside bold or a formula
  tints the whole mark, and one inside a table cell or a column box rides on the
  row that draws it. Quote and callout hold their text in a `Text` of their own
  and now share that flexbox, so the counter and the page reconcile: on the swept
  page 16 hits are 14 boxes plus the 2 in the block the bar stepped into, which
  shows the editor's selection instead (ADR-0043)

- Undo/redo (Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z) — per page, command-based

### Workspace
- Page tree: create / rename in place / duplicate (nested lists survive) /
  delete with confirmation; favorites; recent pages; last page restored
  on startup
- Page title edits in place; word/char count in the editor footer
- Page look (top bar ⋯ → Style): a page picks its own typeface — Default, Serif
  or Monospace — and switches Full width and Small text. All three are stored on
  the page (schema v10: `pages.font`, `pages.layout`), never on a block: one
  derived token layer applies them to the document tier, so the sidebar, menus,
  palette and settings keep their own type. Small text shrinks body and headings
  by the same factor; full width drops the centred column for a left gutter.
  Not undoable, like Favorite — a look is a property, not an edit — and a
  duplicated page starts with its source's (ADR-0044)
- Page icon (top bar ⋯ → Set icon): a 96-emoji grid, twelve rows of eight, plus a
  None row to clear it. The page stores the emoji itself (schema v11:
  `pages.icon`), not the grid's index, so the catalogue can grow without
  rewriting anybody's page. An unset page shows its title's first character in the
  sidebar tree — the shortcuts keep their star and clock until a page really has
  an icon, and the page title shows nothing above itself rather than its own
  first letter at 46px. Like Style, setting one is not an undo step, and a
  duplicated page starts with its source's (ADR-0045). An icon is an emoji only —
  a picture belongs on the cover, not in a 16 px slot (ADR-0046)
- Page cover (top bar ⋯ → Set cover): a local picture behind the page title, with
  Change cover and Remove cover beside it. The page stores an attachment reference
  (schema v12: `pages.cover`, nullable), never a path, so the STORAGE reclaim can
  tell that the page still draws those bytes. A fixed dark veil sits between the
  picture and the title, which is what makes the title's contrast a bound rather
  than an opinion: white ink measures **6.19:1** over the worst picture a user can
  pick — a pure-white one — with room to spare over the 4.5:1 floor (which needs
  only α ≥ 0.535, and this veil is α 0.6196).
  `benchmarks/scripts/contrast_probe.ps1` reads that number off the
  rendered screenshot, self-checks its arithmetic, and ships with a scene built to
  fail so the gate is shown able to say no. The page's own emoji takes the same
  rule (it measured 1.04:1 before this was caught on pixels). Like Style and the
  icon, setting a cover is not an undo step, and a duplicated page starts with its
  source's (ADR-0047)
- Page lock (top bar ⋯ → Lock page): a read-only switch on the page itself
  (schema v13: `pages.locked`, `NOT NULL DEFAULT 0`, so "open" is a value and an
  old library upgrades with nothing locked). It closes the document — every
  block's content and the page's own title — while leaving the page's *look*
  (icon, cover, Style, favourite), the tree (move, delete, duplicate) and every
  read (navigation, search, folding) alone. Two gates, because one is not enough:
  the command funnel refuses the write, and the row's `editing` binding refuses
  the caret, so a stale edit cannot survive a lock/unlock/relock cycle. Nothing
  is swallowed silently — the notice bar says which switch to flip, the page says
  it in a pill above its title, the ⋯ row reads "Unlock page", and the ⋮⋮ menu
  keeps only its two read-only Copy rows. Undo is refused while a page is locked
  and its stack is kept, not dropped; a duplicated page starts **un**locked,
  which is the one place a lock departs from a look (ADR-0048)
- Slash menu ("/") for block types; command palette (Ctrl+K) with page
  jumping, plus Go Back / Go Forward (Alt+← / Alt+→) along the pages
  visited this session — a page deleted since drops out of the history
  instead of being opened
- Full-text search (Ctrl+P) — titles and content, Chinese included
  (FTS5 + segmentation)
- Context menus on pages and blocks; pages move: Move up / Move down
  reorders siblings, Move to reparents anywhere outside the moved
  subtree (the whole hierarchy travels, recorded and restored across
  restarts)

### Persistence & reliability
- SQLite (bundled, no server): pages, blocks, marks, colors, attachments,
  settings, metadata (schema v1–v13)
- Debounced batched writes; Ctrl+S forces a save; close saves too
- Rotating snapshots on every open (5 generations), restore-at-open when
  the main file is damaged, damaged file quarantined (`.corrupt`)
- Startup integrity checks; schema migrations (v1–v13). The steps that only add a
  column share one helper and each guard on its own column's absence, so a
  half-migrated file — one somebody edited by hand, or restored to the middle of a
  sequence — converges instead of erroring on a duplicate column name
- Picture and file bytes live in an `attachments` folder beside the database
  and the row is only a reference — a reference whose file row is gone still
  loads the library and renders as a missing picture. A file is copied in
  without being read, so nothing about attaching one scales with its size
- Settings → STORAGE can **reclaim** the attachments nothing points at any
  more: the rows go first (`Change::AttachmentDeleted`, which no command plan
  emits), then the files beside them, and the notice bar reports how many and
  how many bytes. "Nothing points at" counts every page's blocks, every step
  still on any page's undo *or* redo stack, and the copied block — so the
  100-step undo cap is how long a picture is protected, and the sweep reads
  only the rows this session loaded, never the folder. It runs on the UI
  thread and costs about half a second per thousand attachments
- An unclean end (panic, kill, native crash, power loss) is recognized on
  the next start: the notice bar says so and the fact is queryable in the
  metadata table (a panic additionally keeps its report)

### Desktop integration
- Frameless window with custom title bar, light + dark themes (persisted)
- The light theme's quietest text is measured rather than eyeballed: the third
  text tier (block handles, footer, sidebar, every hint row) went from 2.5–2.7:1
  to 4.1–4.4:1, and the two weakest block colours from 2.81 and 2.48 on their own
  tint to 3.61 and 3.56 — the band the dark theme has always sat in (ADR-0023)
- Window size remembered; last-opened page restored
- Settings: appearance, LAN sharing, the database folder (open it in
  Explorer, take a backup on demand, reclaim unused attachments)
- Markdown export/import (page level, inline marks round-trip)
- "Copy Page as Markdown" (command palette): the page through the
  exporter onto the clipboard, CJK-safe (Win32 FFI write path)
- An attached file opens in whatever the system has registered for its type
  (one `ShellExecuteW` call — no `windows` crate, no subprocess), and saves
  back out to any path picked in a Save-as dialog
- A link — inline, in a link block or on an embed card — leaves the app only
  when it is an address a browser understands: http, https or mailto. A local
  path, a network share, `file://` or a protocol the shell happens to have
  registered now does nothing, because a link's target is data that arrives
  from a file and the shell will *run* a path as readily as it opens a url
- Installer (Inno Setup): per-user, Start menu + desktop shortcuts,
  optional `.md` "Open with" association; `--open <path>` dispatch
- GPU rendering (FemtoVG default; Skia / wgpu builds selectable); idle
  CPU ≈ 0, a 10 000-block page costs ≈10 MB over the empty shell

### Build & test
- `just check`: `cargo check --all-targets`, the whole test suite, a release
  build. Visual regression and the RAM/CPU scenes run from
  `benchmarks/scripts/` (`sweep.ps1` compares against a manifest of hashes)
- A test that needs a folder — a database, a log family, an attachments
  directory — gets one from `quire::testing::ScratchDir`, which deletes it when
  the test ends. Each helper used to create a uniquely named `%TEMP%` directory
  and walk away from it: 3 639 of them had accumulated, and the naming that
  existed to stop a test reading a stale database was only load-bearing
  because nothing ever cleaned up
- The bench scripts delete the scratch database, its `.bak<N>` snapshots and
  their own report file when a run ends; the purge used to happen before the
  run and covered only the `.db`, so every label left its snapshot behind
- `quire-shot --hover x,y` parks the pointer without pressing a button, which
  is the only headless way to light UI that exists on hover alone (a table's
  edge toolbar, a row's ⋮ handle)
- `benchmarks/scripts/diffbbox.ps1 -OldDir A -NewDir B` prints, per scene, the
  bounding box of the pixels that moved between two sweeps. A re-sweep is
  judged from that table rather than from 42 pictures: when every box lands
  inside the band the change explains, one verdict covers the set
- The command palette's ids now resolve through `palette_action()` in Rust, and
  its dispatch is a `match` with no wildcard arm, so a new command that skips a
  variant is a compile error. A test walks the real command registry and asserts
  every row resolves to its own distinct action — the class of bug that once
  shipped with every palette row from id 9 up running one action, green tests
  and all (ADR-0034)
- `--pictures N` makes a bench scene of media: every `rows/N`-th row of the
  bench page becomes an image block, backed by a generated 1280×720 PNG from a
  pool of 200 files written when the library is first built. The fixtures are
  not re-encoded during the measured passes — a run that timed PNG encoding
  would not be measuring pictures — and the app prints its own decode cache
  figures (`--dump-state`, one JSON line on stderr) because a process memory
  counter cannot see a cache whose unit is one raster
- `--marks N` does the same for the other fixture scene D never had: it bolds the
  second word of every `rows/N`-th row, so a 10 000-block page can carry 1 000
  marked paragraphs and the RAM gate can see the runs channel at all. Without it
  the gate was blind — scene D has no marks, so a change to marked rendering
  measured 1.00× of nothing. `--dump-state` now also reports what it built
  (`gs+atlas-blocks=10024 marked=1000`), which lets each arm of an A/B prove its
  own fixture instead of taking the bench script's label on faith
- Scene F — continuous scroll — had never scrolled. Slint measures a list's
  content offset *negative* going down, and the harness timer was adding, so
  every frame wrote a value the clamp rounded straight back to zero: the scroll
  numbers recorded since M2 describe a repaint loop at the top of the page, not
  a scroll. `quire-shot --scroll-y` is now the control that catches this class
  of thing — two offsets that are real must produce two different PNGs — and
  `--scroll-step` sets how far a frame moves, so a flick and a wheel tick are
  separate measurements (ADR-0036)
- `benchmarks/scripts/scroll_ab.ps1` runs the scroll arms against two binaries in
  one sitting — the default build and a `--no-default-features --features
  skia-opengl` one in `target-skia/` — and fails the batch unless each binary's
  own first-paint line reports the renderer it is supposed to be. That
  self-identification is the whole point: the skia arm of the renderer verdict
  had been standing on the same broken scroll as the femtovg one, and a skia
  build that silently keeps the femtovg default would otherwise produce an A/B
  of a binary against itself (ADR-0036)
- The matrix's `-Only` filter now splits a comma list and throws when no scene
  matched. `powershell -File` passes `-Only A,B` to a `[string[]]` parameter as
  one string, so a multi-token filter selected nothing, printed nothing and exited
  0 — a run that measured nothing and said it finished. Two more shapes joined the
  matrix so every row the media batch published is reproducible from the script:
  the flick-sized scroll on a text page, and a short page of pictures
- A RAM gate now comes with its control. The previous four batches each reported
  a same-direction rise on both arms and recorded it as "session drift" without
  ever measuring what drift is, so the math slice built the commit before it
  (`2e9de99`) in a temporary worktree against the same target dir and ran the two
  exes alternately in one sitting: 1.016× on private bytes, with each binary
  first proving which build it is (md5, and the control tree has no math source
  file). The worktree goes once the number is in; the next kind that claims a
  per-row cost repeats the run against its own predecessor commit
- The contents block repeated that run against the commit before it (`cc7ccf0`) and
  came in at 1.007× on private bytes. It also bought the first whole-page projection
  number: the gate's scene has no contents block in it, so a green gate alone says
  nothing about a row that walks the page every time it is projected. Measured
  separately — 10 000 rows project in ≈38 ms with or without a 1 000-line list —
  and the delta is inside the noise of its own control
- The embed card is the third slice to run that comparison, and it lands in the
  same place: **1.005×** on private bytes, a 0.55 MB gap between the arms against
  the control arm's own 1.2 MB spread (rows
  `benchmarks/results/2026-09-21-m11-embed-ram.jsonl`). Three kinds in a row
  inside one megabyte says the gate has a *floor*, not that all three slices are
  free: a change whose whole per-row cost is a string function is below what this
  instrument resolves, and `docs/PERFORMANCE.md` now says that instead of
  publishing a ratio that only looks like a measurement

### Known limitations
- Switching directly from one open menu to another (e.g. ⋮⋮ on a different
  block while a menu is open) takes two clicks — the first click only
  dismisses the open popup (standard Slint popup semantics)
- A menu taller than the window (e.g. Move-to in a large workspace)
  overflows the bottom — the anchor clamps but the list does not scroll
  yet. The Settings dialog no longer has that shape: it fits 1280x800 with
  its shortcut list, ABOUT and Done on screen, and the sidebar chord is now
  one of the rows it lists
- Block colors are cosmetic: they do not survive a Markdown export/import
  round trip, and Callout blocks export as quotes
- Inline-mark paragraphs wrap between words now, and still clip in two shapes:
  a marked phrase longer than the line (a mark is one unbreakable cell), and a
  marked line that needs more lines than the same words unmarked — bold and mono
  are wider, and the row's height is measured from the plain text
  (Slint `Text` has no inline formatting yet)
- Pictures: replacing a stored file on disk in place needs a
  restart to show up. A page of pictures is now measured while it scrolls — the
  decode cache holds nine 1280×720 rasters and stops there by weight, and an
  on-screen picture costs the process roughly three times the raster — but the
  fixtures are generated gradients, so the disk footprint and any decode-time
  arm are optimistic against a real camera original. A clipboard picture other
  than a bitmap (a `file://` HTML image, an SVG) is not read — only
  `CF_DIB`/`CF_DIBV5`
- Attachments are reclaimed by hand, never on their own: Settings → STORAGE
  → Reclaim deletes the stored files no block, undo step or the copied block
  points at, and nothing runs it for you — a picture is protected for the
  100 undo steps of its page, and after that it waits for the click. And
  because the sweep reads only the rows this session loaded from the
  database, a file in the `attachments` folder whose row is already gone
  stays on disk: it is invisible to the count, and deleting it would mean
  listing the folder, which a session that failed to load its attachments
  would get terribly wrong
- A PDF attaches and opens, but shows no first-page thumbnail: it looks
  like any other file apart from its name. Deferred by explicit decision
  2026-09-20; the renderer route for it is still undecided
- Tables are a grid, not a database: no per-column widths, no header
  formatting, no sorting. Inside a cell only Tab / Shift+Tab cross between
  cells — Enter does not split one, Backspace does not merge it with a
  neighbour, and the arrow keys will not step up or down a row. A cell takes
  bold / italic / code / strikethrough / formula but not a link (Ctrl+L is
  not wired there), and converting a marked line into a table keeps its words
  and drops its marks. Hovering the grid adds its toolbar as a row, so
  content below a
  table shifts down by 22 px while the pointer is on it
- Math is a Unicode reading, not typesetting: `\frac{a+b}{2}` paints on one
  line as `(a+b)/2`, so there are no stacked fractions, no alignment, no
  equation numbers, and a superscript falls back to its plain characters
  wherever the alphabet has no glyph for it. Unknown commands come back as
  their own source. A Math mark and the other marks do not stack — a formula
  inside bold text keeps the formula and drops the bold on export
- A contents block lists the headings of the page it sits on, and nothing more: no
  collection across pages, no "show levels 1–2" setting, no compact or inline
  option, and a long heading is elided to one line rather than wrapped. Clicking a
  line moves the caret, not the scrollbar — a heading below the fold still needs a
  wheel turn first, which is the same limit a `quire://block/` anchor has
- An embed card says who a link belongs to, not what it is: no title, no preview,
  no favicon and nothing fetched, because the app has no WebView and no network
  client by design. A site outside the 21 names it knows is labelled with its own
  host, and a Google url is read as its product from the subdomain or the first
  path segment — anything else behind google.com says "Google". And since this
  slice, a link whose target is not http, https or mailto opens nothing at all:
  `file://`, a local path and a network share are the shapes a document can carry
  that a shell would run rather than open
- Markdown reads tables as plain text: export writes GitHub-flavoured
  tables, and importing one back gives a paragraph per row. Deliberate and
  pinned by a test — the importer is line-at-a-time and a table needs
  lookahead
- Chinese IME: the manual acceptance pass (docs/IME_CHECKLIST.md) is
  signed off — 2026-09-20
