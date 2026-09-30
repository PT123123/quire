# Deploy a release into the workshop's archive: one folder per release,
# C:\workshop\quire-desktop-<version>\quire.exe.
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
# This recipe used to keep a second destination — the in-place install at
# C:\workshop\quire-desktop\quire.exe — and every deploy overwrote it, which is
# why the old flow asked any instance running there to quit first (ADR-0105's
# channel). That install path is gone (ADR-0139): the archive folder is new
# every release, so nothing is ever overwritten and no running instance is ever
# in the way. The workshop is a record of what was built; it is not where the
# app is run from, and a folder that a deploy fought a running exe for was
# exactly the folder that never should have held one.
#
# `just deploy-workshop` is the entry point, and this script is the whole flow.
# The steps are ordered so the folder holds a binary that says <version>:
#
#   1. [package] version's patch +1 in Cargo.toml. The folder is named after the
#      version, so this is what makes a deploy *land*: without it the archive
#      would be rewritten by the same build.
#   2. `cargo build --release`. The bump has to come first for more than the
#      folder name: build.rs generates the exe's version resource block from
#      CARGO_PKG_VERSION — the number the file properties tab shows — so a build
#      that runs before the bump carries the old version. That is why the build
#      lives here rather than in the recipe.
#   3. The bump is committed (Cargo.toml + Cargo.lock, nothing else) and pushed,
#      the way every repository in this workspace is left: the commit is the
#      record of what each folder in the workshop was built from.
#   4. target\release\quire.exe is copied into the version folder.
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

# ── 4: the deploy ───────────────────────────────────────────────────────────
# One copy, into the folder named after the version. There is no in-place
# install path any more (ADR-0139), so no running instance sits on a file this
# deploy writes and nothing has to be asked to leave.
$archive = Join-Path 'C:\workshop' "quire-desktop-$version"
New-Item -ItemType Directory -Force -Path $archive | Out-Null
Copy-Item $exe (Join-Path $archive 'quire.exe') -Force
Write-Output ("==> archived v{0} ({1:N1} MiB) -> {2}" -f `
    $version, ((Get-Item $exe).Length / 1MB), (Join-Path $archive 'quire.exe'))