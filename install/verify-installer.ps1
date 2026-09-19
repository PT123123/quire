# A6 · installer end-to-end verification (M8 / D8).
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\verify-installer.ps1
#
# Builds the setup program from install\quire.iss, installs it silently into a
# scratch prefix (never the default {localappdata}\Programs\Quire), checks the
# four things that matter, then uninstalls and proves nothing was left behind:
#
#   (a) the installed exe launches against a scratch database and exits 0
#   (b) the shell icon identity is the exe's embedded resource (build.rs),
#       matching install\quire.ico — the taskbar has no other source, since
#       Slint 1.18 sets no window icon
#   (c) the optional .md "Open with" task lands in HKCU when selected, without
#       claiming a default handler or disturbing the existing candidates
#   (d) after uninstall: the install dir, both registry keys, the Add/Remove
#       entry and the Start-Menu shortcuts are gone
#
# The app's per-user library (%APPDATA%\Quire) is never used: every launch here
# passes --db under the scratch dir. The icon comparison reads pixels, so it
# needs System.Drawing; no window is shown and nothing is screenshotted.
param(
    [string]$Dir = ""
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
Add-Type -AssemblyName System.Drawing

if ($Dir -eq "") { $Dir = Join-Path $root ".scratch\a6\install" }
$data = Join-Path $root ".scratch\a6\data"
if (Test-Path $Dir) { throw "$Dir already exists; clear it first" }
# a fresh library, so the (c) page-count delta is the import and nothing else
if (Test-Path $data) { Remove-Item -Recurse -Force $data }
New-Item -ItemType Directory -Force -Path $data | Out-Null

# The release exe is the input; build it first (cargo build --release --bin
# quire). build-installer.ps1 -SkipBuild compiles the .iss against it.
& (Join-Path $PSScriptRoot "build-installer.ps1") -SkipBuild
if ($LASTEXITCODE -ne 0) { throw "installer build failed" }
$setup = Get-ChildItem "dist\Quire-*-windows-x64-setup.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setup) { throw "no setup program in dist\" }
Write-Output "setup: $($setup.Name) ($([math]::Round($setup.Length / 1MB, 2)) MB)"

# ---- install ----------------------------------------------------------------
$p = Start-Process -FilePath $setup.FullName -PassThru -Wait -ArgumentList @(
    "/VERYSILENT", "/NORESTART", "/SUPPRESSMSGBOXES", "/DIR=$Dir", "/TASKS=assocmd")
Write-Output "install exit: $($p.ExitCode)"
foreach ($f in @("quire.exe", "README.md", "quire.png", "unins000.exe")) {
    Write-Output ("  payload {0,-14} {1}" -f $f, (Test-Path (Join-Path $Dir $f)))
}

# ---- (a) the installed binary runs -----------------------------------------
$err = Join-Path $data "launch.err"
$p = Start-Process -FilePath (Join-Path $Dir "quire.exe") -PassThru -Wait -RedirectStandardError $err -ArgumentList @(
    "--db", (Join-Path $data "quire.db"), "--auto-exit", "3")
Write-Output "(a) installed exe exit: $($p.ExitCode), stderr lines: $((Get-Content $err -ErrorAction SilentlyContinue | Measure-Object).Count)"
Write-Output ("(a) scratch library written: " + ((Get-ChildItem $data -Filter "quire.*" | ForEach-Object Name) -join " "))

# ---- (b) the embedded icon resource ----------------------------------------
function Get-IconSig($path) {
    $icon = [System.Drawing.Icon]::ExtractAssociatedIcon($path)
    $bmp = $icon.ToBitmap()
    $sb = New-Object System.Text.StringBuilder
    for ($y = 0; $y -lt $bmp.Height; $y += 4) {
        for ($x = 0; $x -lt $bmp.Width; $x += 4) {
            $c = $bmp.GetPixel($x, $y)
            [void]$sb.Append(("{0:x2}{1:x2}{2:x2}" -f $c.R, $c.G, $c.B))
        }
    }
    $bmp.Dispose(); $icon.Dispose()
    $sb.ToString()
}
$exeSig = Get-IconSig (Join-Path $Dir "quire.exe")
$icoSig = Get-IconSig (Join-Path $PSScriptRoot "quire.ico")
$exeInfo = (Get-Item (Join-Path $Dir "quire.exe")).VersionInfo
Write-Output ("(b) exe version resource: {0} / {1}" -f $exeInfo.ProductName, $exeInfo.FileVersion)
Write-Output ("(b) embedded icon matches install\quire.ico: {0} ({1} px sample)" -f ($exeSig -eq $icoSig), $exeSig.Length)
Write-Output ("(b) payload is the built exe: {0}" -f ((Get-FileHash (Join-Path $Dir "quire.exe")).Hash -eq (Get-FileHash "target\release\quire.exe").Hash))

# ---- (c) the .md association -----------------------------------------------
$progids = Get-Item "HKCU:\Software\Classes\.md\OpenWithProgids"
$others = @($progids.Property | Where-Object { $_ -and $_ -ne "Quire.Markdown" })
Write-Output ("(c) Quire.Markdown offered: {0}" -f [bool]($progids.Property -contains "Quire.Markdown"))
Write-Output ("(c) pre-existing candidates kept: {0}" -f ($others -join " "))
Write-Output ("(c) .md default handler untouched: {0}" -f (-not (@(Get-Item "HKCU:\Software\Classes\.md").Property -contains "(default)")))
Write-Output ("(c) open command: {0}" -f (Get-ItemProperty "HKCU:\Software\Classes\Quire.Markdown\shell\open\command")."(default)")
$un = Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{889B1B1B-ADCC-4684-8070-903342F028D3}_is1" -ErrorAction SilentlyContinue
Write-Output ("(c) Add/Remove entry: {0} {1}" -f $un.DisplayName, $un.DisplayVersion)

# and the command line the verb points at really imports the file: the same
# argument list Explorer would pass, run against the scratch library.
function Get-PageCount([string[]]$extra) {
    $dump = Join-Path $data "dump.err"
    if (Test-Path $dump) { Remove-Item $dump }
    $a = @("--db", (Join-Path $data "quire.db"), "--auto-exit", "3", "--dump-state") + $extra
    Start-Process -FilePath (Join-Path $Dir "quire.exe") -ArgumentList $a -Wait -RedirectStandardError $dump | Out-Null
    $l = (Get-Content $dump -ErrorAction SilentlyContinue | Where-Object { $_ -like "dump-state: pages=*" } | Select-Object -Last 1)
    if ($l -match "pages=(\d+)") { return [int]$Matches[1] }
    return -1
}
$md = Join-Path $data "verb-check.md"
Set-Content -Path $md -Value "# Verb check`n`nwritten by verify-installer" -Encoding UTF8
$before = Get-PageCount @()
$after = Get-PageCount @("--open", $md)
Write-Output ("(c) the verb's command line imports: pages {0} -> {1} (expect +1)" -f $before, $after)

# ---- (d) uninstall + residue -----------------------------------------------
$p = Start-Process -FilePath (Join-Path $Dir "unins000.exe") -PassThru -Wait -ArgumentList @("/VERYSILENT", "/NORESTART", "/SUPPRESSMSGBOXES")
Write-Output "uninstall exit: $($p.ExitCode)"
Start-Sleep -Seconds 2
$lnks = @(Get-ChildItem "$env:APPDATA\Microsoft\Windows\Start Menu\Programs" -Filter "*Quire*" -ErrorAction SilentlyContinue)
Write-Output ("(d) install dir gone: {0}" -f (-not (Test-Path $Dir)))
Write-Output ("(d) progid key gone: {0}" -f (-not (Test-Path "HKCU:\Software\Classes\Quire.Markdown")))
Write-Output ("(d) Quire.Markdown removed from .md offer list: {0}" -f (-not (@(Get-Item "HKCU:\Software\Classes\.md\OpenWithProgids").Property -contains "Quire.Markdown")))
Write-Output ("(d) Add/Remove entry gone: {0}" -f (-not (Test-Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{889B1B1B-ADCC-4684-8070-903342F028D3}_is1")))
Write-Output ("(d) Start-Menu shortcuts gone: {0}" -f ($lnks.Count -eq 0))
Write-Output ("(d) the per-user library was never in the install dir: {0}" -f (-not (Test-Path (Join-Path $Dir "appdata"))))
