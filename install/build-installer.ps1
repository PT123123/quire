# Build the Windows setup program (M8 / D8).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\build-installer.ps1
#
# Options: -SkipBuild reuses the release exe already in target\release.
param(
    [switch]$SkipBuild
)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

if (-not $SkipBuild) {
    Write-Output "cargo build --release --bin quire"
    cargo build --release --bin quire
    if ($LASTEXITCODE -ne 0) { throw "release build failed" }
}

$exe = "target\release\quire.exe"
if (-not (Test-Path $exe)) { throw "$exe is missing; run without -SkipBuild" }

# ISCC is the Inno Setup command-line compiler. winget install JRSoftware.InnoSetup
# puts it under the per-user Programs folder, which is not on PATH.
$candidates = @(
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
)
$iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) {
    $onPath = Get-Command iscc.exe -ErrorAction SilentlyContinue
    if ($onPath) { $iscc = $onPath.Source }
}
if (-not $iscc) { throw "ISCC.exe not found. Install Inno Setup: winget install JRSoftware.InnoSetup" }

Write-Output "compiling install\quire.iss with $iscc"
& $iscc /Qp "install\quire.iss"
if ($LASTEXITCODE -ne 0) { throw "ISCC failed with exit code $LASTEXITCODE" }

$setup = Get-ChildItem "dist\Quire-*-windows-x64-setup.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
Write-Output "installer ready: $($setup.FullName) ($([math]::Round($setup.Length / 1MB, 2)) MB)"
