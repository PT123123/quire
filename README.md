# Quire

A local, GPU-accelerated, Notion-like document workspace.
Rust + Slint. Single process. No Electron, no WebView, no web stack.

```
cargo run                     # debug, FemtoVG renderer
cargo run --release           # release
cargo run --no-default-features --features skia --release   # Skia build
```

- Status & roadmap: `PLAN.md`
- Why it looks like this: `docs/ARCHITECTURE.md`, `docs/DECISIONS.md`
- Measured numbers: `docs/PERFORMANCE.md` (`benchmarks/scripts/bench.ps1`)

First release (Windows): block editor, local SQLite storage, command
palette, Chinese input via the OS IME — nothing cloud, nothing sync, yet.
