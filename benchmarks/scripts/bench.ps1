param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [int]$Blocks = 0,
    [int]$IdleSeconds = 8,
    [string]$Label = "run",
    [switch]$Scroll,          # scene F: programmatic continuous scroll
    [double]$ScrollStep = 0,  # scene F: px per frame (0 = the app's own 8 px)
    [int]$PageSwitch = 0,     # scene G: switch between N bench pages
    [int]$Pictures = 0,       # scene D/F with media: image rows on the page
    [int]$Marks = 0,          # scene D with marks: rows carrying a bold mark
    # An explicit -Code 0 is a measurement arm too: it asks for the same
    # `--dump-state` line as `-Code 2000` so the two differ by the fixture and
    # by nothing else, which is what makes the pair a control.
    [System.Nullable[int]]$Code = $null, # scene D with highlight: rows turned into coloured code
    [switch]$Typing,          # scene E: run quire-typing against $Exe
    [double]$Rate = 30,       # scene E: keystrokes per second
    [int]$SearchEvery = 0,    # scene E: one full-text query every N strokes
    # idle scenes pin their own file, so a bench run never touches the library
    [string]$PinnedDb = ""
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
    if ($Marks -gt 0) { $childArgs += @("--marks", "$Marks") }
    if ($Code -ne $null) { $childArgs += @("--code", "$Code") }
    $reportFile = Join-Path $env:TEMP "quire-typing-$Label.json"
    if (Test-Path $reportFile) { Remove-Item $reportFile }
} else {
    $childArgs = @("--auto-exit", "$($IdleSeconds + 6)")
    if ($Blocks -gt 0) { $childArgs += @("--blocks", "$Blocks") }
    if ($Scroll) { $childArgs += @("--scroll") }
    if ($ScrollStep -gt 0) { $childArgs += @("--scroll-step", "$ScrollStep") }
    if ($PageSwitch -gt 0) { $childArgs += @("--page-switch", "$PageSwitch") }
    # A media scene has one more number than the process counters can show, so
    # the app prints its decode cache itself and `--dump-state` is the switch
    # that asks for it.
    if ($Pictures -gt 0) { $childArgs += @("--pictures", "$Pictures", "--dump-state") }
    if ($Marks -gt 0) { $childArgs += @("--marks", "$Marks", "--dump-state") }
    if ($Code -ne $null) { $childArgs += @("--code", "$Code", "--dump-state") }
    # A pinned file keeps the run out of the app's real library, and a pinned
    # *empty* file is what makes the second pass a load rather than a seed.
    if ($PinnedDb -ne "") { $childArgs += @("--db", $PinnedDb) }
}

$sw = [System.Diagnostics.Stopwatch]::StartNew()
# The app's own report lines go to stderr; only a media scene asks for one, so
# only a media scene takes the handle.
$cacheFile = ""
if ($Pictures -gt 0 -or $Marks -gt 0 -or $Code -ne $null) {
    $cacheFile = Join-Path $env:TEMP "quire-cache-$Label.txt"
    if (Test-Path $cacheFile) { Remove-Item $cacheFile }
}
$run = @{ FilePath = $Exe; ArgumentList = $childArgs; PassThru = $true }
if ($reportFile -ne "") { $run.RedirectStandardOutput = $reportFile }
if ($cacheFile -ne "") { $run.RedirectStandardError = $cacheFile }
$p = Start-Process @run
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

# A run that never returns is a harness bug, not a measurement; killing it keeps
# the matrix moving and leaves the row with a nonzero exit code.
if (-not $p.WaitForExit(60000)) {
    Write-Warning "$Label did not exit; killing it"
    $p.Kill()
    [void]$p.WaitForExit()
}
$ec = $p.ExitCode
$typingReport = "null"
if ($reportFile -ne "" -and (Test-Path $reportFile)) {
    $line = (Get-Content $reportFile | Where-Object { $_ -like '{*"scene"*' } | Select-Object -Last 1)
    if ($line) { $typingReport = $line.Trim() }
}
$cacheReport = "null"
$dumpState = "null"
if ($cacheFile -ne "") {
    if (Test-Path $cacheFile) {
        $line = (Get-Content $cacheFile | Where-Object { $_ -like '*"event":"attachment_cache"*' } | Select-Object -Last 1)
        if ($line) { $cacheReport = $line.Trim() }
        # what the app says it built, so an arm proves its own fixture instead
        # of taking this script's label on faith
        $dump = (Get-Content $cacheFile | Where-Object { $_ -like 'dump-state:*' } | Select-Object -Last 1)
        if ($dump) { $dumpState = '"' + ($dump.Trim() -replace '\\', '\\') + '"' }
        Remove-Item -Path $cacheFile -Force
    }
}
# Scene E's scratch database and its report are both this script's mess, so
# this script cleans it. Only the `.db` used to be purged, and only before the
# run, so every label left its startup `.bak1` snapshot plus the wal/shm
# siblings behind in `%TEMP%` forever.
if ($dbFile -ne "") { Get-ChildItem -Path "$dbFile*" | Remove-Item -Force }
if ($reportFile -ne "") { Remove-Item -Path $reportFile -Force }
# a Windows path is not a legal JSON string until its backslashes are doubled,
# and `$json` has to survive ConvertFrom-Json for bench_matrix.ps1
$jsonExe = $Exe -replace '\\', '\\'
$jsonLabel = $Label -replace '\\', '\\'
$jsonDb = $PinnedDb
if ($dbFile -ne "") { $jsonDb = $dbFile }
$jsonDb = $jsonDb -replace '\\', '\\'
"{`"label`":`"$jsonLabel`",`"exe`":`"$jsonExe`",`"db`":`"$jsonDb`",`"blocks`":$Blocks,`"startup_ms`":$startupMs,`"idle_cpu_pct`":$cpuPct,`"ram_workingset_mb`":$ramMB,`"ram_private_mb`":$privMB,`"exit_code`":$ec,`"typing`":$typingReport,`"attachment_cache`":$cacheReport,`"dump_state`":$dumpState}"
