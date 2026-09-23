# Put the release build where the workshop keeps it: C:\workshop\quire-<version>\.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\workshop-deploy.ps1
#
# The workshop directory holds one folder per release, named <name>-<version>
# (aura-1.2.6, aw-qtui-0.1.28), filled with what that build needs to run. Quire's
# release exe is self-contained — dist.ps1 zips the one file and nothing beside
# it — so its folder is quire.exe alone. `just workshop-deploy` builds first;
# this script only reads the version out of Cargo.toml and copies. The version
# is what names the folder, so a bump is what makes the next deploy land
# somewhere new instead of over the previous build.

$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Split-Path -Parent $here

$exe = Join-Path $root 'target\release\quire.exe'
if (-not (Test-Path $exe)) { throw "no exe at $exe — run 'cargo build --release' first" }

$hit = Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' |
    Select-Object -First 1
if (-not $hit) { throw "no [package] version line in Cargo.toml" }
$version = $hit.Matches[0].Groups[1].Value

$dest = Join-Path 'C:\workshop' "quire-$version"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
$target = Join-Path $dest 'quire.exe'
Copy-Item $exe $target -Force
Write-Output ("deployed quire.exe ({0:N1} MiB, v{1}) -> {2}" -f `
    ((Get-Item $exe).Length / 1MB), $version, $target)
