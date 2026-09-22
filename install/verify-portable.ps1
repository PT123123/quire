# A1 follow-up · `--portable` end-to-end verification (M8 / D12 escape hatch).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\verify-portable.ps1
#
# `--portable` is the pre-D12 placement: the library stays beside the working
# directory instead of `%APPDATA%\Quire`, which is what a USB-stick install
# wants. A6 verified the installer; it never drove this flag, and the unit
# tests in `storage::data_location` only exercise the pure `decide()` seam. Here
# the real release binary runs, so what is checked is the consequence on disk:
#
#   (1) a portable run puts quire.db *and* quire.log beside the working dir
#   (2) it never creates or writes the per-user folder
#   (3) a second portable run reopens the same library (data, not just files)
#   (4) `--db` beats `--portable`, and the log follows the named file
#   (5) control: with no flag the same harness *does* roam — so a green (2) is
#       not an environment that can never roam
#   (6) a legacy `appdata/` library migrates out on the first roaming run, and
#       its snapshots, sidecars and log travel with it
#   (7) `--portable` suppresses that migration: the stick keeps its own copy
#   (8) with APPDATA absent the app falls back to the portable placement
#
# Every child process gets a redirected APPDATA pointing at a scratch folder,
# so the real per-user library is out of reach by construction — and, because
# that is an assumption about the environment rather than about the code, the
# run also hashes `%APPDATA%\Quire` before and after and fails if it moved.
# No window is screenshotted and nothing is clicked; each launch self-exits.
#
# The scratch root is stamped with the run's own start time and nothing under
# it is ever deleted, so the script is re-runnable at the next head without
# depending on a delete succeeding. That matters here: a guard on this machine
# caps deletes at 50 files per turn, and clearing 8 sticks plus 8 fake AppData
# folders is well past it (see `verify-installer.ps1` for the same fix and the
# observed failure).
param(
    [string]$Exe = "target\release\quire.exe",
    [string]$Scratch = "",
    [int]$ExitAfter = 3
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if ($Scratch -eq "") { $Scratch = ".scratch\portable\run-$(Get-Date -Format 'yyyyMMdd-HHmmss')" }
# The child's APPDATA and working directory must be absolute: a relative
# APPDATA is resolved against the child's cwd, which sends the "per-user"
# library into the stick folder and makes every roaming check vacuously true.
if (-not [System.IO.Path]::IsPathRooted($Scratch)) { $Scratch = Join-Path $root $Scratch }
if (-not (Test-Path $Exe)) {
    throw "no exe at $Exe -- run cargo build --release first"
}
$Exe = (Resolve-Path $Exe).Path

$results = @()
function Check([string]$name, [bool]$ok, [string]$detail) {
    $script:results += [pscustomobject]@{ name = $name; ok = $ok; detail = $detail }
    "{0}  {1}{2}" -f ($(if ($ok) { "PASS" } else { "FAIL" })), $name, $(if ($detail) { "  ($detail)" } else { "" })
}

# A fresh stick (working directory) and a fresh fake per-user folder. The
# scratch root is unique per run and nothing is deleted, so "fresh" is a
# consequence of the path rather than of a successful Remove-Item.
function New-Stick([string]$tag) {
    $d = Join-Path $Scratch "stick-$tag"
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $d
}
function New-FakeAppData([string]$tag) {
    $d = Join-Path $Scratch "appdata-$tag"
    New-Item -ItemType Directory -Force -Path $d | Out-Null
    $d
}

# Launch the real binary with its own working directory and its own APPDATA.
# $AppData = $null removes the variable, which is the "no roaming folder" case.
function Invoke-Quire {
    param([string]$WorkDir, $AppData, [string[]]$ChildArgs)
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $Exe
    $psi.WorkingDirectory = $WorkDir
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardError = $true
    $psi.RedirectStandardOutput = $true
    if ($null -eq $AppData) { [void]$psi.EnvironmentVariables.Remove("APPDATA") }
    else { $psi.EnvironmentVariables["APPDATA"] = $AppData }
    $psi.Arguments = ($ChildArgs | ForEach-Object { '"' + ($_ -replace '"', '') + '"' }) -join ' '
    $p = [System.Diagnostics.Process]::Start($psi)
    $err = $p.StandardError.ReadToEnd()
    $out = $p.StandardOutput.ReadToEnd()
    if (-not $p.WaitForExit(60000)) { $p.Kill(); throw "run did not exit: $($ChildArgs -join ' ')" }
    [pscustomobject]@{ stderr = $err; stdout = $out; code = $p.ExitCode }
}
function Pages([string]$text) {
    if ($text -match 'dump-state: pages=(\d+)') { return [int]$Matches[1] }
    -1
}
function Snapshot([string]$dir) {
    if (-not (Test-Path $dir)) { return "absent" }
    ((Get-ChildItem $dir -File | Sort-Object Name |
        ForEach-Object { "$($_.Name):$((Get-FileHash -Algorithm MD5 -Path $_.FullName).Hash.Substring(0, 8))" }) -join ' ')
}

if (Test-Path $Scratch) { throw "$Scratch already exists; pass a different -Scratch" }
New-Item -ItemType Directory -Force -Path $Scratch | Out-Null

$realLibrary = Join-Path $env:APPDATA "Quire"
$realBefore = Snapshot $realLibrary

# ---- (1)(2)(3) portable placement ------------------------------------------
$stick = New-Stick "p1"
$fake = New-FakeAppData "p1"
$r = Invoke-Quire $stick $fake @("--portable", "--dump-state", "--auto-exit", "$ExitAfter")
$db = Join-Path $stick "appdata\quire.db"
$log = Join-Path $stick "appdata\quire.log"
Check "portable db beside the working dir" (Test-Path $db) $db
Check "portable log beside the db" (Test-Path $log) (Split-Path -Leaf $log)
Check "portable never makes the per-user folder" (-not (Test-Path (Join-Path $fake "Quire"))) $fake
Check "portable seeded a library" ((Pages $r.stderr) -gt 0) "pages=$((Pages $r.stderr))"

# reopen: same stick, a *different* fake APPDATA (so a roam would be visible)
$fake2 = New-FakeAppData "p2"
$r = Invoke-Quire $stick $fake2 @("--portable", "--dump-state", "--auto-exit", "$ExitAfter")
Check "portable reopens the library it wrote" ((Test-Path $db) -and ((Pages $r.stderr) -gt 0)) "pages=$((Pages $r.stderr))"
Check "portable rerun still roams nowhere" (-not (Test-Path (Join-Path $fake2 "Quire"))) $fake2

# ---- (4) --db wins over --portable -----------------------------------------
$stick = New-Stick "db"
$fake = New-FakeAppData "db"
$named = Join-Path $stick "elsewhere\named.db"
$r = Invoke-Quire $stick $fake @("--portable", "--db", $named, "--dump-state", "--auto-exit", "$ExitAfter")
Check "--db beats --portable" (Test-Path $named) $named
Check "--db leaves no appdata/ beside the cwd" (-not (Test-Path (Join-Path $stick "appdata"))) $stick
Check "--db keeps the per-user folder empty" (-not (Test-Path (Join-Path $fake "Quire"))) $fake
Check "the log follows the named file" (Test-Path (Join-Path (Split-Path -Parent $named) "quire.log")) "elsewhere\quire.log"

# ---- (5) control: the same harness does roam when not told otherwise --------
$stick = New-Stick "roam"
$fake = New-FakeAppData "roam"
$r = Invoke-Quire $stick $fake @("--dump-state", "--auto-exit", "$ExitAfter")
$roamed = Join-Path $fake "Quire\quire.db"
Check "control: a default run roams to APPDATA" (Test-Path $roamed) $roamed
Check "control: and leaves nothing in the cwd" (-not (Test-Path (Join-Path $stick "appdata"))) $stick

# ---- (6) the legacy library migrates out, whole ----------------------------
# Seed a real library the portable way, then add a snapshot, a sidecar and a
# log so the move has a family to carry.
$stick = New-Stick "legacy"
$fake = New-FakeAppData "legacy"
$null = Invoke-Quire $stick $fake @("--portable", "--auto-exit", "$ExitAfter")
$legacyDir = Join-Path $stick "appdata"
$srcPages = Pages (Invoke-Quire $stick $fake @("--portable", "--dump-state", "--auto-exit", "$ExitAfter")).stderr
Copy-Item (Join-Path $legacyDir "quire.db") (Join-Path $legacyDir "quire.db.bak1")
"pre-move log line" | Add-Content (Join-Path $legacyDir "quire.log")
$fake = New-FakeAppData "legacy2"
$r = Invoke-Quire $stick $fake @("--dump-state", "--auto-exit", "$ExitAfter")
$dst = Join-Path $fake "Quire\quire.db"
Check "legacy db moved into the per-user folder" (Test-Path $dst) $dst
Check "the old folder no longer holds it" (-not (Test-Path (Join-Path $legacyDir "quire.db"))) $legacyDir
Check "the snapshot travelled" (Test-Path (Join-Path $fake "Quire\quire.db.bak1")) "Quire\quire.db.bak1"
Check "the log travelled" (Test-Path (Join-Path $fake "Quire\quire.log")) "Quire\quire.log"
Check "the move is reported" ($r.stderr -match "moved the library from") "stderr line present"
Check "the same pages are there after the move" ((Pages $r.stderr) -eq $srcPages -and $srcPages -gt 0) "pages $srcPages -> $(Pages $r.stderr)"

# ---- (7) --portable suppresses the migration -------------------------------
$stick = New-Stick "suppress"
$fake = New-FakeAppData "suppress"
$null = Invoke-Quire $stick $fake @("--portable", "--auto-exit", "$ExitAfter")
$legacyDir = Join-Path $stick "appdata"
$fake = New-FakeAppData "suppress2"
$r = Invoke-Quire $stick $fake @("--portable", "--dump-state", "--auto-exit", "$ExitAfter")
$after = if (Test-Path (Join-Path $legacyDir "quire.db")) { (Get-FileHash -Algorithm MD5 -Path (Join-Path $legacyDir "quire.db")).Hash } else { "" }
Check "--portable keeps a stick library in place" ($after -ne "") "the db is still where the stick has it"
Check "--portable reopened it (not an empty library)" ((Pages $r.stderr) -gt 0) "pages=$((Pages $r.stderr))"
Check "--portable created no per-user folder" (-not (Test-Path (Join-Path $fake "Quire"))) $fake
Check "--portable reported no move" ($r.stderr -notmatch "moved the library from") "silent"

# ---- (8) no APPDATA at all --------------------------------------------------
$stick = New-Stick "noappdata"
$r = Invoke-Quire $stick $null @("--dump-state", "--auto-exit", "$ExitAfter")
Check "no APPDATA falls back to the portable placement" (Test-Path (Join-Path $stick "appdata\quire.db")) $stick

# ---- safety: the real per-user library never moved -------------------------
Check "the real %APPDATA%\Quire is untouched" ((Snapshot $realLibrary) -eq $realBefore) $realLibrary

# ---- report -----------------------------------------------------------------
$failed = @($results | Where-Object { -not $_.ok })
""
"portable verification: $($results.Count - $failed.Count)/$($results.Count) checks passed"
if ($failed.Count) { $failed | ForEach-Object { "  FAILED: $($_.name) -- $($_.detail)" }; exit 1 }
exit 0
