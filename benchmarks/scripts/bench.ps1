param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [int]$Blocks = 0,
    [int]$IdleSeconds = 8,
    [string]$Label = "run"
)
$ErrorActionPreference = "SilentlyContinue"

$args = @("--auto-exit", "$($IdleSeconds + 6)")
if ($Blocks -gt 0) { $args += @("--blocks", "$Blocks") }

$sw = [System.Diagnostics.Stopwatch]::StartNew()
$p = Start-Process -FilePath $Exe -ArgumentList $args -PassThru

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
$cpuPct = [math]::Round((($cpu1 - $cpu0).TotalMilliseconds / 1000) / $wall * 100, 2)
$ramMB = [math]::Round($p.WorkingSet64 / 1MB, 1)
$privMB = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)

$p.WaitForExit()
$ec = $p.ExitCode
"{`"label`":`"$Label`",`"exe`":`"$Exe`",`"blocks`":$Blocks,`"startup_ms`":$startupMs,`"idle_cpu_pct`":$cpuPct,`"ram_workingset_mb`":$ramMB,`"ram_private_mb`":$privMB,`"exit_code`":$ec}"
