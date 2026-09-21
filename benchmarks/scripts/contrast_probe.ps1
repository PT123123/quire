# SPEC §二十一 over §三十八: a page title drawn on its cover has to clear the
# same 4.5:1 floor as any other text, and the claim is arithmetic about pixels,
# not an eyeball verdict. This reads a rendered PNG, finds the hero title's own
# ink, and reports the WCAG 2.x contrast ratio against the *brightest* pixel
# that grounds it -- the worst case, since a real cover is whatever photo the
# user picked.
#
# Glyph edges are excluded rather than guessed at: a pixel within -Dilate px of
# an ink pixel is an antialiasing blend between the ink and its background, and
# counting one as background would report a contrast no reader ever sees. A
# 38px bold title has strokes wide enough to leave background between them.
#
#   powershell -File benchmarks/scripts/contrast_probe.ps1 `
#       -Png .scratch/sweep/page-cover-white.png -Ink '#ffffff' -Require 4.5
#
# -Top/-Bottom are the cover band's own rows, measured from the render rather
# than assumed: the page's white background and the top chrome carry the same
# colour as the ink, so a window that spans them would call the chrome "glyphs"
# and the page "the worst-case photo". The reported ink box is what proves the
# window was right -- it has to sit inside the band with room to spare.
#
# Exits 0 on PASS, 1 on FAIL, 2 when the window holds no ink at all, or when
# the known-answer selftest moves -- the scene did not render what it claims,
# which is the failure this file exists to catch: a silent probe is not a
# negative result.
param(
    [Parameter(Mandatory = $true)][string]$Png,
    # the ink colour the hero title is supposed to draw in
    [string]$Ink = "#ffffff",
    # page-content window: past the 260px sidebar, short of the scrollbar
    [int]$Left = 260,
    [int]$Right = 1200,
    [int]$Top = 48,
    [int]$Bottom = 216,
    # channel distance within which a pixel is still the ink itself
    [int]$Tol = 4,
    [int]$Dilate = 2,
    [double]$Require = 4.5
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

# sRGB relative luminance, one lookup table per channel instead of three
# pow() calls per pixel -- the window is a quarter of a million pixels.
$lin = New-Object 'double[]' 256
for ($c = 0; $c -lt 256; $c++) {
    $s = $c / 255.0
    if ($s -le 0.03928) { $lin[$c] = $s / 12.92 }
    else { $lin[$c] = [Math]::Pow((($s + 0.055) / 1.055), 2.4) }
}
function Lum256($r, $g, $b) { 0.2126 * $script:lin[$r] + 0.7152 * $script:lin[$g] + 0.0722 * $script:lin[$b] }
function Ratio($a, $b) {
    $hi = [Math]::Max($a, $b); $lo = [Math]::Min($a, $b)
    ($hi + 0.05) / ($lo + 0.05)
}

# A known-answer arm, run before any screenshot is trusted: pure white on pure
# black is 21:1 by definition, white on white is 1:1, and white on the sRGB 97
# that a 0.6196 black veil composites over the brightest photo a user can hand
# over is the 6.19 this slice's arithmetic claims. If these move, every PASS
# under them is arithmetic that no longer describes pixels.
$known = @(
    @("white/black", (Ratio (Lum256 255 255 255) (Lum256 0 0 0)), 21.00),
    @("white/white", (Ratio (Lum256 255 255 255) (Lum256 255 255 255)), 1.00),
    @("white/97grey", (Ratio (Lum256 255 255 255) (Lum256 97 97 97)), 6.19)
)
foreach ($k in $known) {
    $delta = [Math]::Abs($k[1] - $k[2])
    Write-Output ("selftest  {0,-12} {1,5:N2}:1 want {2,5:N2} {3}" -f $k[0], $k[1], $k[2],
        $(if ($delta -lt 0.02) { "ok" } else { "BROKEN" }))
    if ($delta -ge 0.02) { exit 2 }
}

$ir = [Convert]::ToInt32($Ink.Substring(1, 2), 16)
$ig = [Convert]::ToInt32($Ink.Substring(3, 2), 16)
$ib = [Convert]::ToInt32($Ink.Substring(5, 2), 16)
$inkLum = Lum256 $ir $ig $ib

$bmp = New-Object System.Drawing.Bitmap $Png
$w = $bmp.Width; $h = $bmp.Height
$bytes = New-Object byte[] ($w * $h * 4)
# Format32bppArgb, converted through a destination copy rather than LockBits on
# the decoder's own format: a PNG saved by GDI+ arrives indexed or 24-bit, and
# locking that as ARGB is what threw the MissingMethod above.
$dest = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$g = [System.Drawing.Graphics]::FromImage($dest)
$g.DrawImage($bmp, 0, 0, $w, $h)
$g.Dispose()
$full = New-Object System.Drawing.Rectangle(0, 0, $w, $h)
$data = $dest.LockBits($full, [System.Drawing.Imaging.ImageLockMode]::ReadOnly,
    [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
$stride = $data.Stride
$bytes = New-Object byte[] ($stride * $h)
[System.Runtime.InteropServices.Marshal]::Copy($data.Scan0, $bytes, 0, $bytes.Length)
$dest.UnlockBits($data)
$dest.Dispose()
$bmp.Dispose()

$x0 = [Math]::Max(0, $Left); $x1 = [Math]::Min($w, $Right)
$y0 = [Math]::Max(0, $Top); $y1 = [Math]::Min($h, $Bottom)

# pass 1: the ink, and its box
$near = New-Object 'bool[]' ($w * $h)
$inkCount = 0
$minX = $w; $maxX = -1; $minY = $h; $maxY = -1
for ($y = $y0; $y -lt $y1; $y++) {
    $row = $y * $stride
    $nrow = $y * $w
    for ($x = $x0; $x -lt $x1; $x++) {
        $i = $row + $x * 4
        # the buffer is BGRA on this pixel format
        if ([Math]::Abs($bytes[$i + 2] - $ir) -le $Tol -and
            [Math]::Abs($bytes[$i + 1] - $ig) -le $Tol -and
            [Math]::Abs($bytes[$i] - $ib) -le $Tol) {
            $inkCount++
            if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
            if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y }
            for ($dy = -$Dilate; $dy -le $Dilate; $dy++) {
                $yy = $y + $dy
                if ($yy -lt $y0 -or $yy -ge $y1) { continue }
                $nr = $yy * $w
                for ($dx = -$Dilate; $dx -le $Dilate; $dx++) {
                    $xx = $x + $dx
                    if ($xx -ge $x0 -and $xx -lt $x1) { $near[$nr + $xx] = $true }
                }
            }
        }
    }
}
$leaf = Split-Path $Png -Leaf
if ($inkCount -eq 0) {
    Write-Output ("{0,-28} NO INK of {1} inside x {2}..{3} y {4}..{5} -- the scene is not what was claimed" -f `
        $leaf, $Ink, $x0, $x1, $y0, $y1)
    exit 2
}

# pass 2: the brightest ground pixel that is not a glyph edge
$worstLum = -1.0; $worstR = 0; $worstG = 0; $worstB = 0; $worstX = 0; $worstY = 0
$groundCount = 0
for ($y = $y0; $y -lt $y1; $y++) {
    $row = $y * $stride
    $nrow = $y * $w
    for ($x = $x0; $x -lt $x1; $x++) {
        if ($near[$nrow + $x]) { continue }
        $i = $row + $x * 4
        $l = Lum256 $bytes[$i + 2] $bytes[$i + 1] $bytes[$i]
        $groundCount++
        if ($l -gt $worstLum) {
            $worstLum = $l; $worstR = $bytes[$i + 2]; $worstG = $bytes[$i + 1]
            $worstB = $bytes[$i]; $worstX = $x; $worstY = $y
        }
    }
}
$ratio = Ratio $inkLum $worstLum
$hex = "#{0:X2}{1:X2}{2:X2}" -f $worstR, $worstG, $worstB
$verdict = if ($ratio -ge $Require) { "PASS" } else { "FAIL" }
Write-Output ("{0,-28} window x {1}..{2} y {3}..{4}" -f $leaf, $x0, ($x1 - 1), $y0, ($y1 - 1))
Write-Output ("{0,-28} ink {1} {2} px, box x {3}..{4} y {5}..{6}" -f "", $Ink, $inkCount, $minX, $maxX, $minY, $maxY)
Write-Output ("{0,-28} worst ground {1} @ {2},{3} ({4:N4} L), {5} px measured" -f "", $hex, $worstX, $worstY, $worstLum, $groundCount)
Write-Output ("{0,-28} contrast {1:N2}:1 vs {2:N2} required -> {3}" -f "", $ratio, $Require, $verdict)
if ($verdict -eq "PASS") { exit 0 } else { exit 1 }
