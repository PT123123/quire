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
#   F continuous scroll · D/F with --pictures N: the same page, photo rows
#   G 100-page switching
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
    # The same scroll at a flick-sized step: a wheel tick and a hard flick are
    # two different loads on the renderer, so both are on the record.
    @{ label = "$Tag-F10000-S200"; command = "--blocks 10000 --scroll --scroll-step 200"; args = @{ Blocks = 10000; Scroll = $true; ScrollStep = 200 } },
    # SPEC §三十七's unmeasured shape: the same page with pictures on it. The
    # pool is capped, so P500 and P5000 differ in how often a row re-enters the
    # viewport, not in how many rasters exist on disk.
    @{ label = "$Tag-D10000-P500"; command = "--blocks 10000 --pictures 500"; args = @{ Blocks = 10000; Pictures = 500 } },
    # A picture row is ~400 px tall and the default 8 px/frame reaches one only
    # after half a second of nothing else, so the scrolled media scenes ask for
    # a flick-sized step: 200 px per frame crosses picture rows inside a
    # sampling window, which is the whole point of the scene.
    @{ label = "$Tag-F10000-P500"; command = "--blocks 10000 --scroll --scroll-step 200 --pictures 500"; args = @{ Blocks = 10000; Scroll = $true; ScrollStep = 200; Pictures = 500 } },
    @{ label = "$Tag-D10000-P5000"; command = "--blocks 10000 --pictures 5000"; args = @{ Blocks = 10000; Pictures = 5000 } },
    @{ label = "$Tag-F10000-P5000"; command = "--blocks 10000 --scroll --scroll-step 200 --pictures 5000"; args = @{ Blocks = 10000; Scroll = $true; ScrollStep = 200; Pictures = 5000 } },
    # A short page of pictures: the same nine rasters in a cache that drains and
    # refills every few frames instead of every few hundred rows. If the ceiling
    # were row-count-driven this arm would move; it does not.
    @{ label = "$Tag-F1000-P500-S200"; command = "--blocks 1000 --scroll --scroll-step 200 --pictures 500"; args = @{ Blocks = 1000; Scroll = $true; ScrollStep = 200; Pictures = 500 } },
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

# `powershell -File script.ps1 -Only A,B` delivers the comma list as ONE string,
# so a multi-token filter used to match nothing and exit 0 with no rows — the
# silent-no-op shape this harness has been bitten by before. Split the tokens,
# and refuse to call a run that matched no scene a success.
$Only = @($Only | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$matched = 0

foreach ($scene in $scenes) {
    if ($Only.Count -and -not ($Only | Where-Object { $scene.label -like "*-$_" })) { continue }
    $matched += 1
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
        # A media scene's fixtures sit beside the pinned databases, so all four
        # share one folder. Clearing it before the seed pass is what keeps that
        # pass a write and the measured pass a load — a pool left from the
        # previous scene would be measured re-using files it never made.
        if ($scene.args.ContainsKey("Pictures")) {
            $att = Join-Path $Root "attachments"
            if (Test-Path $att) { Remove-Item -Path $att -Recurse -Force }
        }
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

if ($matched -eq 0) {
    throw "no scene matched (Only=[$($Only -join ',')]); tags present: $($scenes.Count). A filter that matches nothing is not a measurement."
}
