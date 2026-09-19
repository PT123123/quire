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
  Numbered, Quote, Divider, Callout, Code) — picking a row converts the
  new line, clicking away or Escape keeps the empty line, typing filters
  the menu. Toggle list and the database views (Table, Board, Gallery,
  List, Calendar, Timeline) appear as muted "later" placeholders and
  cannot be picked yet
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
  jumping
- Full-text search (Ctrl+P) — titles and content, Chinese included
  (FTS5 + segmentation)
- Context menus on pages and blocks

### Persistence & reliability
- SQLite (bundled, no server): pages, blocks, marks, colors, settings,
  metadata (schema v1–v4)
- Debounced batched writes; Ctrl+S forces a save; close saves too
- Rotating snapshots on every open (5 generations), restore-at-open when
  the main file is damaged, damaged file quarantined (`.corrupt`)
- Startup integrity checks; schema migrations (v1–v4)
- An unclean end (panic, kill, native crash, power loss) is recognized on
  the next start: the notice bar says so and the fact is queryable in the
  metadata table (a panic additionally keeps its report)

### Desktop integration
- Frameless window with custom title bar, light + dark themes (persisted)
- Window size remembered; last-opened page restored
- Markdown export/import (page level, inline marks round-trip)
- Installer (Inno Setup): per-user, Start menu + desktop shortcuts,
  optional `.md` "Open with" association; `--open <path>` dispatch
- GPU rendering (FemtoVG default; Skia / wgpu builds selectable); idle
  CPU ≈ 0, 10 000-block pages cost single-digit MB

### Known limitations
- Switching directly from one open menu to another (e.g. ⋮⋮ on a different
  block while a menu is open) takes two clicks — the first click only
  dismisses the open popup (standard Slint popup semantics)
- Block colors are cosmetic: they do not survive a Markdown export/import
  round trip, and Callout blocks export as quotes
- Inline-mark paragraphs render runs on one line (no cross-run reflow —
  Slint `Text` has no inline formatting yet)
- Chinese IME behavior documented in docs/IME_CHECKLIST.md — acceptance
  pass pending
