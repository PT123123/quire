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
- Block menu (⋮⋮): Turn into submenu (Text / Code / Divider); the slash
  menu and Turn-into list only kinds without a symbol shortcut; fixed the
  menu popup never showing from a real handle click (it only ever rendered
  in the visual-test scene) and anchored it beside the handle
- Inline marks: bold (Ctrl+B), italic (Ctrl+I), inline code (Ctrl+E),
  strikethrough (Ctrl+Shift+X), links (Ctrl+L + dialog; click a link to
  open it)
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
- SQLite (bundled, no server): pages, blocks, marks, settings, metadata
- Debounced batched writes; Ctrl+S forces a save; close saves too
- Rotating snapshots on every open (5 generations), restore-at-open when
  the main file is damaged, damaged file quarantined (`.corrupt`)
- Startup integrity checks; schema migrations (v1–v3)

### Desktop integration
- Frameless window with custom title bar, light + dark themes (persisted)
- Window size remembered; last-opened page restored
- Markdown export/import (page level, inline marks round-trip)
- Installer (Inno Setup): per-user, Start menu + desktop shortcuts,
  optional `.md` "Open with" association; `--open <path>` dispatch
- GPU rendering (FemtoVG default; Skia / wgpu builds selectable); idle
  CPU ≈ 0, 10 000-block pages cost single-digit MB

### Known limitations
- Inline-mark paragraphs render runs on one line (no cross-run reflow —
  Slint `Text` has no inline formatting yet)
- Mouse-drag block reordering is not available (menu and keyboard are)
- Chinese IME behavior documented in docs/IME_CHECKLIST.md — acceptance
  pass pending
