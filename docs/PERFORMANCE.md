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
A scene with pictures carries one number the process counters cannot see, so
the app prints it: `--dump-state` makes the exit write
`{"event":"attachment_cache","bytes":…,"peak_bytes":…,"entries":…,"budget":…,"scroll_y":…}`
to stderr, and `bench.ps1` inlines it as the row's `attachment_cache` field.
`peak_bytes` is a high-water mark recorded on every cache insert, and `scroll_y`
is where the programmatic scroll got to — the field is there because a cache
that never grew is only a finding if the page really moved.
A renderer comparison runs both binaries in one sitting and refuses to trust
either arm's identity: `scroll_ab.ps1` pre-flights each one against the
`renderer` field of its own `first_paint` line, because a skia arm built without
`--no-default-features` renders through femtovg and would produce a
femtovg-vs-femtovg batch that looks like an A/B.

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
| D+F·P | the same pages with N picture rows | `bench.ps1 -Blocks 10000 -Pictures N [-Scroll -ScrollStep 200]` |
| F·2 | the scroll arms, both renderers, one sitting | `scroll_ab.ps1` (8 px and 200 px per frame, plus the picture page) |

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
  **Corrected 2026-09-21, and this row is the one to re-read:** the timer was
  adding to `content-y`, which Slint measures *negative* going down, so scene F
  never moved the viewport. What this row measures is 60 property writes a
  second at scroll 0 — a repaint loop, not a scroll. The same-session re-run of
  the real thing is in "M10 · the media bench scene" at the end, along with the
  pixel control that proves the direction.
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
over 8 s of that, which is ~350 strokes at 30/s. Since 2026-09-20 the harness
deletes that DB, its `.bak<N>` snapshots and its own report file when the run
ends — the pre-run purge covered only the `.db`, so every label ever used left
a snapshot in `%TEMP%`.

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
| scrolling smoothness | ⚠ re-derive before quoting — "continuous scroll of a 10 000-block page: 27.7% (vg) / 18.2% (sk) of one core, no hitching observed" was measured on a scene F that never scrolled (see the M2 reading above and the media batch at the end): both arms are a pinned-at-top repaint loop. Re-derived 2026-09-21 on the fixed scene, both renderers, one sitting: **36.9–43.1 % (vg) / 31.8–33.7 % (skia/GL)** at a wheel tick, 86.1–87.4 / 66.4–70.3 at a flick — the harness measures CPU, not frames, so "no hitching" was never something it could see, and that half of the row still needs a human scroll |
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
10k typing scene (23% vs 24%). Startup is comparable — and M8 later measured
that word to mean skia ≈44 ms faster on an empty shell and a tie on a 10 000
block page, in one batch (see *the skia first-paint baseline* below). Per the
SPEC priority order (memory above everything but UI quality), **femtovg stays
the default**; skia remains one `--no-default-features --features skia` away
for scroll-heavy use — plain `--features skia` leaves the femtovg default
compiled in too, and then it renders with femtovg.

*(2026-09-21: the one arm of this verdict that argues for skia was the scroll
number, and scene F was not scrolling — see the M2 reading and the media batch
at the end. That arm has now been re-measured on a scroll that does move, in one
sitting against its own femtovg control: skia/GL is **still** cheaper to scroll,
by ≈18 % at a wheel tick and ≈21 % at a flick rather than the third this row
claimed, for ≈+34…43 MB of working set. femtovg's memory lead is untouched and
the default stands; "skia for scroll-heavy use" is a proven but smaller offer
than this paragraph advertised — the table is in* ***M10 · the skia arm of the
real scroll*** *at the end of this file.)*

### M7 follow-ups, ranked by value

1. **skia WS creep in E10000-noseck** (320 MB vs vg 128 MB): investigate
   if skia ever becomes the default; a non-issue for the femtovg default.
2. **Continuous-scroll CPU** (28% vg / 18% sk at 60 fps): the partial
   renderer already limits damage; only worth revisiting with real input
   data showing it matters. *(2026-09-21: both numbers in this row are off a
   scene that never moved; on the fixed scene the pair is 36.9–43.1 /
   31.8–33.7 at a wheel tick and 86.1–87.4 / 66.4–70.3 at a flick, so the
   item is now "a 10 000-row page costs most of a core to flick on the default
   renderer" — a bigger number, same advice: revisit when real input data says
   it hurts, and see the M10 scroll rows at the end.)*
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
build's own features, which labels an *app* binary correctly — the one exception
was `quire-shot`, which installs its own software platform and so was captioning
its shots `FemtoVG · GL` while no femtovg code ran; it now overwrites the label
(see `docs/UI_ARCHITECTURE.md`, Visual regression).

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
`.scratch/m9/report.md`). It is measured two sections down; the costing in (3)
turned out to be the least interesting part of the exercise.

## M8 · the skia first-paint baseline (A2 follow-up #3, 2026-09-20)

Conclusion (3) of the section above, closed the same day — after the
instrument itself had to be repaired.

**Two traps, both worth recording because neither is visible in the docs.**

1. `--features skia` does **not** disable the package default, so it builds a
   binary with *both* `femtovg` and `skia` compiled in, and Slint picks
   femtovg. The build that looked like the skia arm was a second femtovg arm;
   `renderer_id()` said so (`"femtovg"`), which is the only reason it was
   caught. A skia-only binary needs `--no-default-features --features skia…`.
2. The default `renderer-skia` build is **mute** to the A2 instrument on this
   machine: `set_rendering_notifier` returns `Ok(())` and no `RenderingState`
   is ever delivered — while the window does paint (working set 251 MB vs the
   femtovg arm's 120 MB, so real GPU work is happening). Upstream cause: all
   three notify sites are inside `Surface::with_graphics_api(..)`, whose trait
   default is an empty function (`i-slint-renderer-skia-1.18.0/lib.rs:1052`),
   so a surface that does not override it stores the callback and never calls
   it. The OpenGL surface does override it, and `--no-default-features
   --features skia-opengl` immediately produced
   `RenderingSetup / BeforeRendering / AfterRendering`. The rule for anyone
   re-running this: **a silent skia run is not a negative result** — check the
   surface before concluding anything about the frame.

**Method.** One batch, both arms from the identical working tree (rebuilt
within 60 s of each other), 5 runs per arm per scene, femtovg in `target/`,
skia-opengl in `target-skia/`. Driver `.scratch/a9/run_ab.ps1`, which refuses
to start the batch unless each arm reports the renderer it claims to be.
Raw rows: `benchmarks/results/2026-09-20-first-paint-ab-*.jsonl`, all four
recomputed clean by `audit_results.ps1` (12 first-paint + 6 bench batches,
"audit ok").

**Numbers** (Release, 100 % scale, warm, medians, min–max in parentheses):

| scene | arm | first paint | inside `ui.run()` | window-up | stderr-observed paint |
|-------|-----|------------:|------------------:|----------:|----------------------:|
| A empty shell | femtovg | 484 (473–534) | 406 (387–448) | 143 (124–173) | 567 (557–611) |
| A empty shell | **skia/GL** | **440** (437–482) | 359 (356–398) | 139 (126–167) | 525 (518–593) |
| D 10 000 blocks | femtovg | 619 (602–683) | 396 (389–450) | 131 (120–164) | 705 (700–757) |
| D 10 000 blocks | **skia/GL** | 609 (573–687) | 381 (361–437) | 168 (126–186) | 702 (663–771) |

The arms' own setup phases match to within noise — `repo_open` 52 vs 51 ms,
`appwindow_new` 21 vs 20, `state_new` 8 vs 8 on scene A, 143 vs 144 on scene D
— which is the control that lets the difference be read as a renderer
difference at all.

**Reading.** (1) The whole delta is in the `ui.run()` floor, as it must be:
nothing in `real_main` differs between the arms, and indeed −47 ms of the
−44 ms scene-A gap lands in that column. (2) It is a *small* win, not a
threshold crossing: −9 % on the empty shell, and on the 10 000-block page the
two ranges overlap (skia 573–687 vs femtovg 602–683), so scene D is a tie.
(3) The old proxy would have told the opposite story: `window_up_ms` says skia
is 37 ms *slower* on scene D (168 vs 131) where first paint says equal.
Fourth independent demonstration that handle-up is not startup.

**Conclusions.** (1) skia is not a startup lever worth its price — ≤ 44 ms,
only on the empty shell, against the ≈2× working-set cost M7 measured (idle
236 vs 114 MB). `femtovg` stays the default; nothing here reopens that.
(2) The baseline M9 asked for now exists, and it exists specifically for the
`skia-opengl` surface, which is the right family for Android's skia-on-GLES
renderer — but this is desktop GL on an Intel iGPU, so it is a floor for
device numbers, not a prediction of them. (3) The desktop `skia` (wgpu /
softbuffer candidates) arm has **no** first-paint number, and cannot have one
with the current method; if it ever needs one, measure it from outside the
process, not through the notifier.

## M10 · the picture cache, and the gate it had to clear (2026-09-20, ADR-0029)

SPEC §三十七 puts a measured gate on every new block kind: the 10 000-block
scene must stay inside 1.2× the recorded RAM baseline. `image` is the kind that
gate was written for — it is the only one that can put megabytes into the
process — so it is measured before it is called done.

Release `quire.exe`, `benchmarks/scripts/bench.ps1`, every idle scene run as a
fresh-DB seed pass plus a measured pass against its own pinned `%TEMP%`
database. Raw rows: `benchmarks/results/2026-09-20-m10-image-ram.jsonl`.

| scene | passes | WS MB | private MB | idle CPU % | M7 matrix | ratio |
|-------|--------|------:|-----------:|-----------:|-----------|-------:|
| A empty shell   | 2 | 116.5 – 116.6 | 88.1 – 88.9 | 0.20 – 0.39 | 114.3 / 88.2 | 1.02× |
| D 10 000 blocks | 4 (2 seed + 2 measured) | 126.4 – 126.5 | 100.9 – 101.3 | 0.39 – 0.59 | 121.5 / 97.4 | 1.04× |

**Reading.** The gate holds with room: 1.04× on the number it is written
against, and the four D passes spread by 0.4 MB, so this is not a difference
that could hide a regression. A 10 000-block page now costs **+10 MB WS /
+12.5 MB private** over the empty shell, against the +7.2 / +9.2 the M7 matrix
recorded — the widening is inside the noise of two batches taken a day apart
on an iGPU box, and it is the honest number to compare the next kind against.

**What this batch does *not* measure.** Neither scene contains a picture. These
rows say the v7 columns, the `attachments` map and the cache fields did not
make everything else dearer; they do not say pictures are cheap. The actual
picture cost is bounded by construction instead: `AppState` holds decoded
rasters in an LRU weighted at `w × h × 4` and capped at
`MAX_ATTACHMENT_CACHE_BYTES = 32 MiB` (≈eight full-width 1280×720 frames), and
`the_picture_cache_spends_its_budget_and_drops_the_stalest_first` asserts both
the ceiling and that eviction is by last-realized, not by insertion order. The
reason a ceiling is needed at all is upstream: Slint's own decode cache
(`i-slint-core-1.18.0/graphics/image/cache.rs`) is a thread-local `CLruCache`
weighted by decoded bytes with a **5 MiB** budget keyed by path + mtime — one
photograph — so relying on it would re-decode on every frame of a scroll
through a photo page.

**Follow-up owed.** A bench scene that seeds N image blocks and scrolls them is
the missing measurement; it belongs with the `file` / PDF thumbnail kind, which
reuses the same store and will want the same scene.

## M10 · the file kind, which had to cost nothing because it reads nothing (2026-09-20, ADR-0030)

`file` is the second kind under the same §三十七 gate, and it is the kind the
gate was least likely to catch: the store `fs::copy`s the bytes and records
their length, so nothing of an attachment ever enters the process to be
painted. The row reads one `attachments` line and formats one string. The
measurement below is therefore a **non-regression** check — proof the extra
block kind, the three new `UIState` callbacks and the wider slash menu did not
make the rest of the app dearer — not a claim about attachments.

Release `quire.exe`, `benchmarks/scripts/bench.ps1`, same method as the batch
above. Raw rows: `benchmarks/results/2026-09-20-m10-file-ram.jsonl`.

| scene | passes | WS MB | private MB | idle CPU % | M7 matrix | ratio |
|-------|--------|------:|-----------:|-----------:|-----------|-------:|
| A empty shell   | 2 | 119.2 – 120.7 | 91.7 – 92.5 | 0.19 | 114.3 / 88.2 | 1.05× |
| D 10 000 blocks | 2 (1 seed + 1 measured) | 126.5 – 126.6 | 101.1 – 101.3 | 0.00 – 0.20 | 121.5 / 97.4 | 1.04× |

**Reading.** D is the number the gate is written against, and it is the same
number the `image` batch recorded — 126.4 – 126.5 / 100.9 – 101.3 against
126.5 – 126.6 / 101.1 – 101.3, i.e. inside 0.2 MB across two batches a session
apart. A moved: 116.5 – 116.6 → 119.2 – 120.7 WS, +2.6 to +4.1 MB. **That delta
is not attributable to this slice, and it is not attributed here.** Neither A
run contains an attachment, so it cannot be the file row's cost; the two A
passes of this batch also spread 1.5 MB against the previous batch's 0.1 MB,
which says the empty-shell reading is less repeatable than the D reading and
that some of the shift is session rather than code. The gate is ≤1.2×, so at
1.05× there is no decision hanging on resolving it; recording it unresolved is
the honest version, and the next kind measured on A gets to compare against
*this* row, not the 116.5 one.

**What this batch does not measure.** A page full of file blocks. The row's
cost is one attachments lookup and one `format_size` per *realized* row — the
`attachment-size` callback exists precisely so that is viewport-proportional
rather than document-proportional — but that is an argument plus the unit test,
not a reading, and it is the same shape of gap the picture batch flagged above.
The scrolled-media scene owed there is now owed twice, and it should measure
pictures and files on one page.

## M10 · the table kind, and the one number that was not the table's (2026-09-20, ADR-0031)

`table` is the third kind under the §三十七 gate, and it is the first one whose
cost is per *row* rather than per *block with a payload*: a grid is child
blocks, so a page with a table on it holds more blocks but no more megabytes of
anything — there is no raster, no file, no cache.

Release `quire.exe`, `benchmarks/scripts/bench.ps1`, same method as the two
batches above (pinned `%TEMP%` database, one seed pass then measured passes).
Raw rows: `benchmarks/results/2026-09-20-m10-table-ram.jsonl`, which holds both
arms of the A/B described at the end.

| scene | passes | WS MB | private MB | idle CPU % | M7 matrix | ratio |
|-------|--------|------:|-----------:|-----------:|-----------|-------:|
| A empty shell   | 3 (1 seed + 2 measured) | 124.8 – 125.3 | 96.7 – 98.3 | 0.00 – 0.39 | 114.3 / 88.2 | 1.10× |
| D 10 000 blocks | 3 (1 seed + 2 measured) | 130.8 – 131.0 | 104.5 – 105.7 | 0.00 – 0.39 | 121.5 / 97.4 | 1.08× |

**Reading.** The gate holds, at 1.08× on the number it is written against, and
D − A is +5.5…6.2 MB WS / +6.2…9.0 private where the M7 matrix recorded +7.2 /
+9.2 for ten thousand blocks — the per-block cost of a page did not change,
which is what "cells are just blocks" is supposed to buy.

**Against the previous batch, both arms moved together, so neither movement is
the table's.** The `file` batch recorded A 119.2 – 120.7 and D 126.5 – 126.6;
this one records A 124.8 – 125.3 and D 130.8 – 131.0, i.e. **+4.1…6.1 on the
empty shell and +4.2…4.5 on the 10 000-block page**. A carries no table and no
attachment, so the shift cannot be this slice's per-row cost — and the fact that
the two arms shifted by nearly the same amount is the shape of a session that got
dearer, not of code that got bigger. This is the third batch in a row to record a
+3…5 MB drift on A (116.5 → 119.9 → 125.1) with nothing in the diff to attribute
it to. It is reported as drift, and the ratio that matters is taken against the
M7 matrix, which is the contract the gate is written on.

**The A/B that *is* attributable.** The first cut of the projection built every
row's `table_cells` model by scanning the page for that row's cells — for every
row, table or not — which is both a quadratic scan on a 10 000-block page and
one empty `VecModel` (with its `ModelNotify`) allocated per row. Guarding the
field on `kind == Table` was measured against the unguarded build in the same
session, same machine, same script:

| arm | unguarded WS / private | guarded WS / private |
|-----|------------------------|----------------------|
| A empty shell (3 passes)  | 124.9 – 125.2 / 96.1 – 98.1 | 124.8 – 125.3 / 96.7 – 98.3 |
| D 10 000 blocks (3 passes) | 132.2 – 133.1 / 106.8 – 108.4 | 130.8 – 131.0 / 104.5 – 105.7 |

A is flat to within its own spread — the right control, because the empty shell
is the seeded demo page and projects 24 rows, so it pays the per-row cost 24
times either way — while D drops
≈2 MB WS and ≈2.5 MB private, i.e. ≈200–250 bytes per row, which is what an
empty model costs. Startup did **not** move (475–525 ms unguarded vs 498–552 ms
guarded), and that is a limit of the method rather than a verdict on the scan:
`startup_ms` stops at the window handle, and the projection demonstrably happens
somewhere past that point, so the quadratic work is real but outside what this
instrument sees. The guard stays because of the memory reading plus the
asymptotics, not because a number moved.

**What this batch does not measure.** A page with a *big* table on it: the scene
seeds 10 000 ordinary paragraphs, so the grid's own projection, its delegate
heights and the per-cell `Text` measurement are unmeasured at scale, and
`grid_blocks` sorts a table's cells on every projection. The scrolled-media
scene owed twice above now has a third shape to cover.

## M10 · the layout kind, and the drift that finally had a number on it (2026-09-20, ADR-0032)

`columns` is the fourth kind under the §三十七 gate, and like the grid it costs
rows rather than payloads: a layout is child blocks all the way down, so the
page holds more blocks and no more bytes of anything. What is new for the gate is
that the layout's content rides on the layout's own row as **two extra model
fields** (`column-items`, `column-boxes`), which is exactly the shape ADR-0031
caught costing ≈2 MB on the bench page when it was left unguarded.

Release `quire.exe`, `benchmarks/scripts/bench.ps1`, same method as the three
batches above (pinned `%TEMP%` database, one seed pass then measured passes).
Raw rows: `benchmarks/results/2026-09-20-m10-columns-ram.jsonl`.

| scene | passes | WS MB | private MB | idle CPU % | M7 matrix | ratio |
|-------|--------|------:|-----------:|-----------:|-----------|-------:|
| A empty shell   | 3 (1 seed + 2 measured) | 126.5 – 127.0 | 98.4 – 99.6 | 0.00 – 0.20 | 114.3 / 88.2 | 1.11× |
| D 10 000 blocks | 3 (1 seed + 2 measured) | 135.5 – 137.0 | 109.9 – 110.7 | 0.00 – 0.78 | 121.5 / 97.4 | 1.13× |

**Reading.** The gate holds, at 1.13× — the widest the gate has been from, and
still inside it. Both arms moved again against the table batch (A +1.2…1.7,
D +4.5…6.0), so the drift the last three batches reported is now four batches
long and running the same direction.

**Why this slice is not the cause, and what it does cost.** The projection builds
the two models only for a `kind == Columns` row; every other row gets
`ModelRc::default()`, which in Slint 1.18 is `ModelRc(None)` — checked in
`i-slint-core-1.18.0/model.rs:891`, and it allocates nothing. So the only
per-row cost the layout adds on a page that holds no layout at all is two
8-byte pointers widened into `BlockRow`: ≈160 KB across the bench page's 10 000
rows, not the ≈3.5 MB that D − A grew by. The arithmetic does not reach the
reading, so the reading is reported as drift and not attributed.

**The gate's method is now the finding.** Four batches of the same script on the
same machine have moved A by +10.7 MB (116.5 → 127.0) with nothing in any of
those diffs that could paint an empty shell. A ratio taken against a matrix
recorded in a different session is therefore measuring the session as much as
the code, and at 1.11× there is one batch of drift left before a slice that
changes nothing at all fails a ≤1.2× gate. The fix is not a wider threshold: it
is a **same-session control** — build the commit before the slice into its own
target dir and measure both arms in one sitting, the way ADR-0031's projection
guard was settled. That control has not been run yet and is owed here; the
number it would produce is the one the next kind should be compared against.

**What this batch does not measure.** A page full of layouts. The bench scene
seeds 10 000 ordinary paragraphs, so the flexbox tiling, the per-box delegate
count and the `column_projection` walk are unmeasured at scale — and
`can_fold` still scans the whole page for every row, which is a pre-existing
quadratic the table batch named and this batch neither fixed nor made worse. The
scrolled-media scene owed above now has a fourth shape to cover.

## M10 批次 A · what one Ctrl+V of a screenshot costs (2026-09-21, ADR-0035)

Pasting a picture from the clipboard adds no block kind, no schema column and no
per-row state — it produces an ordinary `image` block through the store path a
picked file already uses — so no RAM arm is owed for it and none was run (the
sweep confirms it: 0 of 44 scenes moved). What a paste does own is a **latency**:
it runs synchronously on the key press, so the question is how long the user
waits, not how much they keep.

Two legs, both measured in release-profile lib tests that print instead of
asserting (a timing assert would fail on a loaded machine and prove nothing):

```
cargo test --release --lib screenshot -- --ignored --nocapture
```

| leg | input | release ms | note |
|-----|-------|-----------:|------|
| DIB → RGBA (`dib::decode`) | 1920×1080 32-bit, 8.29 MB | 19 | ≈100 Mpixel/s through a per-pixel Rust loop |
| | 3840×2160, 33.18 MB | 74 | scales with pixels, as it must |
| RGBA → PNG (`dib_to_png`) | the same two rasters | 1 / 6 | **floor, not estimate**: the fixtures are one flat colour, so the entropy coder has nothing to do |
| store (`import_bytes`: PNG decode + `MAX_EDGE` downscale + cache encode + two writes) | 1080p PNG, 6.22 MB | 59 | **ceiling, not estimate**: the fixtures are random pixels, which no screen is; a real snip stores for less |
| | 4K PNG, 24.79 MB | 196 | |

**Reading.** A full-screen paste is tens of milliseconds of decode plus tens more
in the store — call it ≈80…250 ms at the top of this table, and clearly less for a
real snip. That is a key press, not a frame: nothing here runs in a projection or
a paint, `AttachmentStore`'s header rule is that import is only ever called from a
user action, and the picture cache is reached the same way by a pasted row as by a
picked one (through `image-for`), so the paste adds no second cache. §三十七's
ceilings hold where they were: `parse` refuses an edge over 16 384 or 64 M pixels
before it allocates, and both fixtures above are inside that.

**What this batch does not measure.** The clipboard read itself (`GlobalLock` plus
one `Vec` copy — no number taken), the PNG encode of real screen content (the 1 /
6 ms row is a flat-colour floor), and the thing the last four batches have each
named: **a page of pictures being scrolled**. The store leg above is one picture at
one moment; the ceiling that protects a scroll is
`MAX_ATTACHMENT_CACHE_BYTES = 32 MiB`, still a construction bound plus a unit test
rather than a reading.

## M10 · the media bench scene, and the scroll that never scrolled (2026-09-21, ADR-0036)

Four batches in a row ended with the same paragraph: *a page of pictures being
scrolled is still unmeasured, and the 32 MiB decode ceiling is a construction
bound plus a unit test, not a reading.* This batch exists to turn that sentence
into a number. It produced a different one first.

Release `quire.exe` built at this commit, `benchmarks/scripts/bench.ps1`, every
scene one fresh-DB seed pass plus two measured passes against its own pinned
`%TEMP%\quire-matrix\<label>.db`, all nine scenes in one sitting (the
same-session control the layout batch asked for, as far as a scene that adds no
kind can use it). Raw rows:
`benchmarks/results/2026-09-21-m10-media-ram.jsonl`. `entries` / `peak` are the
app's own line, not a process counter.

| scene | WS MB | private MB | idle CPU % | rasters | cache peak |
|-------|------:|-----------:|-----------:|--------:|-----------:|
| A empty shell | 125.7 – 126.1 | 99.1 – 99.5 | 0.00 | — | — |
| D 10 000 blocks | 135.7 – 136.1 | 111.3 – 112.2 | 0.00 – 0.20 | — | — |
| D + 500 pictures | 135.4 – 136.1 | 110.9 – 112.4 | 0.20 – 0.39 | 0 | 0 |
| D + 5 000 pictures | 153.3 – 153.9 | 118.7 – 119.7 | 0.00 – 0.20 | 2 | 7.03 MB |
| F 8 px/frame (wheel) | 150.0 – 150.8 | 121.4 – 122.6 | 36.5 – 38.4 | — | — |
| F 200 px/frame (flick) | 156.3 – 156.4 | 133.1 – 134.9 | 83.1 – 87.5 | — | — |
| F + 500 pictures, flick | 191.1 – 193.0 | 165.6 – 166.3 | 77.4 – 78.6 | 9 | 31.64 MB |
| F + 5 000 pictures, flick | 190.3 – 190.8 | 149.0 – 149.8 | 61.4 – 62.3 | 9 | 31.64 MB |
| F · 1 000 rows, 500 pictures, flick | 183.3 – 184.4 | 143.8 – 145.2 | 58.2 – 65.4 | 9 | 31.64 MB |

**The ceiling binds, at nine.** Every scrolled media arm landed on the same
number: 9 rasters, `peak_bytes` 33 177 600 against a 33 554 432 budget — 98.9 %
of it, and a tenth 1280×720 frame would be 36 864 000. Nine is
`floor(32 MiB ÷ 3 686 400)`, so what stops the cache is its byte weight and not
an entry count, and it stops in exactly the same place whether the page holds
500 pictures or 5 000 or whether it is 10 000 rows or 1 000. That spread of
zero across three shapes is the reading `the_picture_cache_spends_its_budget…`
asserts in a unit test; the bench says it happens through Slint's real
ListView, with real paints, on a real frame clock.

**A picture costs nothing until it is on screen.** D + 500 is D — 135.4 – 136.1
against 135.7 – 136.1, inside the spread of the passes themselves — with 0
rasters decoded and 0 bytes of cache. The first picture row on that page sits at
index 19 and the initial viewport does not reach it. A `BlockRow` carries an
attachment as one i32, so 500 pictures on a page you never scroll to cost half a
megabyte and no decodes at all: §三十七's "cost tracks the viewport, not the
document" is now a measured claim rather than a description of the callback.

**And a picture on screen costs about three times its raster.** D + 5 000 is
+17.6 MB WS over the same page without them, and the cache holds 7.03 MB of it —
two frames. The other ≈10 MB is Slint's own 5 MiB path cache plus the texture
upload for a 1280-wide RGBA raster on the iGPU, neither of which the LRU sees.
The scrolled arms say the same thing bigger: +35 MB WS / +32 MB private over the
same page flicked with no pictures on it, for a cache holding 31.64 MB. So
§二十二's arithmetic must budget roughly **9 MB of process memory per on-screen
picture, not the 3.7 MB of its raster** — and the corollary is that the 32 MiB
cache is not the binding constraint on a photo page; the viewport is. Nothing
about the ceiling needs raising: at 9 frames it is reached before the screen can
show a tenth.

**Where the gate stands.** The §三十七 arm is D 10 000 blocks, and here it is
135.7 / 111.3 = 1.12× / 1.14× the M7 matrix rows (121.5 / 97.4), with A at
1.10× / 1.13× — the same session, so the two arms' spread is this machine today
rather than a diff. The media arms are *outside that gate on purpose*: they
exceed 1.2× the baseline (193 / 121.5 = 1.59×) because their content is
photographs, which is what the gate was written to price and not what it was
written to forbid. A kind that added per-row state would show up in the D + 500
row, which is flat.

**The finding the batch was not looking for: scene F never scrolled.** The
instrument was believed broken before it was believed wrong — a 10 000-row page
with 5 000 pictures, driven for 8 s, reported 2 rasters and a `scroll_y` of
+840. Doubling the step to 200 px made the page move *less* per second, which is
not what a slow renderer does. So the report grew a `scroll_y`, and the shot tool
grew a `--scroll-y`, and the pixel control settled it: `--scroll-y 0` and
`--scroll-y 2000` produce **byte-identical PNGs** while `--scroll-y -2000`
differs. Slint's `content-y` is negative going down — its own ListView computes
the first visible index as `round(-content-y / item-height)` — and the timer
installed in M2 had been *adding* ever since. Every F row in this file predates
the fix: the M2 baseline's 19.49 %, the M7 matrix's 27.7 % / 18.2 %, and the D11
checklist's "continuous scroll … ✓ no hitching observed" all measured sixty
property writes a second against a viewport sitting at the top of the page.
They are still what they say they are — a repaint loop with the visible band
invalidated — but they are not a scroll, and the two lines that leaned on them
are now marked in place.

With the direction fixed and the wrap given a second of patience (a ListView
that has not measured its rows yet reports a shorter content height, so the
clamp moves under the scroll; one stalled tick used to mean "bottom reached"),
the scene walked 138 000 – 162 000 px in the sampling window — a 1 000-row page
of pictures, top to bottom and past the wrap, at 58 – 65 % of one core.

**What the real scroll says about the product.** 8 px/frame, a wheel: 36.5 –
38.4 % of one core on 10 000 rows, against the 19.5 – 27.7 % on file, so the
cost of scrolling a long page was under-reported by roughly 1.4×. 200 px/frame,
a hard flick: 83 – 88 % — more than double, and it is a single thread redrawing
the visible band plus whatever the ListView realizes for the new offset. No fix
is claimed here; this is the first honest number for the follow-up that the M7
list ranked second, and that follow-up now has a baseline to argue against.

**What this batch does not measure.** ~~The skia arm of the new scroll numbers~~ —
run the same day, in one sitting with its own femtovg control, in
*M10 · the skia arm of the real scroll* below. A real photo library: the fixtures are
1280×720 gradients, and a flat gradient is the *cheapest* PNG there is to
decode, so every decode figure here is a floor — a phone photo's entropy costs
more per frame, and the 200-fixture pool is 200 names for the same raster
(`create_fixture` does not vary the pixels by id) because the cache is keyed by
path, which is what makes the floor reachable in
a seed pass. The camera-original case (a 12 MP file whose `MAX_EDGE` display
copy is the same 1280-wide raster, so it costs this and buys nothing extra).
A page of pictures on a HiDPI scale, and a page where the pictures are
*between* paragraphs the way a real page has them, rather than on a stride.

## M10 · the skia arm of the real scroll (2026-09-21, ADR-0036 follow-up)

The media batch fixed scene F and re-ran femtovg; that left the renderer verdict
standing on one unproven sentence — since M7, "skia is better at continuous
scroll" has been the only argument the memory case does not win, and the number
behind it came from a viewport that never moved. This is that arm, measured on a
scroll that does.

`benchmarks/scripts/scroll_ab.ps1`: both binaries on disk at once
(`target\release`, and `target-skia\release` from
`--no-default-features --features skia-opengl`), scene outer and arm inner so
machine drift moves both halves of a pair together, and a pre-flight that fails
the batch if either binary's own `first_paint` line reports the wrong renderer —
a skia arm built with the default features still renders through femtovg, and
would otherwise turn this into femtovg against itself. Raw rows:
`benchmarks/results/2026-09-21-m10-scroll-skia.jsonl` (one seed plus two
measured passes per arm; the ranges below are the measured passes).

| scene | femtovg WS / private MB | femtovg CPU % | skia/GL WS / private MB | skia/GL CPU % |
|-------|------------------------:|--------------:|------------------------:|--------------:|
| F 10 000 rows, 8 px/frame (wheel) | 150.4 – 152.0 / 122.2 – 123.4 | 36.9 – 43.1 | 184.7 – 186.6 / 122.4 – 125.2 | 31.8 – 33.7 |
| F 10 000 rows, 200 px/frame (flick) | 156.2 – 158.5 / 135.4 – 136.4 | 86.1 – 87.4 | 190.4 – 191.9 / 128.9 – 130.0 | 66.4 – 70.3 |
| F + 500 pictures, flick | 196.8 – 197.3 / 169.8 – 171.5 | 78.0 – 78.5 | 239.3 – 241.3 / 177.7 – 180.9 | 63.2 – 64.5 |

**The direction survived its instrument; the size of it did not.** skia/GL is
cheaper to scroll on all three arms — ≈18 % less CPU at a wheel tick, ≈21 % at a
flick, ≈18 % on the picture page — but the M7 row claimed 18.2 % against 27.7 %,
a third cheaper, and that gap was two numbers taken off a page sitting still at
the top. What the real scroll costs is the smaller claim.

**And it is bought in exactly the currency the verdict already named.** skia
pays ≈+34 MB of working set on a text page and ≈+43 MB on the picture page for
that CPU. Its *private* bytes are the interesting half: a wash on the wheel arm,
and 6 MB **lower** than femtovg on the flick arm — i.e. the penalty is in
shared, GPU-mapped pages, not in heap the process owns. §二十二's priority order
still resolves this the way it did before: memory outranks everything but UI
quality, so **femtovg stays the default** — but "skia for scroll-heavy use" is
no longer an unproven sentence. It is now about a fifth of the scroll's CPU for
+34…43 MB of working set, and someone on a machine that redraws hot can spend
that knowingly with `--no-default-features --features skia-opengl` — that exact
feature, because it is the arm measured here, not the default-`skia` build whose
first-paint notifier is silent on this driver.

**Two controls worth recording.** The femtovg arm here (150.4 – 152.0 MB,
78.0 – 78.5 % on the picture flick) reproduces the morning batch's independent
run of the same scenes (150.0 – 150.8 MB, 77.4 – 78.6 %) to within a megabyte
and half a percent of CPU, which is what says the two batches are the same
machine on the same day rather than two stories. And the cache line is
renderer-independent: skia reports **9 rasters and `peak_bytes` 33 177 600** too,
the identical ceiling the femtovg arms reached — the LRU is in `AppState`, above
whichever renderer is painting the pixels, so the §三十七 media ceiling does not
move when the renderer does.

**What this arm does not measure.** Startup (measured separately in the skia
first-paint baseline: skia ≤44 ms ahead on an empty shell, a tie on 10 000
blocks), typing, and the idle cost — all three of those already favour femtovg
and none of them were re-run here. skia's WS creep under typing (the M7
follow-up's 320 MB vs 128 MB) is still unexplained and still irrelevant while
femtovg is the default.

## M10 · what one Reclaim click costs (2026-09-21, ADR-0037)

The settings dialog's new button deletes files, so it is the one place in the app
that runs a disk sweep on the UI thread. The number that owes an answer is not
RAM (no block kind, no model field, no per-row state — nothing a bench scene
could see) but the freeze a click causes, and that is measurable without a
window: `a_reclaim_of_a_thousand_orphans_is_timed` (`#[ignore]`, prints one JSON
line) builds a scratch library of 1 000 `attachments` rows with bytes on disk
and no block pointing at any of them, then times the sweep.

Three sittings, one process each, release profile (raw rows:
`benchmarks/results/2026-09-21-m10-reclaim-timing.jsonl`):

| arm | ms |
|-----|---:|
| sweep of 1 000 orphans | 549.9 / 556.7 / 567.6 |
| the same call with nothing to delete | 0.0 |
| (setup: writing the 1 000 files and rows) | 3 934.7 / 4 623.8 / 6 110.4 |

**≈0.55 ms per orphan, and the reachability scan is not on the clock.** An
earlier, independent batch of three sittings read 530.6 / 542.3 / 573.0 ms, so
the spread between batches is about the same as the spread inside one. The
floor arm is a call with an empty book — flush, walk every block of every page,
ask the history what its stacks hold, find nothing — and it reads 0.0 ms, so the
whole 0.55 s belongs to the deletion: one transaction of 1 000 `DELETE`s plus
1 000 `remove_file` calls. Which of those two dominates is not attributed here.
The setup row is the same shape seen from the other side, and it is not this
slice's cost — it is 1 000 separate `apply` calls, each its own transaction,
which is how the test fills a library and is not how the app attaches pictures.

**The control is the row before the number**: the test asserts 1 000 files in
the folder and 1 000 rows in the table before the sweep, and 0 of each after.
Without that, "fast" would be indistinguishable from "there was nothing here",
and a filter that matches no test prints `0 passed` and exits 0 — the first run
of this measurement did exactly that (`--exact` against a bare test name) and
looked like a green result.

**What it means for the button.** A library that a user built up over months and
never reclaimed is on the order of hundreds or low thousands of attachments, so
one click is expected to cost a fraction of a second up to a couple of seconds —
long enough that the notice bar's count is the feedback, short enough that it
does not need a progress UI. A sweep is also idempotent and non-destructive to
anything referenced, so a user who thinks the window has hung and clicks again
loses nothing.

## M11 · math renders at ≈0.6 µs a formula, and the gate finally ran with a control (2026-09-21, ADR-0038)

Two numbers owe an answer for a block kind whose whole job is a string
conversion. One is what the conversion costs; the other is whether adding a 21st
kind, a mark kind and a callback made everything else dearer. The second one is
the measurement 批次 B left unpaid — `columns` compared itself against a baseline
from a different sitting — so this batch built the previous commit in a clean
worktree and ran both arms in one sitting.

**The renderer.** `core::math::tests::cost_per_formula` (`#[ignore]`, prints)
runs five representative sources — a plain script, a greek relation, a fraction
over a radical, an integral with a spacing escape, and an environment that
echoes verbatim — 200 000 times per round, release profile:

| round | ns per formula (mean of the five) |
|-------|----------------------------------:|
| 0 | 658.5 |
| 1 | 625.8 |
| 2 | 608.3 |

**≈0.61 µs per formula.** That is paid once per *run*, at projection time
(`build_runs`), not once per repaint — which is why inline math converts in the
projection instead of in a `Text` binding, and why the block's binding is
guarded by kind (`is-math ? … : ""`): an invisible `Text` still evaluates its
binding, so an unguarded row would run the renderer over every paragraph on the
page. A 10 000-line page whose every line carries one formula therefore costs
≈6 ms per projection — arithmetic from the table above (10 000 × 0.61 µs), not a
measured frame. Nothing on record measures a whole-page projection: the closest
published UI numbers are the typing row's key-handler median (47–66 µs, an
event not a projection) and the storage row's debounced 32-change `apply` median
(3.42 ms with the index, a SQLite commit not a projection), so ≈6 ms is quoted
as a ceiling the renderer could be charged with, not as a cost this build has
been observed to pay. It is the reason there is no plan to render formulas
inside a wrapping line, and the reason the conversion sits in the projection:
a page like that repaints 60 times a second, and a `Text` binding would multiply
this figure by every one of them.

**The gate.** Scene D (10 000 blocks, no formula anywhere in it) with a pinned
database per arm, arms alternating, one sitting (raw rows
`benchmarks/results/2026-09-21-m10-math-ram.jsonl`):

| arm | exe | steady WS MB | steady private MB |
|-----|-----|-------------:|------------------:|
| control `2e9de99` | md5 `57eefe68…`, 21 873 152 B | 135.7 / 135.8 | 109.8 / 110.7 |
| math (this tree) | md5 `5bfeceea…`, 21 929 472 B | 136.6 / 136.7 | 111.7 / 112.4 |

**1.016× the control's private bytes** — inside the ≤1.2× gate, and this time
the ratio is between two builds measured in the same hour on the same machine,
not across a day of drift. Each arm's first run (143.3 / 143.7 WS) is the pass
that *seeds* its database and is excluded from the steady-state pair. The +1.8 MB
private is on a page that contains no formula, no new column and no new per-row
model, and the spread within an arm is 0.9 MB, so the delta is larger than noise
but not attributed: the exe itself grew 56 KB, and the rest is assumed to be the
symbol tables plus one more arm in two `match`es. Recorded rather than explained
away.

**Both arms self-identified, and that is part of the row.** The control came from
`git worktree add` at `2e9de99` with `git status --short` empty and
`src/core/math.rs` absent; the math arm's md5 is the exe this working tree
builds. Two sizes and two md5s are on the line above, so neither arm can be
passed off as the other.

**What this sitting does not settle:** idle CPU read 2.5–4.5 % on both arms here
where the media batch read 0.0–0.2 % on the same scene. Nothing in this slice
adds per-frame work, so the likeliest reading is session state, and it is the
reason the gate is quoted as a ratio between the arms and never as an absolute.

## M11 · a contents block is free at the gate, and the projection it lives in is finally on record (2026-09-21, ADR-0039)

A `Toc` row costs the document one string — `"toc"` in `blocks.kind` — and costs
the UI a list the projection builds on the spot. Two numbers say what that buys
and what it charges.

**The gate, run the way the last batch said to.** Scene D (10 000 blocks, no
contents block anywhere in it), a pinned database per arm, arms alternating, one
sitting, both exes identified by md5 and size first (raw rows
`benchmarks/results/2026-09-21-m11-toc-ram.jsonl`):

| arm | exe | steady WS MB | steady private MB |
|-----|-----|-------------:|------------------:|
| control `cc7ccf0` | md5 `a81ce6df…`, 21 928 960 B | 135.9 / 137.0 | 110.5 / 112.1 |
| toc (this tree) | md5 `f6d9a576…`, 22 017 024 B | 137.0 / 137.3 | 111.6 / 112.5 |

**1.007× the control's private bytes** — and unlike the math batch, the 0.75 MB
gap between the two means is *smaller than the 1.6 MB spread inside the control
arm alone*, so this sitting cannot separate the contents block from noise at
all. That is the honest reading: no measurable cost, not a small one. Each arm's
first run (112.7 / 110.0 private) is the pass that seeds its database and is
excluded. The exe grew 88 064 B for one enum arm, one struct, one callback and
one delegate.

**What the gate cannot see, and so what got measured separately.** Scene D has
no contents block, which makes the RAM ratio a statement about the *kind* and
none about the *list*. `app::state::tests::cost_of_one_contents_block_on_a_ten_thousand_row_page`
(`#[ignore]`, prints, release) times `project_blocks` over 10 000 rows with a
heading every tenth line, once with row 0 as a paragraph and once with it as a
`Toc` listing those 1 000 headings — three rounds, ms per projection:

| round | no contents block | one contents block, 1 000 entries |
|-------|------------------:|----------------------------------:|
| 0 | 39.12 | 37.71 |
| 1 | 37.40 | 37.85 |
| 2 | 37.17 | 38.18 |

The delta is −1.42 / +0.45 / +1.01 ms: **not separable from the run-to-run
spread of the arm that pays it**. The ceiling this sitting can put on 1 000
derived entries is ≈1 ms, against the ≈38 ms the same projection costs without
them. A real page lists a handful of headings, so the walk is some thousands of
times smaller than the noise floor here.

**And that ≈38 ms is the first whole-page projection on record**, which the math
batch had to do without: ADR-0038 quoted ≈6 ms for a formula on every line as
arithmetic precisely because nothing had ever measured the projection those
formulas would land in. Read the two together and the shape of the problem
changes — a per-formula cost is a percentage of a projection, and a projection
is not a frame. This is a Rust-side measurement (blocks in, `BlockRow` values
out, no delegate realized, no repaint, no window), so it says nothing about what
a 10 000-row page costs to *draw* — the published UI numbers stay what they
were: the typing row's key-handler median 47–66 µs and the storage row's
debounced 32-change `apply` median 3.42 ms. What it does settle is that the
contents block's own work is not where the time goes.

## M11 · a link card is below what the gate can resolve (2026-09-21, ADR-0040)

The embed card adds no column, no model field and no per-row allocation: the two
lines it paints are the result of a string function over text the row already
carries. The gate was run to confirm that, not to discover it.

**The gate, third time the same way.** Scene D (10 000 blocks, no embed anywhere
in it), a pinned database per arm, arms alternating in one sitting, both exes
identified by md5 and size before either ran (raw rows
`benchmarks/results/2026-09-21-m11-embed-ram.jsonl`):

| arm | exe | steady WS MB | steady private MB |
|-----|-----|-------------:|------------------:|
| control `4fa2b7b` | md5 `9c5e153f…`, 22 017 024 B | 136.7 / 137.0 | 110.9 / 112.1 |
| embed (this tree) | md5 `19aea07c…`, 22 072 320 B | 138.2 / 138.9 | 111.8 / 112.3 |

**1.005× the control's private bytes.** The 0.55 MB gap between the two means is
less than half the 1.2 MB spread inside the control arm alone, so this sitting
cannot separate the card from noise either — and that is now three consecutive
kinds (math 1.016×, toc 1.007×, embed 1.005×) landing in the same place, which
says something about the instrument as much as about the slices: the gate's floor
is roughly 1 MB of private bytes on this scene, and a change whose whole per-row
cost is two string functions is *under the floor*, not merely small. Each arm's
first run (109.7 / 111.3 private, startup 807 / 1104 ms) is the pass that seeds
its database and is excluded. Idle CPU went the other way — control 4.29–5.85 %,
this tree 3.12–4.09 % — which is the same non-result stated for the counter that
has the widest noise band. The exe grew 55 296 B.

**What the gate cannot see, and why nothing was measured for it instead.** Scene
D has no embed, so `embed-label` and `embed-url` are never called by any run in
the table above. The card's real cost is bounded by construction rather than by a
reading: the two bindings are guarded by `is-embed` (`… ? UIState.embed-label(…)
: ""`), so a row that is not a card evaluates the ternary and does not cross into
Rust — the ADR-0038 lesson, applied to a callback instead of a renderer — and a
row that *is* a card pays it once per repaint of a viewport that holds twenty-odd
rows, not once per row of the document. No timing was taken, because there is no
10 000-row multiplication left to fear: unlike the contents block, which walks
the page once per projection, the card's work has no page in it. The unmeasured
part is the one nothing can measure headless — what the operating system does
with the address after the Open button hands it over.

## M8 · the marked line finally wraps, and the gate had to be built first (2026-09-21, ADR-0041)

The oldest open finding in the project is A4's one HIGH: a paragraph carrying
inline marks lost its word wrap and was clipped mid-word, while the identical
unmarked paragraph wrapped. It is fixed — unmarked stretches are cut to one word
per run and the three surfaces that draw runs lay them out in a wrapping
flexbox — and the measurement half of the slice is mostly about the instrument.

**Scene D could not see this change at all.** The bench page has no marks in it,
so a gate run over it would have reported ≈1.00× for a change whose whole cost
is in marked rows, which is a ratio that only looks like a measurement. So
`--marks N` was added to the harness first: it bolds the second word of every
`rows/N`-th row, and `--dump-state` now prints what the app actually built
(`gs+atlas-blocks=10024 marked=1000`) so each arm identifies its own fixture
instead of taking the bench script's label on faith. Every one of the twelve rows
below carries that line.

**The gate, twice, in one sitting each.** Scene D, 10 000 blocks with 1 000 of
them marked, control = `2c5a25f` from a clean worktree with only the bench knob
ported (so its runs still render on one clipped line), pinned database per arm,
arms alternating, seeding run excluded (raw rows
`benchmarks/results/2026-09-21-m8-wordwrap-ram.jsonl`):

| batch | arm | exe | steady WS MB | steady private MB |
|-------|-----|-----|-------------:|------------------:|
| 1 | control `2c5a25f` | md5 `cdf6da5a…`, 22 074 368 B | 137.4 / 135.7 | 112.7 / 111.4 |
| 1 | this tree | md5 `f59edb1b…`, 22 081 024 B | 139.4 / 140.7 | 115.2 / 115.9 |
| 2 | control (same exe) | md5 `cdf6da5a…` | 138.9 / 137.1 | 114.3 / 111.1 |
| 2 | this tree, all three delegates | md5 `4f9205ca…`, 22 087 680 B | 139.9 / 141.8 | 114.8 / 115.7 |

**1.031× then 1.023×** — inside the ≤1.2× gate both times, and both readings sit
at the instrument's floor: the +2.5…3.5 MB between the two means in each batch is
smaller than the 3.6 MB the *control* arm spread across by itself in batch 2. The
number has a shape though, which is why it is published rather than waved away:
a marked bench row goes from 3 runs to 11 cells, and a cell is a layout item
with its own measured `Text`, so 1 000 rows × 8 extra items is exactly the kind
of cost that should show up as a few MB and not as a doubling. Batch 2 is also
the control for the two delegates scene D never realizes — the table and column
rows joined the change between the batches and moved the reading by nothing
bigger than the noise.

**What the word cut costs the projection, measured on both arms.** An
`#[ignore]`d release timing test, 50 rounds of `project_blocks` over 10 000 rows,
run once per arm in the same sitting:

| arm | 10 000 unmarked rows | same page, 1 000 rows marked | the marks cost |
|-----|---------------------:|-----------------------------:|---------------:|
| control `2c5a25f` | 41.686 ms | 44.147 ms | +2.461 ms |
| this tree | 41.504 ms | 46.942 ms | +5.438 ms |

The unmarked columns agreeing to 0.4 % is the control that both arms saw the same
fixture on the same machine. The word cut is therefore ≈2.8 ms per projection of
a page whose every tenth line carries a mark — ≈2.8 µs per marked line, and
*zero* on a page with no marks anywhere, which is the common case and the reason
the guard stays `block.runs.length > 0` rather than an always-present flex.

**Pixels cost nothing to check and one thing to argue with.** `sweep23` →
`sweep24` (50 scenes): 3 moved, 46 byte-identical, 1 new. `marks` and
`dark-marks` at 1 942 / 1 918 sampled px inside the same box
(`x 390..1148 / y 196..240`). `math-inline` at 360 px inside
`x 420..664 / y 194..210` — a line that already fit, so it should not have
moved; an ink-edge scan put the right-most painted column at 661 before and 664
after, ≈3 px of extra spread from measuring eleven words separately instead of
one string, i.e. sub-pixel per gap. `sweep24` → `sweep25` (52 scenes): 0 of the
50 existing scenes moved and 2 new ones (`table-marks`, `columns-marks`) had to
be added, because the grid and layout fixtures hold no marks and the two
delegate edits were otherwise unverifiable.

## M11 · colouring a code block has no per-row term, and 7 MB this gate will not explain (2026-09-21, ADR-0042)

Five extra `Text` elements per highlighted row, each of them the whole block run
through the lexer. That is the shape the slice had to measure, and scene D has
two thousand of those rows in it.

**Three arms, two binaries, one sitting.** The gate has to separate *this tree*
from the previous commit and *colouring* from *being a code block*, so the third
arm is the candidate exe with the same knob at zero: control `2b62498` built in a
clean worktree, candidate built from this tree, `-Code 0` and `-Code 2000` both
run on the candidate (raw rows
`benchmarks/results/2026-09-21-m11-highlight-ram.jsonl`). Each arm on its own
pinned database, arms alternating, each arm's first (seeding) run excluded, and
every candidate row printing its own fixture (`dump-state: … coloured=2000`) so
an arm cannot pass for another:

| arm | exe | steady WS MB | steady private MB |
|-----|-----|-------------:|------------------:|
| control `2b62498` | md5 `bde7faec…`, 22 087 680 B | 132.2 / 132.2 | 97.4 / 96.9 |
| candidate, no colouring (`coloured=0`) | md5 `961ff115…`, 22 194 176 B | 133.0 / 133.4 | 97.5 / 98.4 |
| candidate, 2 000 coloured rows | same exe | 132.4 / 132.2 | 90.6 / 90.4 |

**1.008× for the tree, and 0.924× once its rows are coloured.** The tree-vs-tree
number is what the slice owes: 0.8 MB against the 0.5 MB spread inside the control
arm alone is the instrument's floor, so the schema column, the `Lang` field on
every block model row and the conditional layer element are **not measurable on
this scene**. The second number is the one that reads like a win, so it is
published with the opposite reading attached rather than as a saving: the same
binary colouring two thousand rows commits *fewer* bytes than the same binary
colouring none.

**What the drop is not, and what is left standing.** It is not the
highlighter's — that is the point of running the third arm on the candidate exe —
and it is not the fixture being smaller, which was this section's first draft and
is wrong: `--code N` swaps a ≈58-byte bench line for a 330-byte, seven-line code
sample, so the coloured arm holds *more* text in *taller* rows than the arm it
came out under. Nothing in the slice's shape explains the sign, and the
readings refuse a slope: +2.75 KB per coloured row at 200, −4.4 KB at 1 800,
−1.85 KB between 2 000 and 5 000. A quantity that changes sign across its own
range is not being measured, so what the gate publishes is the absence of a
per-row term — **25× more coloured rows do not make the process bigger** — and
not the 7.5 MB. That it is committed-not-touched rather than saved is the other
counter: private bytes fall from 97.95 to 90.50 while the **working set stays at
132.2–133.4 on all three arms**, so no page the process actually uses went
away, and the most this sitting supports is that a different allocation sequence
left a smaller arena committed. Idle CPU is 0.2–0.59 % on the steady runs of
every arm; the first run of each (2.53–5.65 %) is the seed pass. The exe grew
106 496 B.

**The typing gate, which is the requirement SPEC §三十七 actually writes down.**
Scene E, 10 000 blocks, ~30 keystrokes/s, one binary, the knob at 0 then 2000,
two rounds each (raw rows
`benchmarks/results/2026-09-21-m11-highlight-typing.jsonl`):

| arm | handler median µs | p95 µs | max µs | n |
|-----|------------------:|-------:|-------:|--:|
| `-Code 0` | 92 / 112 | 160 / 207 | 294 / 559 | 459 / 464 |
| `-Code 2000` | 89 / 89 | 167 / 154 | 421 / 304 | 461 / 464 |

Median 89 µs with two thousand coloured rows against 92–112 µs without them; p95
154–167 against 154–207. The coloured arm is not slower in any column of either
round, and the uncoloured arm's own spread across rounds (92→112) is wider than
the gap between the arms. "长代码块打字延迟不可测" is satisfied — and the
mechanism is the one ADR-0042 records rather than luck: the five lexer calls live
inside a conditional element Slint does not realize while the caret is in the
block, so a keystroke re-renders one plain `Text` and zero lexer runs. What the
coloured arm *does* pay is CPU while typing (29.7–30.8 % against 24.3–27.1 %),
which is the software renderer painting a highlighted row every tick of a scene
with headroom to spare; it is the number to re-read if a future change moves the
layers outside the editing guard.

## M12 · a page's typography changes no row's cost, and the one number it does not have (2026-09-21, ADR-0044)

**No RAM gate ran, and the reason is the shape.** A row read a global property
before this slice and reads a global property after it: `Typography.size-body` was
a constant binding, `PageType.size-body` is that constant times one factor, and the
multiply happens once per evaluation of the global rather than once per row. No
block field, no `DocumentRow` field, no per-row allocation, no new model — so the
≤1.2× private-bytes gate has nothing here in its own units to resolve. The
substitute evidence is the pixel kind, and it is the stronger of the two for a
token change: `sweep31` → `sweep32` moved **1 of 57** scenes, `menu.png` alone at
1 430 px inside the submenu's own box, with the other **56 byte-identical**. A
derivation that had leaked past the page into chrome could not produce that
reading — it would have moved the sidebar in every scene on the set.

**The number this slice does not have**, stated rather than skipped: switching a
page's font on the 10 000-block bench page is untimed. It is a re-layout of the
visible window — every realized row's text is measured again in a new family —
which is the same work a window resize does, and the bench scenes never switch
style on the large page (the five `style-*` arms run on the sample page, whose
rows are a few hundred at most). If a future slice puts a style switch anywhere
on a hot path — per keystroke, per scroll, per page-open on a large document —
that is the moment this owes a measurement, not before.

## M12 · a page's icon costs two small strings per sidebar rebuild, and no bench (2026-09-22, ADR-0045)

**No RAM gate ran, and the reason is the shape.** The stored value is one short
string per page, read twice on the paths that matter: once when a page opens
(`apply_page_style` writes `UIState.page-icon`), and once per sidebar row per
rebuild (`icon_slot` clones the stored emoji, then falls back to the title's
initial). That is two allocations where the row previously made none for the
slot — but the same row already cloned its title into `label`, so the rebuild's
allocation *class* is unchanged and its count grows by a constant factor on a
path that runs on tree mutations, not per frame or per keystroke. The picker is
the one new allocation with a size worth writing down: `fill_icon_picker` copies
96 `&str` out of `core::icon::PICKER` into a `VecModel` of `SharedString` — a few
kilobytes, once per menu activation, dropped when the popup closes.

**The substitute is the pixel kind, and this slice's reading is the unusual one:
64 of 64 scenes moved.** They were supposed to — an iconless tree row now paints
its title's first character where it painted a generic page glyph, so a baseline
that stayed still would mean the feature did not render. What separates that from
"a change leaked everywhere" is where the pixels are: 72 937 px in total, the
modal scene at 1 153 px inside **x 13..52 / y 365..713** (the 16 px slot at depths
0..2), and the same pass restricted to **x 53 → 1 279** returns **0 px for 63 of
the 64 scenes**, `menu.png` alone at 345 px inside x 254..415 / y 723..775 for its
own one-row-taller popup. The two traps this slice actually had were geometry
traps and both show up in that number: putting the emoji in a *second* box beside
a parent's chevron pushed every parent's label one indent past its own children,
and applying the initial to Favorites / Recent replaced the star and the clock —
either one would have read as movers past x 53 in all 64.

## T2 · the backlink panel is one index seek, and the control that says so (2026-09-22, ADR-0051)

SPEC §四十 asks the panel for "all the blocks that reference this page" and
**forbids answering it with a scan**. The panel runs on every projection of a
page, so the honest question is not "how fast is the query" but "does opening a
page cost more because the library is bigger" — and the second half of that
question can only be answered against the counterfactual, which is the read with
migration 16's index taken away.

One database, one sitting, release: 1 200 pages, 1 200 blocks, **100 200 marks**,
of which exactly **200 are references** to the opened page and the other 100 000
are bold spans on unrelated pages. Seeding took 1.88 s and is not part of any
number below.

`app::state::tests::cost_of_the_backlink_panel_on_a_page_open` (`#[ignore]`,
prints, release):

```
cargo test --release --lib cost_of_the_backlink_panel -- --ignored --nocapture
```

**The reference read** — the pair `refresh_backlinks` makes (the count, then the
window), µs per read over 20 rounds:

| arm | read | µs |
|-----|------|---:|
| A | nothing points at the page (`count(*)` → 0, plus a window that the app skips) | 28.3 |
| B | 200 references, folded window (5 rows) | 78.2 |
| D | 200 references, unfolded window (50 rows) | 113.9 |
| **C** | **B with `idx_marks_reference` dropped — not a shipped configuration** | **6 067.9** |

**C is 78× B**, on a table that holds 1 200 rows more than the reference count
needs, and a single count on the same unindexed table is **2 567 µs** against a
whole folded read's 78. That is the "no full scan" sentence turned into a
measurement: the cost of this read follows the *index*, and the 100 200 marks
that are not references are invisible to it. Arm A always calls both queries
where the app calls one (it skips the window when the count is zero), so 28.3 µs
is an upper bound on what a page nobody quotes pays.

**The open** — the same call the UI makes, ms per open over 20 rounds:

| arm | page opened | ms/open |
|-----|-------------|--------:|
| A | nothing points at it | 0.35 |
| B | 200 references, folded | 0.41 |
| D | 200 references, unfolded | 0.43 |

The panel's own share is **+0.06 ms folded, +0.07 ms unfolded** — against the
SPEC §二十二 budget of 50 ms for a page open, and against the ≈38 ms a
10 000-row projection of the same page costs (M11 · ADR-0039), which is where
this slice's time would go if it had one to spend. The window is what keeps it
there: 200 references cost 5 rows of drawing, not 200.

**What this does not measure**, stated rather than implied: the open arms are on
a one-block page, so they isolate the panel and say nothing about a large page's
projection (that number belongs to ADR-0039 and is quoted, not re-measured); the
`count(*)` in every arm is the *unfiltered* count for one page, and nothing here
covers a filter over references. The pixel evidence for the drawing itself is the
T2 sweep (`backlinks`, `backlinks-open`, `backlinks-small`, `dangling` and their
`dark-` arms): **67 of 67 pre-existing scenes byte-identical**, 10 new.

## M14 · a database view costs its window, not its table — the three numbers §三十九 owed (2026-09-22, Track 3 D8, ADR-0060…0087)

SPEC §三十九 closes with 「数字进 docs/PERFORMANCE.md：10 000 行的 RAM、切换
视图耗时、打开公式编辑器的耗时」, and D0 left that line open with an honest
caveat: the projection and the SQL were proven, but 「没有 UI 臂就进不了
PERFORMANCE.md」. D8 does not add a mechanism — it measures the shapes D1–D7
already shipped, at the store and core layer, where the size-dependent cost
lives. Three `#[ignore]`, prints, release probes; the two new ones are
`storage::database_store::probe::a_view_switch_...` and
`core::database_formula::perf::a_formula_...`. Raw rows in
`benchmarks/results/2026-09-22-track3-d8.jsonl` (plus D1's and D4's files for
the window/sort arms). Every number below is a **warm** range over repeated runs
of one sitting; the first (seeding) run of each is excluded, per this file's own
method note.

The harness for all of them:

```
cargo test --release --lib -- --ignored --nocapture
```

**Number 1 — 10 000 行的 RAM.** The window is the only thing that holds row
objects, and it is bounded by the viewport, not the table. The counting allocator
(D0's, `core::database::probe`) is deterministic across runs:

| shape (10 000 records) | realized rows | heap held |
|---|---:|---:|
| window at the top, 3 text cols | 31 | 6 806 B |
| window, 5 typed cols (D1) | 31 | 5 576 B |
| window, 2 cols sorted (D4) | 31 | 3 782 B |
| **materialise the whole table** (control, never done) | 10 000 | 1.16 – 2.26 MB |

The margin is **≈330×**, and it does not move with the table: the same
geometry gives the same `0..31` window at 100 rows and at 1 000 000. Process
readings (working set / private, the same `K32GetProcessMemoryInfo` ADR-0025
uses) are **not load-bearing here** — they ran 4–14 MB private across runs and
are scheduler- and allocator-cache-dominated; the heap ratio is the number that
repeats. So the answer to 「10 000 行的库占多少内存」 is: **a few kilobytes of
row objects, the same as a 100-row library's**, and only the forbidden
whole-table fetch costs megabytes.

**Number 2 — 切换视图耗时.** A switch at the store is: decode the view's JSON
document, `COUNT(*)` over the filtered set, then fetch the window. Warm, over
10 000 rows:

| term | µs / ms |
|---|---:|
| definition decode (`ViewDefinition::parse`) | 2.6 – 7.6 µs |
| row-shaped switch (decode + count + 31-row window fetch) | 0.35 – 1.0 ms |
| grouped switch adds the `GROUP BY` tallies (board / calendar / chart) | +5.5 – 18.1 ms |
| control — a switch that fetched all 10 000 rows | 14.3 – 35.9 ms |

The honest wrinkle D8 put a number on is the same one D4 §4.1 named
qualitatively: **`OFFSET` walks the rows it skips**, so a window read is not one
constant cost. Reading the *bottom* `0..31` of 10 000 rows by offset costs
**16 – 25 ms** (D1), and a *sorted* bottom window **18 – 52 ms** (D4), because
both are dominated by the skip, not the slice. The probe isolates it: the same
bottom window as a bare index walk (no joins) is **30 – 44 µs**, and the same
window asked for by **cursor** (one key, then a range scan) is **212 – 356 µs**
— ~80× under the offset read. A switch to the *top* of a view (what opening one
actually does) is the sub-millisecond row above; only a jump-scroll to the far
end pays the offset walk, and the keyset fetch is the measured way to retire it.

**Number 3 — 打开公式编辑器耗时.** Opening the editor parses the stored
expression once and evaluates it on the sample row once — pure `core::database_formula`,
no I/O:

| term | ns / µs |
|---|---:|
| parse a representative nested formula (`if([Done], [Points]*2, [Points]+length("pending"))-1`) | 1.6 – 3.4 µs |
| one evaluation (one painted cell) | 68 – 129 ns |
| recompute a visible formula column across the whole window (31 rows) | 2.1 – 4.0 µs |
| three visible formula columns, whole window | 6.3 – 12.0 µs |
| **forbidden shape** — recompute 10 000 rows | 0.68 – 1.29 ms |

So 「打开公式编辑器」 is **single-digit microseconds, dominated by the parse**, and
「输入一个单元格后重算多少行」 is answered by the arithmetic the probe prints:
the whole 10 000-row recompute is **108× the window's** recompute, and the
projection has no path that walks it (ADR-0083). The eval-time depth and step
budgets (ADR-0082) are what keep even the forbidden number finite.

**What this does not measure**, stated rather than implied: these are the
*data* costs of a switch and an open, not the frame. Slint's repaint of 31
delegates, the calendar's 42 cells, the chart's path build are not sampled —
that needs the real window and `bench.ps1`'s RAM/pixel arms, which no headless
probe here can stand in for (the same limit D0 recorded). The `cell_write`
figure D4 prints (**9 – 18 ms** for one cell in its own transaction, **17 – 33
µs** for the same write batched into a 500-change transaction) is the commit
durability floor, not a query cost, and the app batches every user edit into one
transaction (one Ctrl+Z), so it is the batched number the user waits on. The
grouped-switch `GROUP BY` over 10 000 rows (5 – 18 ms) is real and grows with the
table — it is the one per-refresh term here that is *not* window-bounded, and it
is the input to whichever view the user is switching to, not to scrolling.

