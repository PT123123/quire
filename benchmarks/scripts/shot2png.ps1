# Convert the quire-shot BMP to PNG and drop the intermediate.
# Used by `just shot`; see docs/UI_ARCHITECTURE.md (visual regression).
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
$bmp = ".scratch\shots\latest.bmp"
$png = ".scratch\shots\latest.png"
if (-not (Test-Path $bmp)) { Write-Output "no $bmp"; exit 1 }
$img = [System.Drawing.Image]::FromFile((Resolve-Path $bmp))
$img.Save((Join-Path (Get-Location) $png), [System.Drawing.Imaging.ImageFormat]::Png)
$img.Dispose()
Remove-Item $bmp
Write-Output "wrote $png"
