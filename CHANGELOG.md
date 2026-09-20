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
  "[] ", "[x] ", "> ", "---" or "```" at a block start to convert it as you
  type (one undo step)
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
  Columns) — picking a
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
  into, and the picture lands in the page. The bytes go to an
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
  Ctrl+E / Ctrl+Shift+X work inside a cell). Turning a line into a table
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
- Every popup (page menus, ⋮⋮ menu, slash menu, command palette, search)
  dismisses on a click outside it and on Escape; UI state follows so
  nothing stays blocked behind an already-closed menu
- The "/" block menu now measures its own height when it picks where to sit, so
  a long list stays inside the window instead of running off the bottom edge as
  soon as another kind is added to it
- Fixed the handle (+/⋮⋮) being clickable while invisible on the block
  being edited: it now shows whenever the row or the buttons are hovered,
  editing or not
- Inline marks: bold (Ctrl+B), italic (Ctrl+I), inline code (Ctrl+E),
  strikethrough (Ctrl+Shift+X), links (Ctrl+L + dialog; click a link to
  open it — internal quire:// links navigate in-app)
- Per-page find bar (Ctrl+F) with hit counter and selection navigation
- Undo/redo (Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z) — per page, command-based

### Workspace
- Page tree: create / rename in place / duplicate (nested lists survive) /
  delete with confirmation; favorites; recent pages; last page restored
  on startup
- Page title edits in place; word/char count in the editor footer
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
  settings, metadata (schema v1–v8)
- Debounced batched writes; Ctrl+S forces a save; close saves too
- Rotating snapshots on every open (5 generations), restore-at-open when
  the main file is damaged, damaged file quarantined (`.corrupt`)
- Startup integrity checks; schema migrations (v1–v8)
- Picture and file bytes live in an `attachments` folder beside the database
  and the row is only a reference — a reference whose file row is gone still
  loads the library and renders as a missing picture. A file is copied in
  without being read, so nothing about attaching one scales with its size
- An unclean end (panic, kill, native crash, power loss) is recognized on
  the next start: the notice bar says so and the fact is queryable in the
  metadata table (a panic additionally keeps its report)

### Desktop integration
- Frameless window with custom title bar, light + dark themes (persisted)
- Window size remembered; last-opened page restored
- Settings: appearance, LAN sharing, the database folder (open it in
  Explorer, take a backup on demand)
- Markdown export/import (page level, inline marks round-trip)
- "Copy Page as Markdown" (command palette): the page through the
  exporter onto the clipboard, CJK-safe (Win32 FFI write path)
- An attached file opens in whatever the system has registered for its type
  (one `ShellExecuteW` call — no `windows` crate, no subprocess), and saves
  back out to any path picked in a Save-as dialog
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

### Known limitations
- Switching directly from one open menu to another (e.g. ⋮⋮ on a different
  block while a menu is open) takes two clicks — the first click only
  dismisses the open popup (standard Slint popup semantics)
- A menu taller than the window (e.g. Move-to in a large workspace)
  overflows the bottom — the anchor clamps but the list does not scroll
  yet. The Settings dialog has the same shape at 1280x800: its shortcut
  tail, ABOUT and the Done button fall below the bottom edge, so Esc is
  the way out
- Block colors are cosmetic: they do not survive a Markdown export/import
  round trip, and Callout blocks export as quotes
- Inline-mark paragraphs render runs on one line (no cross-run reflow —
  Slint `Text` has no inline formatting yet)
- Pictures: pasting an image from the clipboard is not wired yet (insert
  from a file is), replacing a stored file on disk in place needs a
  restart to show up, and a page of pictures is not yet measured in the
  benchmark scenes — the decode cache is capped by construction, not by
  a reading
- Attachments are never garbage-collected: deleting the last block that
  points at a stored file leaves the bytes in the `attachments` folder.
  Same for pictures and files, and it is deliberate for now — undo has to
  be able to bring a reference back without touching disk
- A PDF attaches and opens, but shows no first-page thumbnail: it looks
  like any other file apart from its name. Deferred by explicit decision
  2026-09-20; the renderer route for it is still undecided
- Tables are a grid, not a database: no per-column widths, no header
  formatting, no sorting. Inside a cell only Tab / Shift+Tab cross between
  cells — Enter does not split one, Backspace does not merge it with a
  neighbour, and the arrow keys will not step up or down a row. A cell takes
  bold / italic / code / strikethrough but not a link (Ctrl+L is not wired
  there), and converting a marked line into a table keeps its words and drops
  its marks. Hovering the grid adds its toolbar as a row, so content below a
  table shifts down by 22 px while the pointer is on it
- Markdown reads tables as plain text: export writes GitHub-flavoured
  tables, and importing one back gives a paragraph per row. Deliberate and
  pinned by a test — the importer is line-at-a-time and a table needs
  lookahead
- Chinese IME: the manual acceptance pass (docs/IME_CHECKLIST.md) is
  signed off — 2026-09-20
