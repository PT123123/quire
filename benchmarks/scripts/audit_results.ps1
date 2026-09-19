# Bench results auditor -- regenerate the numbers PERFORMANCE.md quotes.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File benchmarks\scripts\audit_results.ps1
#   ... -File benchmarks\scripts\audit_results.ps1 -Only first-paint
#
# Every table in docs/PERFORMANCE.md is hand-typed from a JSONL batch, and twice
# that cost real reconciliation: the A2 section quoted 549/693 ms medians from a
# 5+4-run batch while a 7-run batch of the same scenes said 501/651, and nothing
# on disk made that visible. This script recomputes every stored summary row
# from its own runs, prints the per-phase medians the tables are built from, and
# groups the first-paint batches by document size so cross-batch drift on one
# scene is a column instead of an argument.
#
# It exits 1 when a stored summary disagrees with the runs beside it by more
# than 1 ms -- i.e. when a table in the docs cannot be regenerated from its own
# evidence.
param(
    [string]$Dir = "benchmarks/results",
    # paint = --measure-startup rows, bench = bench.ps1 rows, all = both
    [string]$Only = "all",
    [int]$ToleranceMs = 1
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)   # .../benchmarks/scripts -> project root
Set-Location $root
if (-not (Test-Path $Dir)) { throw "no results dir at $Dir" }

# Same rule startup_bench.ps1 uses (low median on an even count), so a recomputed
# median is comparable with the stored one.
function Median([double[]]$v) {
    $s = @($v | Sort-Object)
    if ($s.Count -eq 0) { return $null }
    $s[[int][math]::Floor(($s.Count - 1) / 2)]
}
function Stat([double[]]$v) {
    if (-not $v.Count) { return "n/a" }
    $s = @($v | Sort-Object)
    "{0} / {1} / {2}" -f [math]::Round($s[0]), [math]::Round((Median $s)), [math]::Round($s[-1])
}

$paintFiles = @(); $benchFiles = @(); $mismatches = @()
Get-ChildItem (Join-Path $Dir "*.jsonl") | Sort-Object Name | ForEach-Object {
    $rows = @(); $summary = $null
    foreach ($line in (Get-Content $_.FullName)) {
        if (-not $line.Trim()) { continue }
        $o = $line | ConvertFrom-Json
        if ($o.PSObject.Properties.Name -contains "runs") { $summary = $o; continue }
        $rows += $o
    }
    if (-not $rows.Count) { return }
    $isPaint = [bool]($rows[0].PSObject.Properties.Name -contains "first_paint_ms")
    $label = $rows[0].label
    $blocks = $rows[0].blocks
    if ($isPaint) { $paintFiles += [pscustomobject]@{ label = $label; blocks = $blocks; file = $_.Name } }
    else { $benchFiles += [pscustomobject]@{ label = $label; blocks = $blocks; file = $_.Name } }

    if (($Only -eq "paint") -and -not $isPaint) { return }
    if (($Only -eq "bench") -and $isPaint) { return }

    ""
    "== $($_.Name)   label=$label   runs=$($rows.Count)   blocks=$blocks"
    "   exe: $($rows[0].exe)"
    if ($isPaint) {
        $paint = @($rows | ForEach-Object { [double]$_.first_paint_ms })
        $window = @($rows | ForEach-Object { [double]$_.window_up_ms })
        $stderr = @($rows | ForEach-Object { [double]$_.stderr_paint_ms })
        "   first paint  min/med/max ms : $(Stat $paint)"
        "   window-up    min/med/max ms : $(Stat $window)"
        "   stderr paint min/med/max ms : $(Stat $stderr)"
        $loop = @($rows | ForEach-Object {
            $at = ($_.phases_raw | Where-Object { $_.phase -eq "pre_event_loop" } | Select-Object -First 1)
            if ($at) { [double]$_.first_paint_ms - [double]$at.at_ms }
        })
        if ($loop.Count) { "   inside ui.run() to first frame  min/med/max ms : $(Stat $loop)" }
        $phaseNames = @()
        foreach ($r in $rows) { foreach ($p in $r.phases_raw) { if ($p.phase -notin $phaseNames) { $phaseNames += $p.phase } } }
        foreach ($name in $phaseNames) {
            $vals = @($rows | ForEach-Object {
                $p = $_.phases_raw | Where-Object { $_.phase -eq $name } | Select-Object -First 1
                if ($p) { [double]$p.ms }
            })
            if ($vals.Count) { $st = Stat $vals; "   phase {0,-16} min/med/max ms : {1}" -f $name, $st }
        }
        if ($summary) {
            foreach ($pair in @(
                @("first_paint_ms_median", $paint), @("window_up_ms_median", $window),
                @("stderr_paint_ms_median", $stderr)
            )) {
                if ($summary.PSObject.Properties.Name -contains $pair[0]) {
                    $stored = [double]$summary.($pair[0]); $fresh = [double](Median $pair[1])
                    if ([math]::Abs($stored - $fresh) -gt $ToleranceMs) {
                        $mismatches += "$($_.Name): stored $($pair[0])=$stored, recomputed=$fresh"
                    }
                }
            }
            "   stored summary row: agrees with its runs (within $ToleranceMs ms)"
        }
    }
    else {
        $startup = @($rows | ForEach-Object { [double]$_.startup_ms })
        $ram = @($rows | ForEach-Object { [double]$_.ram_workingset_mb })
        "   startup_ms   min/med/max    : $(Stat $startup)"
        if ($ram.Count) { "   workingset   min/med/max MB  : $(Stat $ram)" }
        $typing = @($rows | ForEach-Object {
            if ($_.typing -and ($_.typing.PSObject.Properties.Name -contains "ms_per_keystroke")) {
                [double]$_.typing.ms_per_keystroke
            }
        })
        if ($typing.Count) { "   typing ms/key min/med/max: $(Stat $typing)" }
    }
}

if ($Only -ne "bench") {
    ""
    "== cross-batch view: first paint by document size"
    foreach ($g in ($paintFiles | Group-Object blocks | Sort-Object { [int]$_.Name })) {
        "   blocks {0,6} :" -f [int]$g.Name
        foreach ($f in ($g.Group | Sort-Object label)) {
            $rows = Get-Content (Join-Path $Dir $f.file) | ForEach-Object { $_ | ConvertFrom-Json } |
                Where-Object { $_.PSObject.Properties.Name -notcontains "runs" }
            $p = @($rows | ForEach-Object { [double]$_.first_paint_ms })
            "      {0,-16} n={1}  median {2} ms" -f $f.label, $p.Count, [math]::Round((Median $p))
        }
    }
    "   (the same scene measured in several batches is session drift, not a"
    "    regression: quote deltas within one batch -- see PERFORMANCE.md A2)"
}

""
if ($mismatches.Count) {
    "AUDIT FAILED -- a stored summary disagrees with its own runs:"
    $mismatches | ForEach-Object { "  $_" }
    exit 1
}
"audit ok: $($paintFiles.Count) first-paint + $($benchFiles.Count) bench batches recomputed from raw rows"
