# Publish a release: bump the patch, build the release exe, and put it on GitHub.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\release-publish.ps1
#
# This is the desktop shell's *delivery* — the sibling of the two Android shells'
# scripts\release-publish.ps1, and the same shape: the patch is bumped, the
# artifact is built, the bump lands as its own commit, and `gh release create`
# attaches what was built. `just deploy-workshop` is the other door a release
# leaves by (this machine's C:\workshop), and it advances the version too: the two
# are independent acts, not two halves of one, which is why a version can move
# without a GitHub release existing for it and the other way round.
#
# Two assets, because this app is installed two ways and both come from the one
# exe:
#
#   * Quire-<version>-windows-x64-setup.exe — install\build-installer.ps1's Inno
#     Setup program: per-user, no elevation, and uninstalling leaves the library
#     alone. quire.iss reads the version out of the exe's own version resource,
#     which build.rs stamps from [package] version.
#   * Quire-<version>-windows-x64.zip — the portable single file `just dist`
#     packages, for someone who would rather not install anything.
#
# The steps are ordered so nothing ships without its bookkeeping:
#
#   1. [package] version's patch +1 in Cargo.toml. The bump has to precede the
#      build: build.rs generates the exe's version resource from CARGO_PKG_VERSION,
#      and the installer's own file name and AppVersion are read from that
#      resource — so a publish that built before the bump would publish the
#      previous version under a new tag.
#   2. `cargo build --release`, and then the exe is asked to confirm it took the
#      bump. A stale exe reports the previous version here; `NotAttempted` (no
#      rc.exe on this machine) leaves the block empty, and an empty value is a
#      soft miss rather than a failed publish, the way build.rs treats it.
#   3. The two assets. The installer first, because it is the one that can fail
#      for a reason that has nothing to do with this repository (Inno Setup is
#      not installed) — and it must fail before a push, not after.
#   4. The bump is committed (Cargo.toml + Cargo.lock, nothing else) and pushed,
#      so the tag created next points at a commit carrying the version it names.
#   5. `gh release create v<version>` attaches both files. Re-running after a
#      failed publish finds the tag already there and re-uploads over it rather
#      than erroring out. Needs `gh` signed in.
#
# One rule for the text below: every character inside a Write-Output or a throw
# is ASCII. Windows PowerShell 5.1 reads a BOM-less script as ANSI — this file is
# UTF-8 like the rest of the repository — so a non-ASCII character in a *string
# literal* reaches the console as mojibake (measured: an em dash came out as
# "鈥?", and this machine's codepage is GBK). Comments are free; messages are
# not, because the one place they are read is a publish that went wrong.

$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Split-Path -Parent $here
Set-Location $root

if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "gh is not on PATH - install the GitHub CLI and 'gh auth login' first"
}

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
# Default renderer, the same `just build` produces: the release gets the exe that
# ships. This is the slow step (~2m30s), and it is one crate: the bump
# invalidated `quire` itself, which is where codegen-units = 1 + thin LTO spends
# its time (ADR-0024's update, PERFORMANCE.md "Build cost").
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$exe = Join-Path $root 'target\release\quire.exe'
if (-not (Test-Path $exe)) { throw "no exe at $exe; the build reported success but left nothing" }

$reported = (Get-Item $exe).VersionInfo.FileVersion
if ($reported -and $reported -notlike "$version.*") {
    throw "$exe reports version $reported, expected $version; the build did not take the bump"
}

# ── 3: the two assets ───────────────────────────────────────────────────────
# The setup program. `-SkipBuild` because the exe above is the one it must wrap,
# and building again would only be a second chance to wrap a different one.
& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $here 'build-installer.ps1') -SkipBuild
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$setup = Join-Path $root ("dist\Quire-{0}-windows-x64-setup.exe" -f $version)
if (-not (Test-Path $setup)) {
    throw "no installer at $setup - build-installer.ps1 reported success but left nothing"
}

# The portable zip, from the packaging recipe `just dist` runs, renamed to carry
# the version the way both assets here and both Android shells' do: a downloaded
# `quire-windows-x64.zip` cannot say which release it came from.
& powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $root 'benchmarks\scripts\dist.ps1')
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$zipPlain = Join-Path $root 'dist\quire-windows-x64.zip'
if (-not (Test-Path $zipPlain)) { throw "no zip at $zipPlain - dist.ps1 reported success but left nothing" }
$zip = Join-Path $root ("dist\Quire-{0}-windows-x64.zip" -f $version)
Move-Item $zipPlain $zip -Force

# ── 4: the bump goes in as its own commit ───────────────────────────────────
git add Cargo.toml Cargo.lock
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
git commit -m "chore(release): $version"
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
git push
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# ── 5: the release ──────────────────────────────────────────────────────────
$repo = (& gh repo view --json nameWithOwner | ConvertFrom-Json).nameWithOwner
$tag = "v$version"
$existing = & gh release list --json tagName --limit 200 | ConvertFrom-Json
if (@($existing | Where-Object { $_.tagName -eq $tag }).Count -gt 0) {
    Write-Output "==> release $tag already exists; replacing its assets"
    & gh release upload $tag $zip $setup --clobber
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} else {
    $notes = @"
Quire $version for Windows x64: a local, GPU-accelerated, Notion-like document
workspace in one process (Rust + Slint) - nothing to install alongside it.

Two ways in, both in this release:

- **Quire-$version-windows-x64-setup.exe** installs per-user (no elevation, no
  admin prompt), and uninstalling never touches your notes.
- **Quire-$version-windows-x64.zip** is the portable build: unzip it and run
  quire.exe. Nothing else is needed beside it.

Your library is a single SQLite file at %APPDATA%\Quire\quire.db - the same
folder name the Android shells use, so a library moves between them. `--portable`
and `--db <path>` move it on purpose.

Requires Windows 10+ (x64). See CHANGELOG.md for what is in this build.
"@
    & gh release create $tag $zip $setup --target master --title "Quire $version" --notes $notes
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
Write-Output "==> published $tag ($zip, $setup)"
