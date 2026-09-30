# Reclaim build disk *without* giving up incrementality (ADR-0134).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\sweep.ps1
#   just sweep
#
# The problem this exists for. `target` reached 71 GB, and none of that was the
# app. It was cargo's bookkeeping:
#
#   * `target/debug/incremental` — 7.5 GB. One directory per compilation unit
#     per session, and a unit's identity changes whenever its feature set
#     changes. `just run` builds `femtovg`, `just shot` builds `software`, and
#     alternating between them mints a new unit each time, so the count only
#     ever went up.
#   * `target/debug/build` — 6.7 GB. `build.rs` compiles the whole Slint tree, so
#     every copy of `build_script_build.exe` is 29 MB and its two pdbs are
#     96 MB each. 15 of them was 3.7 GB of the same program.
#
# What is deliberately NOT deleted: `deps/` (the rlibs and pdbs a link needs)
# and `.fingerprint/`. Those are the difference between a rebuild that takes
# seconds and one that takes `codegen-units = 1` minutes — which is the entire
# reason the release tree was worth keeping in the first place. `cargo clean`
# would delete them too, and then the next `just deploy-workshop` pays for a
# cold build of a ~1 GB crate. So this is the cheap, repeatable half of
# `just clean`, not a replacement for it.
#
# Safe to run mid-session: it holds no build lock, and cargo rebuilds anything
# it needs. It is *not* safe to run during a build, so the check below refuses
# rather than pulling files out from under a running rustc.

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo    = Split-Path -Parent (Split-Path -Parent $PSCommandPath)
# The pipe leads each continuation line, never trailing: Windows PowerShell 5.1
# (what `just` invokes) rejects a pipeline that starts on the line *after* an
# array literal.
$targets = @('target', 'target-shot', 'target-skia', 'target-wgpu') |
           ForEach-Object { Join-Path $repo $_ } |
           Where-Object { Test-Path $_ }

if (-not $targets) {
    Write-Host 'nothing to sweep: no target directory in this repo'
    exit 0
}

# A live cargo holds the build directory; deleting under it fails in ways that
# read like a permissions problem rather than "a build is running".
#
# `cargo` is the process that actually owns the lock, so it alone is the test.
# `rustc` is deliberately NOT part of it: a killed build leaves a rustc behind
# that has lost its parent, has no image path, and cannot be terminated even by
# a force kill — one has been sitting on this machine since 15:18 doing nothing.
# Waiting on a process list that includes it means the recipe never runs. The
# rustc children of a *live* cargo are already covered by the cargo check, so
# nothing is lost by looking only at cargo.
$busy = Get-Process -Name cargo -ErrorAction SilentlyContinue
if ($busy) {
    Write-Host "a build is running (cargo pid $($busy.Id -join ', ')); sweep it later" -ForegroundColor Yellow
    exit 1
}

function Get-Size([string]$path) {
    if (-not (Test-Path $path)) { return 0L }
    $sum = (Get-ChildItem $path -Recurse -Force -File -ErrorAction SilentlyContinue |
            Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return 0L }
    return [int64]$sum
}

function Format-GB([int64]$bytes) { '{0,7:N2} GB' -f ($bytes / 1GB) }

# Only the caches: the unit-keyed directories with no value to the next build.
$patterns = @('incremental', 'build')

$before = 0L
$freed  = 0L
$rows   = @()

foreach ($root in $targets) {
    $rootBefore = Get-Size $root
    $before    += $rootBefore

    $victims = @()
    foreach ($p in $patterns) {
        $victims += Get-ChildItem -Path $root -Recurse -Force -Directory -Filter $p `
                        -ErrorAction SilentlyContinue
    }
    # A parent's own `build` cache contains its children's; taking both would
    # count the same bytes twice in the report, and the second delete is a no-op.
    $victims = $victims | Sort-Object { $_.FullName.Length }
    $kept    = New-Object System.Collections.Generic.HashSet[string]
    foreach ($v in $victims) {
        $insideKept = $false
        foreach ($k in $kept) {
            if ($v.FullName.StartsWith($k + [IO.Path]::DirectorySeparatorChar)) { $insideKept = $true; break }
        }
        if ($insideKept) { continue }
        [void]$kept.Add($v.FullName)
        $size = Get-Size $v.FullName
        try {
            Remove-Item -Recurse -Force -LiteralPath $v.FullName -ErrorAction Stop
            $freed += $size
            $rows  += [pscustomobject]@{
                Dir   = $v.FullName.Substring($repo.Length + 1)
                Freed = Format-GB $size
            }
        }
        catch {
            Write-Host ("  skipped {0}: {1}" -f $v.FullName, $_.Exception.Message) -ForegroundColor DarkGray
        }
    }

    $rootAfter = Get-Size $root
    $rows     += [pscustomobject]@{ Dir = "$($root.Substring($repo.Length + 1)) (total)"; Freed = Format-GB ($rootBefore - $rootAfter) }
}

Write-Host ''
Write-Host 'swept:'
$rows | Sort-Object { $_.Dir } | Format-Table -AutoSize | Out-String -Width 200 | Write-Host
Write-Host ("before {0}   after {1}   freed {2}" -f (Format-GB $before), (Format-GB ($before - $freed)), (Format-GB $freed)) -ForegroundColor Green
Write-Host 'kept deps/ and .fingerprint/, so the next build links instead of recompiling'
