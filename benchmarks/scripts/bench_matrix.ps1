param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$Tag = "run",
    [string]$Out = "",
    [string]$Root = (Join-Path $env:TEMP "quire-matrix"),
    # scene E is a different executable: the typing harness opens its own
    # window and drives it, so it has to be built from the same renderer
    [string]$TypingExe = "",
    # rerun a subset without repeating the rest, e.g. -Only E1000,E10000-120
    [string[]]$Only = @()
)
$ErrorActionPreference = "Stop"

if ($TypingExe -eq "") {
    $TypingExe = Join-Path (Split-Path $Exe -Parent) "quire-typing.exe"
}

# One script per renderer, one JSON line per scene, in the order
# docs/PERFORMANCE.md records them. Every row carries the `command` and the
# `db` it ran with, so a number can be re-derived from the line that holds it.
#   A empty shell · B/C/D 100 / 1 000 / 5 000 / 10 000 blocks
#   F continuous scroll · G 100-page switching
#   E typing (30 strokes/s, plus the 120/s and no-search comparisons)
#
# Idle scenes run twice against the same pinned database file: the first pass
# starts from nothing, so it is the run that *writes* the mock session, and the
# second is the one that measures a session that loads it. Only `-seed` rows
# are disposable; the labelled row is the measurement. Scene E pins its own
# fresh file inside bench.ps1, is a single pass, and is run by `quire-typing`
# rather than the app binary — its `--blocks` build the session it types into.
$scenes = @(
    @{ label = "$Tag-A"; command = ""; args = @{} },
    @{ label = "$Tag-B100"; command = "--blocks 100"; args = @{ Blocks = 100 } },
    @{ label = "$Tag-B1000"; command = "--blocks 1000"; args = @{ Blocks = 1000 } },
    @{ label = "$Tag-C5000"; command = "--blocks 5000"; args = @{ Blocks = 5000 } },
    @{ label = "$Tag-D10000"; command = "--blocks 10000"; args = @{ Blocks = 10000 } },
    @{ label = "$Tag-F10000"; command = "--blocks 10000 --scroll"; args = @{ Blocks = 10000; Scroll = $true } },
    @{ label = "$Tag-G100"; command = "--page-switch 100"; args = @{ PageSwitch = 100 } },
    @{ label = "$Tag-E1000"; command = "typing 1000 blocks, 30/s, search every 100"; args = @{ Typing = $true; Blocks = 1000; Rate = 30; SearchEvery = 100 } },
    @{ label = "$Tag-E10000"; command = "typing 10000 blocks, 30/s, search every 100"; args = @{ Typing = $true; Blocks = 10000; Rate = 30; SearchEvery = 100 } },
    @{ label = "$Tag-E10000-nosearch"; command = "typing 10000 blocks, 30/s"; args = @{ Typing = $true; Blocks = 10000; Rate = 30 } },
    @{ label = "$Tag-E10000-120"; command = "typing 10000 blocks, 120/s, search every 100"; args = @{ Typing = $true; Blocks = 10000; Rate = 120; SearchEvery = 100 } }
)

function Clear-Database([string]$path) {
    foreach ($stale in @($path, "$path-wal", "$path-shm",
        "$path.bak1", "$path.bak2", "$path.bak3", "$path.bak4", "$path.bak5")) {
        if (Test-Path $stale) { Remove-Item $stale -Force }
    }
}

if (-not (Test-Path $Root)) { New-Item -ItemType Directory -Path $Root | Out-Null }

foreach ($scene in $scenes) {
    if ($Only.Count -and -not ($Only | Where-Object { $scene.label -like "*-$_" })) { continue }
    $isTyping = $scene.args.ContainsKey("Typing")
    $base = @{ Exe = $Exe } + $scene.args
    if ($isTyping) {
        # scene E runs the harness binary, not the app: same renderer, and it
        # is the process that owns the window being typed into
        $base.Exe = $TypingExe
        $rows = @($base + @{ Label = $scene.label })
    } else {
        $db = Join-Path $Root "$($scene.label).db"
        Clear-Database $db
        $rows = @(
            ($base + @{ Label = "$($scene.label)-seed"; PinnedDb = $db }),
            ($base + @{ Label = $scene.label; PinnedDb = $db })
        )
    }
    foreach ($params in $rows) {
        $line = & "$PSScriptRoot\bench.ps1" @params | ConvertFrom-Json
        $line | Add-Member -NotePropertyName command -NotePropertyValue $scene.command
        $text = $line | ConvertTo-Json -Depth 6 -Compress
        Write-Output $text
        if ($Out -ne "") { Add-Content -Path $Out -Value $text }
    }
}
