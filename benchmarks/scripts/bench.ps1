param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [int]$Blocks = 0,
    [int]$IdleSeconds = 8,
    [string]$Label = "run",
    [switch]$Scroll,          # scene F: programmatic continuous scroll
    [int]$PageSwitch = 0,     # scene G: switch between N bench pages
    [switch]$Typing,          # scene E: run quire-typing against $Exe
    [double]$Rate = 30,       # scene E: keystrokes per second
    [int]$SearchEvery = 0     # scene E: one full-text query every N strokes
)
$ErrorActionPreference = "SilentlyContinue"

# Scene E types into the real window; the binary prints one JSON report line,
# which we capture to a file and inline below. The typing window is padded so
# it still runs for the whole sample interval (startup + 2 s lead-in).
$reportFile = ""
$dbFile = ""
if ($Typing) {
    # Type against a real file database: the persistence flush, the WAL write
    # and the FTS5 index update are part of the scene. Fresh each run, so the
    # startup load sees exactly the page the app seeds.
    $dbFile = Join-Path $env:TEMP "quire-typing-$Label.db"
    foreach ($stale in @($dbFile, "$dbFile-wal", "$dbFile-shm")) {
        if (Test-Path $stale) { Remove-Item $stale }
    }
    $childArgs = @("--blocks", "$Blocks", "--rate", "$Rate", "--duration", "$($IdleSeconds + 3)", "--db", "$dbFile")
    if ($SearchEvery -gt 0) { $childArgs += @("--search-every", "$SearchEvery") }
    $reportFile = Join-Path $env:TEMP "quire-typing-$Label.json"
    if (Test-Path $reportFile) { Remove-Item $reportFile }
} else {
    $childArgs = @("--auto-exit", "$($IdleSeconds + 6)")
    if ($Blocks -gt 0) { $childArgs += @("--blocks", "$Blocks") }
    if ($Scroll) { $childArgs += @("--scroll") }
    if ($PageSwitch -gt 0) { $childArgs += @("--page-switch", "$PageSwitch") }
}

$sw = [System.Diagnostics.Stopwatch]::StartNew()
if ($reportFile -ne "") {
    $p = Start-Process -FilePath $Exe -ArgumentList $childArgs -PassThru -RedirectStandardOutput $reportFile
} else {
    $p = Start-Process -FilePath $Exe -ArgumentList $childArgs -PassThru
}
# without event raising the .NET Process object never caches ExitCode (PS 5.1)
$p.EnableRaisingEvents = $true

# startup: until the main window handle appears
$h = [IntPtr]::Zero
while ($h -eq [IntPtr]::Zero -and $sw.Elapsed.TotalSeconds -lt 15) {
    Start-Sleep -Milliseconds 20
    $proc = Get-Process -Id $p.Id
    if ($proc) { $h = $proc.MainWindowHandle }
}
$startupMs = [math]::Round($sw.Elapsed.TotalMilliseconds)

# idle: CPU time delta over the idle window
Start-Sleep -Seconds 2
$p.Refresh(); $cpu0 = $p.TotalProcessorTime; $ws0 = $p.WorkingSet64
$sampleStart = Get-Date
Start-Sleep -Seconds $IdleSeconds
$p.Refresh(); $cpu1 = $p.TotalProcessorTime
$wall = ((Get-Date) - $sampleStart).TotalSeconds
# percent of ONE core (no divisor for logical cores)
$cpuPct = [math]::Round((($cpu1 - $cpu0).TotalMilliseconds / 1000) / $wall * 100, 2)
$ramMB = [math]::Round($p.WorkingSet64 / 1MB, 1)
$privMB = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)

$p.WaitForExit()
$ec = $p.ExitCode
$typingReport = "null"
if ($reportFile -ne "" -and (Test-Path $reportFile)) {
    $line = (Get-Content $reportFile | Where-Object { $_ -like '{*"scene"*' } | Select-Object -Last 1)
    if ($line) { $typingReport = $line.Trim() }
}
"{`"label`":`"$Label`",`"exe`":`"$Exe`",`"blocks`":$Blocks,`"startup_ms`":$startupMs,`"idle_cpu_pct`":$cpuPct,`"ram_workingset_mb`":$ramMB,`"ram_private_mb`":$privMB,`"exit_code`":$ec,`"typing`":$typingReport}"
