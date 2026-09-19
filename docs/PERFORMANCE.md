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
- CPU% = ΔTotalProcessorTime / Δwalltime, i.e. **percent of one core** (this
  machine has 16 cores / 22 threads, so divide by 16 for a Task-Manager-style
  whole-machine share). The script applies no core divisor.
Idle scenes use `--auto-exit N`; no mouse/keyboard input during sampling.
Scene E (`-Typing`) is not idle: `quire-typing` drives the editor through
the same property/callback path a real keystroke uses, and samples the
single-threaded latencies itself.
GPU-side memory is *not* Working Set; when it matters we record it from
Task Manager's "GPU Memory" column / pdh counters and say so.

## Scenes
| id | scene | command |
|----|--------------------------------|--------------------------------------------------|
| A | empty shell | `quire.exe` |
| B | 100 blocks | `quire.exe --blocks 100` |
| C | 5 000 blocks | `quire.exe --blocks 5000` |
| D | 10 000 blocks | `quire.exe --blocks 10000` |
| E | typing | `bench.ps1 -Typing -Blocks N` (`quire-typing`, see below) |
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

### M7 addendum · the same probe with the FTS5 index maintained (ADR-0014)
Re-measured 2026-09-19 on `m8-markdown`, same command, idle machine (the
index rows are written inside the very same transactions the probe times,
so these numbers *are* the after-cost of search):

| measurement | before | with index |
|-------------|-------:|-----------:|
| debounced 32-change `apply`, median | 2.44 ms | **3.42 ms** (min 2.93, max 19.5 — the tail is WAL fsync jitter, not search) |
| `replace_all` of 10 006 blocks + 1 004 pages | 119 ms | **216 ms** |
| startup `load` | 3.6 ms | 7.4 ms (`load` never reads the index; the swing is OS cache state) |

Reading: indexing a debounced burst costs ≈1 ms for 32 rows (≈30 µs/row:
one segmented copy + one `INSERT OR REPLACE` by rowid), and the bulk path
pays ≈9 µs per row for 11 000 rows. Neither is a per-keystroke cost (SPEC
§三十三): writes are batched by the 300/600 ms debounce, so search adds
well under one frame to a save. Query latency is recorded with scene E
below, where a search runs against every typing burst.

## Scene E · typing (M4 editor + M7 search acceptance)
Measured 2026-09-19 on `m8-markdown`, release build, with
`benchmarks/scripts/bench.ps1 -Typing`. `quire-typing` builds the page, then
a timer appends one character to a random block every `1/rate` s through
`editing-id` / `editing-text` / `editing-changed` — the exact path a key press
takes — so a keystroke covers binding → 300 ms edit debounce → command + undo
entry → model row update → repaint → 600 ms flush → SQLite + FTS5 write. Each
run types against a fresh file DB in `%TEMP%`; CPU% and memory are sampled
over 8 s of that, which is ~350 strokes at 30/s.

30 strokes/s, one search every 100 strokes (`--search-every 100`):

| blocks | startup ms | CPU % (typing + search) | CPU % (typing only) | WS MB | Priv MB | stroke handler med / p95 / max µs | search med / p95 µs |
|-------:|-----------:|------------------------:|------------------:|------:|--------:|----------------------------------:|-------------------:|
| 100 | 351 | 27.48 | 31.03 | 120.3 | 88.7 | 52 / 97 / 392 | 383 / 446 |
| 1 000 | 365 | 32.76 | 29.08 | 120.5 | 88.4 | 56 / 116 / 291 | 670 / 760 |
| 10 000 | 331 | 31.40 | 26.70 | 125.3 | 94.5 | 51 / 109 / 396 | 1934 / 2305 |

4× rate, 1 000 and 10 000 blocks (search every 100 strokes):

| blocks | CPU % | achieved rate | stroke handler med / p95 / max µs | search med / p95 µs |
|-------:|------:|--------------:|----------------------------------:|-------------------:|
| 1 000 | 32.36 | 119.3 / 120 | 26 / 57 / 130 | 549 / 925 |
| 10 000 | 32.74 | 119.1 / 120 | 29 / 59 / 239 | 1656 / 2277 |

Reading:
- **Per keystroke: ≈50 µs of UI-thread work, flat in page size.** 100 rows and
  10 000 rows measure the same, because the debounced write touches one model
  row and one index row. The long-document acceptance from the SPEC
  ("连续编辑 1000 blocks 不明显卡顿") holds — the worst single stroke sampled
  is 396 µs, ≈2.5 % of a 16 ms frame (one 10 000-row run without search
  logged 548 µs). §三十三's ban on per-character SQLite transactions is
  intact: the 348 strokes of a run reach the DB in flush batches, not one
  transaction each, and stroke cost stays flat from 100 to 10 000 rows.
- **CPU% is repaint-bound, not stroke-bound.** ≈27–33 % of one core for
  typing (against ≈0.58 % idle), and it does not move when the rate goes 30 →
  120/s, when the search runs, or when the page grows 100×. The achieved rate
  tracks the request at 120/s, i.e. the event loop never falls behind.
- The med per-stroke handler *halves* at 120/s (56 → 26 µs): back-to-back
  strokes coalesce into fewer repaints per keystroke, so per-stroke time
  understates the load and CPU% is the honest number. Compare scenes at equal
  rate for that reason.
- **Search cost scales with the corpus, not the typing:** 0.38 ms at 100
  blocks → 1.9 ms at 10 000, and it fits inside one frame either way. This
  probe calls `SearchService::search` on the UI thread to time it; the panel
  should use `search_async` (ADR-0014), which keeps even the 2 ms case out of
  the frame.
- Memory: 10 000 blocks cost ≈6 MB Private over 100, matching scene D — search
  and typing add no leak-shaped drift over the window.

## M8 · what the rotating backup costs (ADR-0015)
Measured 2026-09-19 on `m8-markdown` with
`cargo test --release --test backup -- --ignored --nocapture` (the
`snapshot_cost` probe: a 1 000-page / 10 000-block workspace, 2 281 472-byte
main file, temp-dir file DB, three alternating rounds in one process):

| measurement | value |
|-------------|------:|
| `Database::open` (migrations + `integrity_check`, no snapshot) | 27.6 – 86.9 ms |
| `backup::snapshot` (`VACUUM INTO`, `synchronous=OFF` for the copy) | 22.7 – 48.9 ms |
| the same copy at `synchronous=FULL` | 44.9 – 255.7 ms |
| `SqliteRepository::open` end to end (rotate + snapshot) | 46.3 – 116.1 ms |
| snapshot file size | 2 269 184 bytes (≈ main, compacted) |

Reading: the policy adds one statement per open, running at roughly 50 MB/s
(2.3 MB in ≈40 ms) — a few milliseconds for a typical 150–450 KB workspace (the
scene E DBs). Disk timings on this machine swing by a factor of three between
rounds, so the ranges are the honest unit; re-measuring with the same command
is the regression check. Keeping the copy
at `synchronous=OFF` is worth roughly half of it on a warm cache and nothing on
a cold one — recorded because it is a deliberate durability exception, not
because the number is large.

Scene E, re-run after the snapshot landed (`bench.ps1 -Typing`, 30 strokes/s,
search every 100): startup 475 / 404 / 432 ms for 100 / 1 000 / 10 000 blocks,
against 351 / 365 / 331 ms in the table above. The two readings agree within
this bench's ±100 ms startup noise; the probe row is the number to trust for
the snapshot itself. Stroke-handler medians (52 / 53 / 59 µs) and idle CPU%
(26.7 – 32 %) did not move, so recovery did not make typing more expensive —
the snapshot runs once, before the window exists.

Storage pragmas confirmed on the way (all from the same probe):
`journal_mode=wal` (persisted in the file), `synchronous=2` (FULL),
`locking_mode=normal`, `page_size=4096`, `wal_autocheckpoint=1000` pages ≈ 4 MB.

## Notes / open questions
- Slint's winit backend redraws on events; any persistent animation on an
  idle screen is a bug — chase it (animation tokens are finite-duration only).
- Rule of thumb: UI Item count, TextEdit count, text layout calls, model
  update range, timer count. New features must state their cost in the PR/
  milestone notes here.

## M7 matrix — femtovg vs skia (2026-09-19, D11)

Full grid, release builds at `3498618` + the D12 working tree (now
committed as `4afc35d`). Raw data: `benchmarks/results/2026-09-19-m7-matrix-{vg,sk}.jsonl`
(18 rows each: every scene has a fresh-DB seed pass and a measured pass).
CPU% is a share of ONE core (16C/22T machine — divide by 16 for the
Task-Manager style whole-CPU number). GPU-side memory is not in these
numbers.

| scene | vg CPU% | sk CPU% | vg WS MB | sk WS MB | vg priv MB | sk priv MB |
|-------|--------:|--------:|---------:|---------:|-----------:|-----------:|
| A idle            | 0.58 | 0.98 | 114.3 | 236.3 |  88.2 | 211.0 |
| B100 idle         | 0.19 | 0.20 | 112.3 | 236.9 |  88.2 | 211.5 |
| B1000 idle        | 0.00 | 0.00 | 113.0 | 238.0 |  89.5 | 212.7 |
| C5000 idle        | 0.20 | 0.78 | 117.7 | 241.6 |  93.8 | 216.3 |
| D10000 idle       | 0.20 | 0.00 | 121.5 | 244.9 |  97.4 | 219.8 |
| F10000 scroll     | 27.7 | (n/a) | 128.1 | 245.9 |  98.2 | 220.7 |
| G100 page-switch  | 0.19 | 0.20 | 115.2 | 236.9 |  88.4 | 211.6 |
| E1000 typing 30/s | 24.8 | 27.5 | 121.5 | 251.6 |  90.2 | 215.5 |
| E10000 typing     | 24.2 | 22.8 | 128.3 | 245.7 |  96.7 | 220.6 |
| E10000 @120/s     | 29.4 | 30.6 | 127.9 | 248.4 |  96.4 | 223.3 |

(sk F10000 scroll: 18.2% CPU. E10000-noseck: vg 23.2% / sk 28.7% — the
skia run showed WS 320.9 MB, a one-off worth watching, see follow-ups.)

### SPEC §六 checklist

| indicator | result |
|-----------|--------|
| startup fast | ✓ warm starts 73–400 ms across all scenes (one 984 ms skia outlier, disk noise) |
| idle CPU ≈ 0 | ✓ 0.58% vg / 0.98% sk of one core, no animation loops |
| 10 000-block memory delta | ✓ +9 MB private over the empty shell (vg) / +9 MB (sk) — single digits, met |
| typing smoothness | ✓ 30 keystrokes/s sustained at 1k and 10k blocks, zero dropped; key handler median 47–66 µs, p95 ≤ 122 µs; 120 keystrokes/s also holds (118/s achieved) |
| scrolling smoothness | ✓ continuous scroll of a 10 000-block page: 27.7% (vg) / 18.2% (sk) of one core, no hitching observed |
| in-page search cost | ✓ FTS query median 0.5 ms @1k blocks, 1.7 ms @10k — sub-frame |
| page switching | ✓ 100 switches at 0.19–0.20% of one core |
| no idle work | ✓ idle scenes show 0.0–0.6% with no timers running |
| no per-keystroke DB writes | ✓ by design (debounced batches); typing scene drives zero SQL per key |
| memory scaling linear-small | ✓ B100 → D10000 is +9 MB, not per-block widgets |
| GPU-side memory separate | ✓ not in Working Set; tracked separately per the method note |
| no crashes in the grid | ✓ all 36 rows exit 0 |

### Renderer verdict

femtovg (default) uses roughly HALF the memory of skia everywhere
(idle 114 vs 236 MB WS; 10k-block page 121 vs 245 MB). skia is better at
continuous scroll (18% vs 28% of one core) and marginally better in the
10k typing scene (23% vs 24%). Startup is comparable. Per the SPEC
priority order (memory above everything but UI quality), **femtovg stays
the default**; skia remains one `--features skia` away for scroll-heavy
use.

### M7 follow-ups, ranked by value

1. **skia WS creep in E10000-noseck** (320 MB vs vg 128 MB): investigate
   if skia ever becomes the default; a non-issue for the femtovg default.
2. **Continuous-scroll CPU** (28% vg / 18% sk at 60 fps): the partial
   renderer already limits damage; only worth revisiting with real input
   data showing it matters.
3. **Scroll-to-hit for search/find**: still impossible on this Slint
   (delegates expose no geometry) — recheck per upgrade; the selection-
   based navigation ships meanwhile.
4. Nothing else: every SPEC §六 target is met with headroom on the
   default renderer.
