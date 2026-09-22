param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Profile,
    [string]$Tag = "p",
    [int]$Seconds = 8,
    [string]$Out = ""
)
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "redact.ps1")

# One [profile.release] audit pass: the runtime-shaped scenes only —
#   A  empty shell      · D10000  10 000 blocks idle
#   E10000 typing 30/s with a search every 100 strokes (docs/PERFORMANCE.md)
# bench.ps1's startup_ms is window-up, so every idle scene runs twice against
# the same pinned database (a seed pass, then the measured pass) and the warm
# second pass is the number the profile comparison uses.
# `$plan`, not `$passes`: `$Passes` is a read-only automatic variable.
$runPlan = @(
    @{ label = "$Tag-A"; command = ""; args = @{} },
    @{ label = "$Tag-D10000"; command = "--blocks 10000"; args = @{ Blocks = 10000 } },
    @{ label = "$Tag-E10000"; command = "typing 10000 blocks, 30/s, search every 100"; args = @{ Typing = $true; Blocks = 10000; Rate = 30; SearchEvery = 100 } }
)

$root = Join-Path $env:TEMP "quire-profile"
if (-not (Test-Path $root)) { New-Item -ItemType Directory -Path $root | Out-Null }
$bench = Join-Path $PSScriptRoot "bench.ps1"
$typingExe = Join-Path (Split-Path $Exe -Parent) "quire-typing.exe"

$exeItem = Get-Item $Exe
"{`"kind`":`"meta`",`"profile`":`"$Profile`",`"exe`":`"$((Redact-MachinePath $Exe) -replace '\\','\\')`",`"exe_bytes`":$($exeItem.Length),`"exe_mtime`":`"$($exeItem.LastWriteTime.ToString('s'))`"}"

foreach ($scene in $runPlan) {
    if ($scene.args.ContainsKey("Typing")) {
        # scene E is the harness binary, and it pins its own fresh file
        $plan = @(@{ Label = $scene.label; TypingExe = $typingExe })
    } else {
        $db = Join-Path $root "$($scene.label).db"
        foreach ($stale in @($db, "$db-wal", "$db-shm", "$db.bak1", "$db.bak2", "$db.bak3")) {
            if (Test-Path $stale) { Remove-Item $stale -Force }
        }
        $plan = @(
            @{ Label = "$($scene.label)-seed"; PinnedDb = $db },
            @{ Label = $scene.label; PinnedDb = $db }
        )
    }
    foreach ($step in $plan) {
        $benchArgs = @{ Exe = $Exe; IdleSeconds = $Seconds } + $scene.args + $step
        # hashtable addition refuses a repeated key, so the harness swap goes
        # through an assignment
        if ($step.ContainsKey("TypingExe")) { $benchArgs.Exe = $step.TypingExe }
        $benchArgs.Remove("TypingExe") | Out-Null
        $line = & $bench @benchArgs | ConvertFrom-Json
        $line | Add-Member -NotePropertyName command -NotePropertyValue $scene.command
        $line | Add-Member -NotePropertyName profile -NotePropertyValue $Profile
        $line | Add-Member -NotePropertyName exe_bytes -NotePropertyValue $exeItem.Length
        $text = Open-JsonPlaceholders ($line | ConvertTo-Json -Depth 6 -Compress)
        Write-Output $text
        if ($Out -ne "") { Add-Content -Path $Out -Value $text }
    }
}
