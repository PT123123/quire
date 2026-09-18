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
|----|--------------------------------|--------------------------------------------------|
| A | empty shell | `quire.exe` |
| B | 100 blocks | `quire.exe --blocks 100` |
| C | 5 000 blocks | `quire.exe --blocks 5000` |
| D | 10 000 blocks | `quire.exe --blocks 10000` |
| E | typing | deferred to M4 (no editor yet) |
| F | continuous scroll | `bench.ps1 -Scroll` (programmatic proxy, below) |
| G | switch 100 pages | `bench.ps1 -PageSwitch 100` |

## Baseline — M2 (Release, idle ≈ 5 s after window appears)

Re-measured 2026-09-19 after the M2 navigation work (live sidebar model,
per-page content, headless ADR-0009 stack change). Same machine.

| renderer | build | startup ms | idle CPU % | WS MB | Private MB |
|----------|-------|-----------:|-----------:|------:|-----------:|
| FemtoVG·GL | A | 414 | 0.58 | 111.5 | 86.6 |
| FemtoVG·GL | D (10 000 blocks) | 402 | 0.58 | 112.4 | 90.6 |
| FemtoVG·GL | F (continuous scroll) | 1006 | 19.49 | 134.8 | 108.3 |
| FemtoVG·GL | G (100-page switching) | 393 | 0.39 | 111.5 | 86.7 |

Reading:
- Idle CPU stays ≈ 0 %: the M2 sidebar/search/menu additions did not
  introduce idle work. 10 000 blocks still cost only ≈ 4 MB Private over
  the empty shell — ListView virtualization intact.
- Scene F is a *programmatic proxy*: a 16 ms timer advances the editor
  viewport-y through its two-way binding, i.e. ~60 full-window repaints/s
  driving the same path as wheel input (real input injection stays a
  manual pass). 19.5 % of one core for 60 fps is acceptable; revisit if
  M7 comparisons regress.
- Scene G opens one of 100 mock pages every 120 ms (model swap + sidebar
  rebuild + editor delegate churn): 0.39 % of one core, no memory drift
  over the sampling window.
- Second-launch startup ≈ 400 ms (warm OS cache); numbers above are
  first-launch-after-build runs.

## M3 · save latency (Release, SQLite WAL + synchronous=FULL)
Measured 2026-09-19 on branch `m3-storage` via
`cargo test --test storage -- --ignored --nocapture save_latency`
(temp-dir file DB, not the UI pipeline — wiring adds only the 300 ms
debounce window). A debounced burst = one `apply` of 32 changes (30 text
sets + block insert + setting): **median 2.44 ms, max 2.73 ms** per batch
(debug build ≈ same, the commit fsync dominates); startup `load` of
10 006 blocks + 1 004 pages **3.6 ms**; bulk checkpoint
(`replace_all`, ≈21 000 rows) 119 ms.

## Notes / open questions
- Slint's winit backend redraws on events; any persistent animation on an
  idle screen is a bug — chase it (animation tokens are finite-duration only).
- Rule of thumb: UI Item count, TextEdit count, text layout calls, model
  update range, timer count. New features must state their cost in the PR/
  milestone notes here.
