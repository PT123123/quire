# PERFORMANCE — baselines and regressions

All numbers from `Release` builds unless stated. Method below; never trust
a number whose scene and command line aren't recorded here.

## Environment
- Machine: Intel Core Ultra 9 185H (16 cores / 22 threads) · 32 GB RAM ·
  Intel Arc Graphics · Windows 11 build 26200
- Display scale tested at: 100% (further scales tracked as findings)
- Slint: 1.18.0 · Renderer: per-binary feature (ADR-0004)

## Method
`benchmarks/scripts/bench.ps1` starts the release exe with a fixed scene,
waits for the main window (that interval ≈ startup time), then samples the
process for N seconds:
- Working Set / Private Bytes (task-manager semantics via Get-Process)
- CPU% = ΔTotalProcessorTime / (Δwalltime × logical cores)
Idle scenes use `--auto-exit N`; no mouse/keyboard input during sampling.
GPU-side memory is *not* Working Set; when it matters we record it from
Task Manager's "GPU Memory" column / pdh counters and say so.

## Scenes
| id | scene | command |
|----|--------------------------------|-----------------------------------|
| A | empty shell (mock sidebar only) | `quire.exe` |
| B | 100 blocks | `quire.exe --blocks 100` |
| C | 5 000 blocks | `quire.exe --blocks 5000` |
| D | 10 000 blocks | `quire.exe --blocks 10000` |
| E | typing | deferred to M4 (no editor yet) |
| F | continuous scroll | manual pass, recorded per milestone |
| G | switch 100 pages | deferred to M2 |

## Baseline — M0/M1 (Release, idle ≈ 5 s after window appears)

`startup_ms` = process start → main window handle exists (window-up time,
slightly before first paint). `idle_cpu_pct` is % of one logical core.
Measured 2026-09-18 with `benchmarks/scripts/bench.ps1`.

| renderer | build | startup ms | idle CPU % | WS MB | Private MB |
|----------|-------|-----------:|-----------:|------:|-----------:|
| FemtoVG·GL | A | 838 | 0.39 | 108.8 | 84.7 |
| FemtoVG·wgpu | A | 773 | 0.20 | 199.1 | 147.5 |
| Skia | A | 833 | 0.00 | 245.2 | 211.5 |
| FemtoVG·GL | D (10 000 blocks) | 160 | 0.19 | 108.4 | 87.5 |
| FemtoVG·wgpu | D | 125 | 0.00 | 190.2 | 149.0 |
| Skia | D | 120 | 0.00 | 234.9 | 211.9 |

Reading:
- All three GPU paths run natively on this machine (Intel Arc, GL + wgpu +
  Skia/Metal-like GL all fine). Binary sizes: GL 15 MB, wgpu 22 MB, Skia 25 MB.
- 10 000 blocks add ≈ 3 MB Private and no idle CPU → the single-`for`
  ListView virtualizes as intended (ADR-0007).
- FemtoVG·GL has roughly half the memory of wgpu and a third of Skia →
  default renderer stays `femtovg` until M7 text-clarity comparisons say
  otherwise. Second-launch `startup_ms` is lower (OS cache warm).
- Idle CPU ≈ 0 % on all three: no animation loops, no polling. Keep it that way.

## Notes / open questions
- Slint's winit backend redraws on events; any persistent animation on an
  idle screen is a bug — chase it (animation tokens are finite-duration only).
- Rule of thumb: UI Item count, TextEdit count, text layout calls, model
  update range, timer count. New features must state their cost in the PR/
  milestone notes here.
