# Bounding box of the pixels that changed between two sweeps (UI_ARCHITECTURE.md,
# Visual regression). Sampling every 2nd pixel: a layout change that only moves
# the page title should report a box inside the title band, and if every scene's
# box lands inside the region the change explains, one verdict covers the set.
param(
    [string]$OldDir = ".scratch/sweep-baseline",
    [string]$NewDir = ".scratch/sweep"
)
Add-Type -AssemblyName System.Drawing
Get-ChildItem -Path $NewDir -Filter *.png | ForEach-Object {
    $np = $_.FullName
    $op = Join-Path $OldDir $_.Name
    if (-not (Test-Path $op)) { Write-Output ("{0,-22} NEW" -f $_.BaseName); return }
    $a = [System.Drawing.Bitmap]::FromFile($op)
    $b = [System.Drawing.Bitmap]::FromFile($np)
    if ($a.Width -ne $b.Width -or $a.Height -ne $b.Height) {
        Write-Output ("{0,-22} SIZE" -f $_.BaseName); $a.Dispose(); $b.Dispose(); return
    }
    $minx = 99999; $miny = 99999; $maxx = -1; $maxy = -1; $n = 0
    for ($y = 0; $y -lt $a.Height; $y += 2) {
        for ($x = 0; $x -lt $a.Width; $x += 2) {
            $ca = $a.GetPixel($x, $y); $cb = $b.GetPixel($x, $y)
            if ([Math]::Abs($ca.R - $cb.R) -gt 6 -or [Math]::Abs($ca.G - $cb.G) -gt 6 -or [Math]::Abs($ca.B - $cb.B) -gt 6) {
                $n++
                if ($x -lt $minx) { $minx = $x }; if ($x -gt $maxx) { $maxx = $x }
                if ($y -lt $miny) { $miny = $y }; if ($y -gt $maxy) { $maxy = $y }
            }
        }
    }
    if ($n -eq 0) { Write-Output ("{0,-22} same" -f $_.BaseName) }
    else { Write-Output ("{0,-22} px={1,-6} bbox x {2}..{3}  y {4}..{5}" -f $_.BaseName, $n, $minx, $maxx, $miny, $maxy) }
    $a.Dispose(); $b.Dispose()
}
