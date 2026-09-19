param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [int]$Runs = 5,
    [string]$Label = "startup",
    # the long-document scenario: --blocks N on top of the seeded page
    [int]$Blocks = 0,
    [string]$ResultsDir = ""
)
$ErrorActionPreference = "SilentlyContinue"

# A2 · first-paint latency, measured against the harness's old window-up
# number inside the same run.
#
# Under --measure-startup the binary installs a Slint rendering notifier and
# prints one JSON line to stderr the moment the renderer reports its first
# painted frame (`first_paint_ms`, timed inside the process from the start of
# main()). The rest of the stderr traffic — the startup facts — is ignored.
#
# The harness adds its own stopwatch over the same run:
#   window_up_ms    — until the main window handle exists (what startup_ms has
#                     always measured),
#   stderr_paint_ms — until the first_paint line lands in the file: the paint
#                     moment as observed from outside.
# Each run gets its own scratch database, so no run pays another's WAL
# recovery and the real per-user library is never touched.
if ($ResultsDir -eq "") {
    $ResultsDir = Join-Path (Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)) "results"
}
$outFile = Join-Path $ResultsDir "$(Get-Date -Format 'yyyy-MM-dd')-first-paint-$Label.jsonl"
if (Test-Path $outFile) { Remove-Item $outFile }

$jsonExe = ($Exe -replace '\\', '\\') -replace '"', '\"'
$rows = @()
$renderer = ""
$method = ""
for ($i = 1; $i -le $Runs; $i++) {
    $db = Join-Path $env:TEMP "quire-firstpaint-$Label-$i.db"
    foreach ($stale in @($db, "$db-wal", "$db-shm")) { if (Test-Path $stale) { Remove-Item $stale } }
    $errFile = Join-Path $env:TEMP "quire-firstpaint-$Label-$i.err"
    if (Test-Path $errFile) { Remove-Item $errFile }

    $childArgs = @("--measure-startup", "--auto-exit", "4", "--db", $db)
    if ($Blocks -gt 0) { $childArgs += @("--blocks", "$Blocks") }
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $p = Start-Process -FilePath $Exe -ArgumentList $childArgs -PassThru -RedirectStandardError $errFile
    $p.EnableRaisingEvents = $true

    # the old proxy: until the window handle appears
    $handle = [IntPtr]::Zero
    while ($handle -eq [IntPtr]::Zero -and $sw.Elapsed.TotalSeconds -lt 15) {
        Start-Sleep -Milliseconds 20
        $proc = Get-Process -Id $p.Id
        if ($proc) { $handle = $proc.MainWindowHandle }
    }
    $windowUpMs = [math]::Round($sw.Elapsed.TotalMilliseconds)

    # the new number: until the first_paint line is written. The 15 s cap
    # catches a hung harness run; it is never a measurement ceiling.
    $line = $null
    while (-not $line -and $sw.Elapsed.TotalSeconds -lt 15) {
        $line = (Get-Content $errFile | Where-Object { $_ -like '{"event":"first_paint"*' } | Select-Object -Last 1)
        if (-not $line) { Start-Sleep -Milliseconds 20 }
    }
    $stderrPaintMs = [math]::Round($sw.Elapsed.TotalMilliseconds)

    if (-not $p.WaitForExit(60000)) {
        Write-Warning "run $i did not exit; killing it"
        $p.Kill(); [void]$p.WaitForExit()
    }
    Remove-Item $errFile
    foreach ($stale in @($db, "$db-wal", "$db-shm")) { if (Test-Path $stale) { Remove-Item $stale } }
    if (-not $line) {
        Write-Warning "run $i recorded no first_paint line (exit $($p.ExitCode))"
        continue
    }
    $o = $line | ConvertFrom-Json
    $renderer = $o.renderer
    $method = $o.method
    $rows += [pscustomobject]@{
        paint  = [double]$o.first_paint_ms
        window = $windowUpMs
        stderr = $stderrPaintMs
    }
    "{`"label`":`"$Label`",`"run`":$i,`"exe`":`"$jsonExe`",`"blocks`":$Blocks,`"renderer`":`"$renderer`",`"method`":`"$method`",`"first_paint_ms`":$($o.first_paint_ms),`"window_up_ms`":$windowUpMs,`"stderr_paint_ms`":$stderrPaintMs,`"exit_code`":$($p.ExitCode)}" | Add-Content $outFile
}

if ($rows.Count -eq 0) {
    Write-Warning "no runs recorded"
    exit 1
}
function Get-Stat([array]$values) {
    $sorted = @($values | Sort-Object)
    [pscustomobject]@{
        min    = [math]::Round($sorted[0])
        median = [math]::Round($sorted[[int][math]::Floor(($sorted.Count - 1) / 2)])
        max    = [math]::Round($sorted[-1])
    }
}
$pS = Get-Stat @($rows.paint); $wS = Get-Stat @($rows.window); $eS = Get-Stat @($rows.stderr)
$summary = "{`"label`":`"$Label`",`"exe`":`"$jsonExe`",`"runs`":$($rows.Count),`"blocks`":$Blocks,`"renderer`":`"$renderer`",`"method`":`"$method`",`"first_paint_ms_min`":$($pS.min),`"first_paint_ms_median`":$($pS.median),`"first_paint_ms_max`":$($pS.max),`"window_up_ms_min`":$($wS.min),`"window_up_ms_median`":$($wS.median),`"window_up_ms_max`":$($wS.max),`"stderr_paint_ms_min`":$($eS.min),`"stderr_paint_ms_median`":$($eS.median),`"stderr_paint_ms_max`":$($eS.max)}"
$summary | Add-Content $outFile
Write-Host $summary
Write-Host "raw runs: $outFile"
