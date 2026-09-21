# One sitting, both renderers, the two arms of a scene back to back — machine
# drift then moves both of a pair together, which is the only way a cross-batch
# CPU number means anything (docs/PERFORMANCE.md, method finding).
#
# Scene F was fixed on 2026-09-21 (it had been adding to a viewport offset Slint
# measures negative, so it never moved), and the femtovg side was re-run then.
# The skia side of the same three arms is what this script produces: it is the
# one arm the renderer verdict leans on, and the verdict's skia half has been
# standing on a scroll that did not scroll since M2.
#
# Pre-flight, in both directions: each binary has to report the renderer it is
# supposed to be. A skia arm built without --no-default-features renders (and
# labels itself) femtovg, which would silently turn this into femtovg-vs-itself.
# The skia arm is built separately so neither build invalidates the other:
#   cargo build --release --no-default-features --features skia-opengl --target-dir target-skia
param(
    [int]$IdleSeconds = 8,
    [int]$Passes = 2,                     # measured passes per arm (plus one seed)
    [string]$Out = (Join-Path $env:TEMP "quire-scroll-ab.jsonl")
)
$ErrorActionPreference = "Stop"
$root = $PSScriptRoot | Split-Path | Split-Path
$bench = Join-Path $root "benchmarks\scripts\bench.ps1"
$matrix = Join-Path $env:TEMP "quire-scrollab"
if (-not (Test-Path $matrix)) { New-Item -ItemType Directory -Path $matrix | Out-Null }

$arms = @(
    @{ label = "vg";   exe = (Join-Path $root "target\release\quire.exe");          want = "femtovg" },
    @{ label = "skgl"; exe = (Join-Path $root "target-skia\release\quire.exe");     want = "skia" }
)

foreach ($arm in $arms) {
    if (-not (Test-Path $arm.exe)) { throw "missing arm exe: $($arm.exe)" }
    $db = Join-Path $env:TEMP "quire-scrollab-preflight-$($arm.label).db"
    $err = Join-Path $env:TEMP "quire-scrollab-preflight-$($arm.label).err"
    Remove-Item $db, "$db-wal", "$db-shm", $err -ErrorAction SilentlyContinue
    # stderr to a file, as the harness does: piping 2>&1 into the pipeline hands
    # back ErrorRecords under Stop, not lines.
    $p = Start-Process -FilePath $arm.exe `
        -ArgumentList @("--measure-startup", "--auto-exit", "3", "--db", $db) `
        -PassThru -RedirectStandardError $err
    if (-not $p.WaitForExit(30000)) { $p.Kill(); throw "pre-flight: arm $($arm.label) hung" }
    $line = Get-Content $err -ErrorAction SilentlyContinue |
        Where-Object { $_ -like '{"event":"first_paint"*' } | Select-Object -Last 1
    Remove-Item $db, "$db-wal", "$db-shm", $err -ErrorAction SilentlyContinue
    if (-not $line) { throw "pre-flight: arm $($arm.label) recorded no first_paint line — a silent run is not a negative result" }
    $got = ($line | ConvertFrom-Json).renderer
    if ($got -ne $arm.want) { throw "pre-flight: arm $($arm.label) reports '$got', expected '$($arm.want)' — wrong build?" }
    "pre-flight ok: $($arm.label) -> $got"
}

$scenes = @(
    @{ name = "F10000";          args = @{ Blocks = 10000; Scroll = $true } },
    @{ name = "F10000-S200";    args = @{ Blocks = 10000; Scroll = $true; ScrollStep = 200 } },
    @{ name = "F10000-P500";    args = @{ Blocks = 10000; Scroll = $true; ScrollStep = 200; Pictures = 500 } }
)

if (Test-Path $Out) { Remove-Item $Out }
foreach ($scene in $scenes) {
    $att = Join-Path $matrix "attachments"
    foreach ($arm in $arms) {
        # One seed pass writes the page (and, for a media scene, the fixture
        # pool), then $Passes measured passes load it. The folder is cleared per
        # arm, not per scene, so the skia arm never measures femtovg's warm-up.
        if (Test-Path $att) { Remove-Item -Path $att -Recurse -Force }
        $db = Join-Path $matrix "$($scene.name)-$($arm.label).db"
        foreach ($stale in @($db, "$db-wal", "$db-shm", "$db.bak1", "$db.bak2", "$db.bak3")) {
            if (Test-Path $stale) { Remove-Item $stale -Force }
        }
        $base = @{ Exe = $arm.exe; Label = "$($scene.name)-$($arm.label)-seed"; PinnedDb = $db; IdleSeconds = $IdleSeconds } + $scene.args
        & $bench @base | Add-Content -Path $Out
        for ($i = 1; $i -le $Passes; $i++) {
            $run = @{ Exe = $arm.exe; Label = "$($scene.name)-$($arm.label)-$i"; PinnedDb = $db; IdleSeconds = $IdleSeconds } + $scene.args
            & $bench @run | Add-Content -Path $Out
        }
        "done: $($scene.name) / $($arm.label)"
    }
}
"rows: $Out"
