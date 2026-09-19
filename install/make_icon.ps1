# Quire app icon — drawn here, not hand-made, so the asset is reproducible.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File install\make_icon.ps1
#
# Writes install\quire.ico (the exe/Start-menu icon: 16, 24, 32, 48, 64, 128
# and 256 px frames in one container) and install\quire.png (the 256 px frame,
# for Slint's `Window.icon`). Each frame is the same geometry scaled to its
# size, which is what keeps the ring readable at 16 px.
#
# The picture: the vertical gradient the app's dark theme uses, and a `Q`
# drawn as a ring plus a tail stroke, so nothing depends on a installed font.

Add-Type -AssemblyName System.Drawing

$sizes = 16, 24, 32, 48, 64, 128, 256
$top = [System.Drawing.Color]::FromArgb(255, 14, 27, 46)      # #0E1B2E
$bottom = [System.Drawing.Color]::FromArgb(255, 10, 36, 66)   # #0A2442
$ink = [System.Drawing.Color]::FromArgb(255, 245, 247, 250)   # near white
$accent = [System.Drawing.Color]::FromArgb(255, 94, 234, 212) # #5EEAD4

function New-FramePng([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap $size, $size, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)

    # rounded square: 22% corner radius, the same shape at every size
    $radius = $size * 0.22
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $radius * 2
    $path.AddArc(0, 0, $d, $d, 180, 90)
    $path.AddArc($size - $d, 0, $d, $d, 270, 90)
    $path.AddArc($size - $d, $size - $d, $d, $d, 0, 90)
    $path.AddArc(0, $size - $d, $d, $d, 90, 90)
    $path.CloseFigure()

    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        (New-Object System.Drawing.RectangleF(0, 0, $size, $size)),
        $top, $bottom, 90)
    $g.FillPath($brush, $path)
    $brush.Dispose(); $path.Dispose()

    # the Q: a ring centred a little high, and the tail through its lower right
    $stroke = [Math]::Max(1.5, $size * 0.10)
    $ring = $size * 0.30
    $cx = $size * 0.5
    $cy = $size * 0.46
    $pen = New-Object System.Drawing.Pen($ink, $stroke)
    $pen.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $pen.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $g.DrawEllipse($pen, ($cx - $ring), ($cy - $ring), ($ring * 2), ($ring * 2))

    $tail = New-Object System.Drawing.Pen($accent, $stroke)
    $tail.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $tail.EndCap = [System.Drawing.Drawing2D.LineCap]::Round
    $edge = [Math]::Sqrt(2) / 2
    $g.DrawLine($tail,
        ($cx + $ring * $edge * 0.7), ($cy + $ring * $edge * 1.1),
        ($cx + $ring * $edge * 2.0), ($cy + $ring * $edge * 2.2))
    $pen.Dispose(); $tail.Dispose()

    $g.Dispose()
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bytes = $ms.ToArray()
    $ms.Dispose(); $bmp.Dispose()
    ,$bytes
}

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
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
