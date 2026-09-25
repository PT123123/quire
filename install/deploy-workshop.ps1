# Deploy a release into the workshop, twice: the version folder is the archive,
# C:\workshop\quire-desktop\quire.exe is the install.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\deploy-workshop.ps1
#
# The workshop directory holds one folder per release, named <name>-<version>
# (aura-1.2.6, aw-qtui-0.1.36), filled with what that build needs to run. Quire's
# release exe is self-contained — dist.ps1 zips the one file and nothing beside
# it — so its folder is quire.exe alone.
#
# The name is `quire-desktop`, not the manifest's `quire`: the workshop is where
# this drops beside *other* applications, and the two shells this repository has
# (quire-desktop, quire-droid) are two builds of one application that ship on
# different days. A bare `quire-<version>` would read as one application in a
# list that already holds one folder per release of several, with nothing saying
# which shell made it.
#
# Two destinations, and they are two different things:
#
#   * C:\workshop\quire-desktop-<version>\quire.exe — the archive. One folder per
#     release, never moved, so the workshop can still say what was built when.
#   * C:\workshop\quire-desktop\quire.exe — the install. This is the path the app
#     is *run* from, and every deploy overwrites it in place, so "the newest
#     release" is one path instead of a folder name to remember. Windows will not
#     replace a running exe, which is why the instance living there has to leave
#     first (step 4) — through the app's own quit channel, never a kill.
#
# `just deploy-workshop` is the entry point, and this script is the whole flow.
# The steps are ordered so each destination holds a binary that says <version>:
#
#   1. [package] version's patch +1 in Cargo.toml. Both destinations are named
#      after (or checked against) the version, so this is what makes a deploy
#      *land*: without it the install would be overwritten by the same build.
#   2. `cargo build --release`. The bump has to come first for more than the
#      folder name: build.rs generates the exe's version resource block from
#      CARGO_PKG_VERSION — the number the file properties tab shows — so a build
#      that runs before the bump carries the old version. That is why the build
#      lives here rather than in the recipe.
#   3. The bump is committed (Cargo.toml + Cargo.lock, nothing else) and pushed,
#      the way every repository in this workspace is left: the commit is the
#      record of what each folder in the workshop was built from.
#   4. The instance running from the install path is asked to end its own
#      session, and waited for. Deliberately last before the copy: closing the
#      user's window is the one step here they can feel, so nothing that can
#      fail — a build, a commit, a push — is allowed to happen after it.
#   5. target\release\quire.exe is copied to the version folder and over the
#      install.
#
# One rule for the text below: every character inside a Write-Output or a throw
# is ASCII. Windows PowerShell 5.1 reads a BOM-less script as ANSI — this file is
# UTF-8 like the rest of the repository — so a non-ASCII character in a *string
# literal* reaches the console as mojibake (measured: an em dash came out as
# "鈥?", and this machine's codepage is GBK). Comments are free; messages are
# not, because the one place they are read is a deploy that went wrong.

$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Split-Path -Parent $here
Set-Location $root

# The two destinations are known before anything happens, so a typo fails here
# rather than after a build and a push.
$install = Join-Path 'C:\workshop' 'quire-desktop'
$installExe = Join-Path $install 'quire.exe'

# ── 1: the bump ─────────────────────────────────────────────────────────────
# The first top-level `version` key in the manifest is [package]'s: the
# [workspace.dependencies] table above it indents its keys, and dependency tables
# below it do the same. The [dependencies] check turns that convention into a
# guard, so a reshuffled manifest fails loudly instead of quietly bumping some
# library.
$manifest = Join-Path $root 'Cargo.toml'
$text = [System.IO.File]::ReadAllText($manifest)
$m = [regex]::Match($text, '(?m)^version\s*=\s*"(\d+)\.(\d+)\.(\d+)"')
if (-not $m.Success) { throw "no [package] version line in Cargo.toml" }
$deps = $text.IndexOf('[dependencies]')
if ($deps -ge 0 -and $m.Index -gt $deps) {
    throw "the first top-level 'version' key sits below [dependencies]; refusing to bump the wrong table"
}
$version = "{0}.{1}.{2}" -f $m.Groups[1].Value, $m.Groups[2].Value, ([int]$m.Groups[3].Value + 1)
# splice exactly the three digits — group 1 starts at the major, group 3 ends at
# the patch — so every other byte of the manifest is untouched
$vStart = $m.Groups[1].Index
$vEnd = $m.Groups[3].Index + $m.Groups[3].Length
$text = $text.Substring(0, $vStart) + $version + $text.Substring($vEnd)
$utf8 = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($manifest, $text, $utf8)
# read it back before anything expensive happens on top of a bad manifest
$check = [regex]::Match([System.IO.File]::ReadAllText($manifest), '(?m)^version\s*=\s*"([^"]+)"')
if (-not $check.Success -or $check.Groups[1].Value -ne $version) {
    throw ("the bump wrote '{0}', expected '{1}'; aborting before the build" -f `
        $check.Groups[1].Value, $version)
}
Write-Output "==> version -> $version"

# ── 2: the build ────────────────────────────────────────────────────────────
# Default renderer, the same `just build` produces: the workshop gets the exe
# that ships. This is the slow step (~2m30s) and it is one crate: the bump
# invalidated `quire` itself, which is where codegen-units = 1 + thin LTO spends
# its time (ADR-0024's update, PERFORMANCE.md "Build cost").
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Join-Path $root 'target\release\quire.exe'
if (-not (Test-Path $exe)) { throw "no exe at $exe; the build reported success but left nothing" }

# The exe is asked to confirm it took the bump, because it is the artifact that
# will be run from a folder named after the version. A stale one — a build that
# silently did not happen — reports the previous version here. `NotAttempted`
# (no rc.exe on this machine) leaves the block empty, and an empty value is a
# soft miss rather than a failed deploy, the same way build.rs treats it.
$reported = (Get-Item $exe).VersionInfo.FileVersion
if ($reported -and $reported -notlike "$version.*") {
    throw "$exe reports version $reported, expected $version; the build did not take the bump"
}

# ── 3: the bump goes in as its own commit ───────────────────────────────────
# Cargo.lock records the workspace member's version too, so it is part of the
# bump; nothing else is staged, so nothing else is committed.
git add Cargo.toml Cargo.lock
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
git commit -m "chore(release): $version"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
git push
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# ── 4: the old instance leaves through its own door ─────────────────────────
# Only the instance sitting on a file this deploy writes is asked, and it is
# asked by name, never killed: `--quit` goes through the app's quit channel,
# which is the same route the tray menu's exit item takes, so its final flush
# runs and it writes its clean-exit record instead of the next start reporting a
# crash that never happened (ADR-0105).
#
# An instance running out of an older quire-desktop-<version> folder holds
# nothing here — those folders are the archive and this deploy does not touch
# them — so it is named and left alone. Quitting one would be closing a window
# the user still has open for no reason.
$running = @(Get-Process quire -ErrorAction SilentlyContinue)
$blocking = @($running | Where-Object { $_.Path -eq $installExe })
foreach ($p in @($running | Where-Object { $_.Path -ne $installExe })) {
    Write-Output "==> note: an older release is still running from $($p.Path); it holds nothing this deploy writes, leaving it alone"
}
if ($blocking.Count -gt 0) {
    $pids = @($blocking | ForEach-Object { $_.Id })
    Write-Output "==> asking the instance at $installExe (pid $($pids -join ', ')) to quit"
    # Start-Process -Wait -PassThru, not `& $exe --quit`: the release exe is a
    # GUI-subsystem binary (main.rs's windows_subsystem), and PowerShell neither
    # waits for one of those nor sets $LASTEXITCODE after it — so `&` here would
    # compare against nothing and report a quit that worked as a failure. The
    # process object's own exit code is the answer either way.
    $quit = Start-Process -FilePath $exe -ArgumentList '--quit' -NoNewWindow -Wait -PassThru
    if ($quit.ExitCode -ne 0) {
        throw "$installExe is running and did not accept the quit request; end it from its tray icon (right-click, Exit) and run the deploy again"
    }
    # It accepted, so it is on its way out: the flush and the log line happen
    # after the event loop returns, and the exe stays locked until the process is
    # really gone. Waiting is what keeps the copy below honest.
    $deadline = (Get-Date).AddSeconds(30)
    while ((Get-Date) -lt $deadline) {
        if (@(Get-Process quire -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $installExe }).Count -eq 0) { break }
        Start-Sleep -Milliseconds 200
    }
    if (@(Get-Process quire -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $installExe }).Count -gt 0) {
        throw "$installExe accepted the quit request but is still running after 30 s; refusing to overwrite a live exe"
    }
    Write-Output "==> the old instance exited (pid $($pids -join ', '))"
}

# ── 5: the deploy ───────────────────────────────────────────────────────────
# Archive first, install second: if the copy onto the install fails, the release
# is still in the workshop under its own version rather than nowhere.
$archive = Join-Path 'C:\workshop' "quire-desktop-$version"
New-Item -ItemType Directory -Force -Path $archive | Out-Null
Copy-Item $exe (Join-Path $archive 'quire.exe') -Force
New-Item -ItemType Directory -Force -Path $install | Out-Null
Copy-Item $exe $installExe -Force
Write-Output ("==> archived v{0} ({1:N1} MiB) -> {2}" -f `
    $version, ((Get-Item $exe).Length / 1MB), (Join-Path $archive 'quire.exe'))
Write-Output "==> installed in place -> $installExe"
