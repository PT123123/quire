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

## Next: M2 · App shell navigation on mock pages
- Search panel, favorites/recent wiring, page tree collapse/expand round-trip,
  right-click menu on pages, window resize/snap polish decisions.

## Later (unchanged from brief)
M3 SQLite persistence → M4 block editor MVP (IME = test item, ADR-0002) →
M5 slash menu + command palette real wiring → M6 rich text →
M7 virtualization/performance → M8 Windows RC (packaging, crash recovery,
import/export) → M9 Android (separate IME/editor test plan).

## Explicitly out of scope for v1
Sync, collaboration, cloud, plugin market, AI, multi-process IPC,
custom TSF/IME implementation, image/table/toggle/database blocks.
