param(
    [string]$OutDir = ".scratch/sweep",
    [int]$W = 1280,
    [int]$H = 800,
    # comma-separated scene list; empty = every scene apply_scene understands
    [string]$Scenes = ""
)
$ErrorActionPreference = "SilentlyContinue"
Add-Type -AssemblyName System.Drawing

# A4 · full visual regression sweep (SPEC §二十一): render every declared
# scene through the headless software renderer into $OutDir/<scene>.png.
# Nothing here judges the pixels — the PNGs plus .scratch/sweep/report.md are
# the deliverable, reviewed by hand against the checklist.
$shot = "target\debug\quire-shot.exe"
if (-not (Test-Path $shot)) {
    Write-Output "build it first: cargo build --features software --bin quire-shot"
    exit 1
}

# light as declared, plus every dark-* the controller knows
$all = @(
    "default", "edit", "empty", "nest", "rename", "title-edit", "recovered",
    "find", "marks", "block-colors", "page-block", "link-block",
    "slash", "plus", "block-menu", "move-to", "page-move-to", "text-color",
    "bg-color", "link", "menu", "palette", "search", "search-notes",
    "dialog", "settings",
    "dark", "dark-slash", "dark-find", "dark-marks", "dark-link",
    "dark-block-menu", "dark-block-colors", "dark-title-edit"
)
$targets = if ($Scenes -eq "") { $all } else { $Scenes -split ',' }

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
foreach ($s in $targets) {
    $bmp = Join-Path $OutDir "$s.bmp"
    $png = Join-Path $OutDir "$s.png"
    if (Test-Path $png) { Remove-Item $png }
    if (Test-Path $bmp) { Remove-Item $bmp }
    $sceneArg = if ($s -eq "default") { @() } else { @("--scene", $s) }
    & $shot --out $bmp --w $W --h $H @sceneArg | Out-Null
    if (-not (Test-Path $bmp)) {
        Write-Output ("{0,-20} RENDER-FAIL" -f $s)
        continue
    }
    $img = [System.Drawing.Image]::FromFile((Resolve-Path $bmp))
    $img.Save((Join-Path (Get-Location) $png), [System.Drawing.Imaging.ImageFormat]::Png)
    $img.Dispose()
    Remove-Item $bmp
    Write-Output ("{0,-20} {1} px {2} KB" -f $s, "$W`x$H", [math]::Round((Get-Item $png).Length / 1KB))
}
Write-Output "sweep at commit: $((git rev-parse --short HEAD) -join '')"
Write-Output "pngs in: $OutDir"
