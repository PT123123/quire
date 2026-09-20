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
Startup latency has two measurements, not one: `bench.ps1`'s `startup_ms` is
window-handle-up, and `startup_bench.ps1`'s `first_paint_ms` is the first
painted frame (see the A2 section at the end — they differ by ~4×).

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
numbers. Matrix methodology: idle scenes pin an independent `--db` under
%TEMP% (quire-matrix/<label>.db) so runs never share state, and every
scene runs TWICE — a fresh-DB seed pass, then the measured pass; only the
measured pass is a loaded-state number. Earlier skia rows reporting
exit_code -1 were a bench-driver bug (wrong binary + overlapping rounds),
not an app defect; they were discarded and the scene re-run.

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

## M8 · release profile audit (SPEC §二十四)

Measured 2026-09-20, `master` at `fa6a654`, femtovg default, release build
of `quire.exe` + `quire-typing.exe` per configuration. Driver:
`benchmarks/scripts/profile_bench.ps1` (a slim `bench_matrix.ps1`: scene A,
scene D idle at 10 000 blocks with a pinned temp DB, scene E typing at
10 000 blocks / 30 strokes/s / one search per 100 strokes, plus the exe
size); raw rows in `benchmarks/results/2026-09-20-release-profile-*.jsonl`.

| `[profile.release]` | exe MB | build | A warm startup / idle CPU / priv MB | D10000 startup / priv | E CPU / handler med·p95 µs |
|---------------------|-------:|------:|------------------------------------:|----------------------:|---------------------------:|
| `thin` + `cgu1` + `strip=debuginfo` (**current**) | 19.77 | 3m 10s (1m 52s warm) | 85–192 ms / 0–0.58 % / 88.3–89.7 | 135–442 ms / 97.3–99.0 | 24–30 % / 85–121 · 171–266 |
| `lto = true` (fat) + `cgu1` | 17.25 | 7m 51s | 298 ms / 0.39 % / 87.8 | 442 ms / 98.3 | 44 % / 119 · 266 |
| `thin` + `cgu1` + `panic = "abort"` | 16.14 | 4m 55s | 209 ms / 0.58 % / 88.4 | 224 ms / 97.9 | 26–30 % / 96–97 · 171–173 |
| `thin` + `codegen-units = 16` | 21.79 | 1m 45s | 146 ms / 0 % / 89.3 | 168 ms / 97.7 | 24 % / 85 · 202 |

Reading — **the profile stays exactly as it is**:

- **Runtime memory does not move.** Idle private bytes are 88–90 MB and the
  10 000-block page 97–99 MB in every configuration: a ±1.5 MB spread, which
  is this harness's noise, not a codegen effect. The memory cost of the app is
  Slint's item tree and SQLite's page cache, neither of which LTO or `codegen-
  units` touches.
- **Runtime CPU does not move.** Idle stays 0–0.6 % of one core and typing
  24–30 % in all four — repaint-bound, as the scene E reading above says.
  No configuration bought a measurable frame-budget win.
- **Startup does not move either**, and it is the noisiest of the three (see
  the caveat below): the current profile's warm window-up is 85–192 ms, and
  nothing else was outside that band.
- The only consistent differences are in exe size (fat LTO −2.5 MB,
  `panic = "abort"` −3.6 MB, `codegen-units = 16` +2 MB) and build time
  (fat LTO 2.5×). SPEC §二十四 ranks size last and forbids trading runtime or
  maintainability for it, so those deltas are not reasons to move a knob.
- **`panic = "abort"` is rejected on behaviour, not on numbers.** It is the
  only configuration that could have been tempting (smaller *and* quicker to
  build), but `abort` unwinds nothing: the panic hook `logging::install`
  registers never runs, so `panic-report.txt` is never written and
  `last_session_aborted` — the crash-recovery evidence of SPEC §二十五 and
  ADR-0018 — goes blind exactly when it is needed. `catch_unwind` (used by the
  hook's own test) cannot function either. Runtime costs nothing to keep
  `unwind`, so recoverability wins outright.
- `strip = "debuginfo"` stays as the symbol strategy: it removes the
  debuginfo the linker embeds while keeping the COFF symbol table, and
  `just dist` records the exe separately from the debug artifacts
  (§二十四's "debug artifacts 分离" is handled by keeping `target/release`
  intact next to the zip, not by stripping symbols).

**Method caveat, stated because it is the real limit of this audit:** warm
window-up startup and scene E's CPU/latency swing by ±100 ms and ±5 µs when
anything else is compiling on the machine (the parallel track's builds were
live during the first rounds — scene E's *startup* read 2 787 ms there against
409 ms on an idle machine). Scene E's `startup_ms` is not an app number at all:
the harness builds its 10 000-block page before the window appears, so that
field measures the tree plus the pinned-DB write, which is why it is quoted as
a range and why only A and D carry startup conclusions. Comparisons inside one
quiet round (rb1 vs rb2 vs cgu16) agree to within 30 ms and 10 µs, which is
the resolution this harness can honestly claim.

**What would change this conclusion:** a lever that moves idle private bytes
or typing CPU by more than the ≈2 MB / ≈3 pp noise floor. None of the four
§二十四 knobs does, so the audit's answer is "already right", recorded as a
measured outcome rather than an assumption.

## M8 · first paint, not window-up (A2, 2026-09-20)

Closes the gap carried in PLAN since M0/M1: *"window startup_ms measures
window-up, not first paint."*

**Method.** `quire.exe --measure-startup` installs a Slint rendering notifier
(`slint::Window::set_rendering_notifier`, the 1.18 public API) while the
window is still hidden, and prints one JSON line to stderr the first time the
backend reports `RenderingState::AfterRendering` — the first painted frame —
with the latency timed from the top of `main()`. Driver:
`benchmarks/scripts/startup_bench.ps1`, which also times the *old* proxy
(process → main-window-handle) in the same run, so the two numbers are never
compared across rounds. Raw rows:
`benchmarks/results/2026-09-20-first-paint-{a-shell,d-10k}.jsonl`. Normal runs
without the flag install nothing — no timer, no thread, no extra frame — and
emit no line (verified: stderr empty, exit 0).

**Boundary and its bias.** In the vendored femtovg backend the notifier fires
after the scene is rendered and the GPU commands are submitted, immediately
before `present_surface` (`i-slint-renderer-femtovg-1.18.0/lib.rs`, end of
`draw()`), so the number is a *sub-millisecond underestimate* of true
on-screen time. It also starts at `main()`, excluding OS process creation and
dynamic-linking — the harness's `stderr_paint_ms` column shows that preamble
to be ≈60–100 ms, so total launch-to-pixel is the reported figure plus that.
The **software** renderer exposes no notifier (`set_rendering_notifier`
returns `Unsupported`), and there the flag falls back to a documented proxy:
the first timer to run inside the event loop, which lands *before* any frame
is drawn — an underestimate of paint latency, marked
`"method":"event_loop_proxy","confidence":"low"` in its own line so it can
never be confused with the real thing. `renderer_name()` comes from the
build's own features, so each binary labels itself correctly.

**Numbers** (Release, femtovg, 100% scale, warm, 5 + 4 runs, medians):

| scene | window-up (old `startup_ms`) | **first paint** | invisible gap |
|-------|-----------------------------:|----------------:|--------------:|
| A empty shell | 133–142 ms (median) | **549 ms** | ≈410 ms |
| D 10 000 blocks | 124–125 ms | **693 ms** | ≈570 ms |

Run 1 of the A batch is excluded from the window-up range and called out: its
handle appeared at 940 ms (first launch of a freshly linked exe — AV scan /
cold image load) while its *first paint* was 579.7 ms, in band with the other
four. The paint number is stable even when the handle number is not, which is
the practical argument for measuring paint.

**Reading.** The window handle is up in ~140 ms and the user sees a complete
frame ~400–550 ms later — the old number understated perceived startup by
**3.9× (scene A) and 5.6× (scene D)**. The gap is *not* decomposed here; the
candidates in it are the app's own pre-loop work (open + integrity-check the
SQLite file, rebuild the workspace tree and document, bind the controller) and
Slint's first-frame setup (glyph atlas, shader/program build). What the data
does say is that the gap barely scales with the document: 10 000 blocks add
only ≈145 ms to first paint, because virtualization means the frame renders
the visible rows, not the page — so the fixed part (setup, not content)
dominates.

**Conclusions.** (1) Quote first-paint, not window-up, as startup latency;
bench.ps1's `startup_ms` remains what it always measured (handle-up) and the
two are not interchangeable. (2) ≈410 ms of pre-paint work is now visible to
attack, and the storage layer is already known not to be most of it: the ADR-
0015 probe puts a full `SqliteRepository::open` — 10 000 blocks, rotate +
snapshot — at 46–116 ms, i.e. ≤ 25 % of scene D's gap even at the pessimistic
end. The remaining ~300–500 ms is the tree/document rebuild plus Slint's
first-frame setup, so a follow-up worth its cost would stamp the phases inside
`real_main` behind the same flag and see which half moves. (3) The skia
comparison is deliberately not claimed here: it needs its own release build,
and A3's lesson about measuring while a parallel track compiles applies. Left
as a follow-up (`--features skia` + the same script).

## M8 · where the pre-paint time actually goes (A2 follow-up, 2026-09-20)

The follow-up in conclusion (2) above, landed. `--measure-startup` now also
stamps each step of `real_main` (`src/main.rs`, `PhaseLog`) and appends them to
the same JSON line as `"phases":[{"phase":…,"ms":…,"at_ms":…}]` — `ms` is the
step's own cost, `at_ms` its end relative to the start of `main()`. One
`Instant::now()` per stamp, and only when the flag is set. `startup_bench.ps1`
carries the array into each run row (`phases_raw`) and reports per-phase
medians plus `in_run_loop_ms_median` — first paint minus the last stamp, i.e.
everything that happens *after* our own setup returns.

**Numbers** (Release, femtovg, 100% scale, warm, 7 runs each, medians; the two
scenes are the same A/D pair as the section above):

| phase | A empty shell | D 10 000 blocks |
|-------|--------------:|----------------:|
| `logging_init` | 2 ms | 2 ms |
| `data_location` (resolve + one-off move) | 0 ms | 0 ms |
| **`repo_open`** (SQLite open + integrity + rotate) | **57 ms** | **56 ms** |
| `appwindow_new` (Slint window build) | 20 ms | 21 ms |
| **`state_new`** (tree + document rebuild) | 5 ms | **148 ms** |
| `bind_wire` / `import_open` / `lan_setup` / `pre_event_loop` | 0 ms | 1 ms |
| — subtotal inside `real_main` | ≈84 ms | ≈233 ms |
| **inside `ui.run()` up to the first frame** | **≈419 ms** | **≈413 ms** |
| first paint | **501 ms** | **651 ms** |
| window-up (old proxy) | 155 ms | 142 ms |

**Reading.** The gap is not one mystery, it is two, and they are nothing alike:

1. **A fixed ≈410–420 ms floor inside `ui.run()` before the first frame**, and
   it is *flat*: 419 ms for an empty shell, 413 ms for 10 000 blocks (per-run
   range 391–447 ms across both scenes). Nothing we do in `real_main` moves it.
   That window is Slint starting its event loop, showing the window, and the
   renderer's first-frame setup — GL context, glyph atlas, shader/program
   build. It is 84 % of scene A's startup and 63 % of scene D's, so it is the
   only large, document-independent lever left in the start path.
2. **A size-dependent term that is exactly one phase**: `state_new`, 5 ms →
   148 ms. That is the workspace-tree and document rebuild, and it is the whole
   difference between the two scenes (233 − 84 = 149 ms). It confirms the
   virtualization story from above — 10 000 blocks cost model rebuild time, not
   frame time.

**Corrections to the section above.** (a) `repo_open` measured 51–64 ms in
every run of both scenes (one 126 ms outlier, run 7 of D, whose paint was still
in band), so the ADR-0015 probe's 46–116 ms band holds at its *low* end and the
storage layer is confirmed to be ~11 % of scene A and ~9 % of scene D — not
"≤25 %". (b) The earlier medians (549 / 693 ms) were 5 + 4 runs; this batch of
7 runs each is tighter (A 480–534, D 602–727) and lower. Treat the two batches
as the same measurement at different precision, not a regression or an
optimisation. (c) The "invisible gap" of the previous section ≈ the
`in_run_loop` column plus `state_new`; the guess that it was "rebuild plus
first-frame setup" was right, and the split is now measured: setup 419 ms,
rebuild 148 ms, storage 57 ms.

**Conclusions.** (1) Startup work now has an owner per phase; anything that
promises "faster startup" should say which column it moves. (2) The only lever
big enough to matter for the empty-shell case is the ~410 ms renderer/window
floor, which is upstream — the practical mitigations are showing *something*
sooner (paint the frame before the GL/shader warm-up completes, or a splash
that is honest about it), not shaving our own ~84 ms. (3) `state_new` is the
term that scales, so it is the one to watch if the model rebuild grows; a
5 000-block point would tell us whether 148 ms is linear or a cliff.

## M8 · is the scaling term linear? (A2 follow-up #2, 2026-09-20)

Conclusion (3) above asked for the 5 000-block point and got a whole series:
four points in one batch, same build, same machine moment, 7 runs each.

**Method.** `startup_bench.ps1 -Exe target\release\quire.exe -Runs 7 -Blocks N`
with N = 0 / 1 000 / 5 000 / 10 000, labels `scale-a`, `scale-1k`, `scale-5k`,
`scale-10k`; each run gets its own scratch database. The exe is the release
build of `6c115b5` (the newest Rust/UI commit — everything after it in this
session touched docs and scripts only). Raw rows:
`benchmarks/results/2026-09-20-first-paint-scale-*.jsonl`. Every number in the
table above is regenerable with `benchmarks/scripts/audit_results.ps1`: it
recomputes each stored summary row from the runs beside it and exits 1 if they
disagree, prints the per-phase medians, and ends with a cross-batch view of the
first-paint batches grouped by document size — which is where the drift
paragraph below comes from.

**Numbers** (medians, ms):

| phase | shell | 1 000 | 5 000 | 10 000 |
|-------|------:|------:|------:|-------:|
| `repo_open` | 51 | 51 | 52 | 51 |
| `appwindow_new` | 21 | 20 | 22 | 21 |
| **`state_new`** | **5** | **17** | **68** | **136** |
| `logging_init` | 2 | 2 | 2 | 2 |
| everything else | 0 | 0 | 0 | 1 |
| inside `ui.run()` to the first frame | 388 | 388 | 378 | 369 |
| **first paint** | **466** | **485** | **522** | **581** |
| window-up (old proxy) | 136 | 148 | 124 | 144 |

**Answer: linear, no cliff.** Fitting `state_new` on the four points gives
`≈ 4 + 13.1 × (blocks / 1000)` ms, and that line predicts 4.1 / 17.2 / 69.6 /
135.2 against the measured 5 / 17 / 68 / 136 — the worst residual is 1.5 ms.
The 148 ms of the previous section was not the start of a knee; it is the same
13 ms per thousand blocks the 1 000-block page already pays. A 20 000-block
page should therefore cost ≈266 ms of rebuild, and SPEC §六's "10 000 blocks
without hitching" budget can be stated as a formula instead of a data point.

**Two things the series settles that two points could not.**

1. **`repo_open` is O(1) in document size.** 51 / 51 / 52 / 51 ms while the
   database it opens grows to 10 000 blocks. The SQLite open, integrity probe
   and snapshot rotation never look at the rows — the whole load cost is in
   `state_new`. That also re-pins the ADR-0015 band: its 46–116 ms was measured
   *with* a 10 000-block file, and the flat column says the size contribution
   inside that figure is nil.
2. **The `ui.run()` floor is flat over a 20× document range**, not just over
   the two scenes measured before: 388 → 369 ms, a spread of 19 ms against a
   first-paint change of 115 ms. Virtualization is doing exactly what M7
   claimed — the frame renders the visible rows.

**Cross-batch drift, stated plainly.** The shell scene has now been measured
three times today at 549 (5 runs, pre-phase build), 501 (7 runs, same build)
and 466 ms (7 runs, `6c115b5`). The floor moved the same way (≈419 → ≈388).
Two of those batches differ in build *and* in machine moment, so none of it is
attributable — treat it as ±15 % session drift, which is why the fit above uses
one batch only. The practical rule for anyone re-running this: quote deltas
within a batch, never across.

**Conclusions.** (1) The startup story is now: ≈380 ms renderer/window floor
(document-independent, upstream), ≈75 ms of fixed app setup (of which storage
is 51 ms), and one linear term at 13 ms per 1 000 blocks. (2) If someone wants
a faster start for the common case, the floor is the only prize; for the heavy
case, the rebuild is the only term that grows, and it grows slowly enough that
no cliff is hiding between 10 000 and 20 000 blocks. (3) The skia comparison is
still open, and the reason is narrower than it was costed: `target-skia/` is
still on disk from the M7 matrix, and neither `Cargo.lock` nor
`[profile.release]` has changed since `3498618`, so a `--features skia` build
there should be incremental (the `quire` crate alone) rather than the ≈10 min
full rebuild it was estimated at — unmeasured, so treat it as the optimistic
case. What it really needs is a settled tree: the comparison is only honest if
both binaries come from the same source state, and the femtovg batch above came
from a committed one. Deferred on those grounds, not on cost. (4) It stopped
being optional on 2026-09-20: the M9 evaluation measured that Slint 1.18
compiles FemtoVG out for Android (`cfg(not(target_os = "android"))`) and
renders there through **skia on GLES**, so any on-device number will only be
interpretable against this desktop skia baseline (evidence in
`.scratch/m9/report.md`).
