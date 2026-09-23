# Quire app icon — rasterised from the artwork in this folder, not drawn here.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\make_icon.ps1
#
# install\icon.svg is the artwork's source and install\icon_master_1024.png is
# its 1024 px raster: a document glyph on a **transparent** background — only
# the glyph's own pixels are opaque, so no frame ships a plate behind it. Every
# size below is that one picture scaled, which is what keeps them agreeing.
#
# Writes:
#   install\quire.ico      the exe/Start-menu icon: 16, 24, 32, 48, 64, 128 and
#                          256 px frames in one container. The installer
#                          (`quire.iss`) and `build.rs` both read this one file,
#                          so the setup icon and the exe icon cannot drift.
#   install\quire.png      the 256 px frame. Nothing in the shell reads it —
#                          Slint 1.18 has no Window::set_icon — but the
#                          installer check and the docs name it.

Add-Type -AssemblyName System.Drawing

$here = Split-Path -Parent $MyInvocation.MyCommand.Path

$masterPath = Join-Path $here 'icon_master_1024.png'
if (-not (Test-Path $masterPath)) { throw "no artwork at $masterPath" }
$master = [System.Drawing.Bitmap]::FromFile($masterPath)

function New-Canvas([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceOver
    $g.Clear([System.Drawing.Color]::Transparent)
    ,@($bmp, $g)
}

function Save-Png($bmp) {
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bytes = $ms.ToArray()
    $ms.Dispose(); $bmp.Dispose()
    ,$bytes
}

# The frame: the picture, edge to edge, on a transparent square. Windows draws
# these itself (the shell paints its own tile behind an icon), so nothing is
# added here but the art.
function New-FramePng([int]$size) {
    $c = New-Canvas $size
    $bmp = $c[0]; $g = $c[1]
    $g.DrawImage($master, (New-Object System.Drawing.Rectangle 0, 0, $size, $size))
    $g.Dispose()
    Save-Png $bmp
}

$sizes = 16, 24, 32, 48, 64, 128, 256
$frames = @{}
foreach ($size in $sizes) {
    $frames[$size] = New-FramePng $size
    "  {0,3}px  {1,6:n0} KB png" -f $size, ($frames[$size].Length / 1KB)
}

# .ico: a 6-byte header, one 16-byte directory entry per frame, then the PNG
# payloads back to back. Windows has read PNG-compressed entries since Vista,
# and the target here is the Windows 10/11 shell.
$out = Join-Path $here 'quire.ico'
$fs = New-Object System.IO.FileStream $out, ([System.IO.FileMode]::Create)
$w = New-Object System.IO.BinaryWriter $fs
$w.Write([UInt16]0); $w.Write([UInt16]1); $w.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
foreach ($size in $sizes) {
    $bytes = $frames[$size]
    $w.Write([Byte]($(if ($size -ge 256) { 0 } else { $size })))  # width, 0 = 256
    $w.Write([Byte]($(if ($size -ge 256) { 0 } else { $size })))
    $w.Write([Byte]0); $w.Write([Byte]0)                          # palette, reserved
    $w.Write([UInt16]1)                                           # planes
    $w.Write([UInt16]32)                                         # bit depth
    $w.Write([UInt32]$bytes.Length)
    $w.Write([UInt32]$offset)
    $offset += $bytes.Length
}
foreach ($size in $sizes) { $w.Write($frames[$size]) }
$w.Flush(); $w.Close(); $fs.Close()

$png = Join-Path $here 'quire.png'
[System.IO.File]::WriteAllBytes($png, $frames[256])

"wrote {0} ({1:n0} bytes) and {2} ({3:n0} bytes)" -f `
    $out, (Get-Item $out).Length, $png, (Get-Item $png).Length

# self-check: the shell must be able to read the container back
$probe = New-Object System.Drawing.Icon $out, 32, 32
"icon loads: {0}x{1}" -f $probe.Width, $probe.Height
$probe.Dispose()
$master.Dispose()
