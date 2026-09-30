# Where the build disk went.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\size.ps1
#   just size
#
# `target` is a number with no intuition attached: "71 GB" says nothing about
# whether it is normal or whether something is leaking. This prints the tree
# largest-first, per profile, so a repeat visit can be compared against the last
# one and the growth attributed to a directory rather than guessed at.
#
# The two columns that matter when it is big again are the *count* of a
# directory's files and how recently it was touched. A `deps` full of many
# same-sized rlibs is the feature-set problem (ADR-0134) — cargo keeps one per
# unit it has ever built — and it grows every time `just run` and `just shot`
# alternate. A `build` full of 250 MB build-script directories is the same leak
# seen from the other end, since `build.rs` compiles the whole Slint tree and
# every copy of `build_script_build` carries its pdbs. The date column is the
# *newest* file, which is when this directory was last written — a `Newest` of
# months ago next to a large size is the signature of a directory nothing
# touches any more.

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
    Write-Host 'no target directory: nothing has been built here yet'
    exit 0
}

function Get-Size([string]$path) {
    if (-not (Test-Path $path)) { return 0L }
    $sum = (Get-ChildItem $path -Recurse -Force -File -ErrorAction SilentlyContinue |
            Measure-Object -Property Length -Sum).Sum
    if ($null -eq $sum) { return 0L }
    return [int64]$sum
}

$grand = 0L
foreach ($root in $targets) {
    $name = $root.Substring($repo.Length + 1)
    $rows = Get-ChildItem $root -Force -Directory -ErrorAction SilentlyContinue | ForEach-Object {
        $files = @(Get-ChildItem $_.FullName -Recurse -Force -File -ErrorAction SilentlyContinue)
        # Under Set-StrictMode a Measure-Object over nothing has no .Sum, and
        # the release tree's `examples` and `tmp` are exactly that case, so the
        # count is taken first and the aggregate is only asked for when it is > 0.
        $size  = 0L
        $newest = $null
        if ($files.Count -gt 0) {
            $size = [int64](($files | Measure-Object -Property Length -Sum).Sum)
            $newest = ($files | Measure-Object -Property LastWriteTime -Maximum).Maximum
        }
        [pscustomobject]@{
            Dir    = $_.Name
            GB     = [math]::Round($size / 1GB, 2)
            Files  = $files.Count
            Newest = if ($null -eq $newest) { '' } else { $newest.ToString('yyyy-MM-dd') }
        }
    }
    $total = Get-Size $root
    $grand += $total
    Write-Host ''
    Write-Host ("{0}  —  {1:N2} GB" -f $name, ($total / 1GB)) -ForegroundColor Cyan
    $rows | Sort-Object GB -Descending | Format-Table -AutoSize | Out-String -Width 200 | Write-Host
}

# The global download cache is a separate 2.8 GB that never used to be pruned;
# `cache.auto-clean-frequency` in .cargo/config.toml is what bounds it now.
$cargoHome = Join-Path $env:USERPROFILE '.cargo'
if (Test-Path $cargoHome) {
    $cache = Get-Size $cargoHome
    Write-Host ("{0}  —  {1:N2} GB  (auto-cleaned, see .cargo/config.toml)" -f '~/.cargo', ($cache / 1GB)) -ForegroundColor Cyan
    $grand += $cache
}

$free = (Get-PSDrive C).Free
Write-Host ''
Write-Host ("total {0:N2} GB   C: free {1:N1} GB" -f ($grand / 1GB), ($free / 1GB)) -ForegroundColor Green
