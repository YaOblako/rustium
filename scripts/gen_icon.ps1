Add-Type -AssemblyName System.Drawing
New-Item -ItemType Directory -Force -Path "assets" | Out-Null

function Draw-RustiumIcon([int]$sz) {
    $bmp = New-Object System.Drawing.Bitmap($sz, $sz, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.Clear([System.Drawing.Color]::Transparent)

    # Background squircle
    $r = [float]($sz * 0.28)
    $d = $r * 2.0
    $w = [float]($sz - 2)
    $h = [float]($sz - 2)

    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc(1.0, 1.0, $d, $d, 180.0, 90.0)
    $path.AddArc(1.0 + $w - $d, 1.0, $d, $d, 270.0, 90.0)
    $path.AddArc(1.0 + $w - $d, 1.0 + $h - $d, $d, $d, 0.0, 90.0)
    $path.AddArc(1.0, 1.0 + $h - $d, $d, $d, 90.0, 90.0)
    $path.CloseFigure()

    # Gradient brush
    $colorStart = [System.Drawing.Color]::FromArgb(255, 255, 87, 34)   # Rust orange
    $colorEnd = [System.Drawing.Color]::FromArgb(255, 124, 77, 255)   # Purple accent
    $rectF = New-Object System.Drawing.RectangleF(0.0, 0.0, [float]$sz, [float]$sz)
    $brush = New-Object System.Drawing.Drawing2D.LinearGradientBrush($rectF, $colorStart, $colorEnd, [System.Drawing.Drawing2D.LinearGradientMode]::ForwardDiagonal)
    $g.FillPath($brush, $path)

    # Draw flame accent
    $flameColor = [System.Drawing.Color]::FromArgb(90, 255, 180, 0)
    $flameBrush = New-Object System.Drawing.SolidBrush($flameColor)
    $flamePath = New-Object System.Drawing.Drawing2D.GraphicsPath
    $flamePath.AddEllipse([float]($sz * 0.1), [float]($sz * 0.35), [float]($sz * 0.8), [float]($sz * 0.55))
    $g.FillPath($flameBrush, $flamePath)

    # Draw Letter 'R'
    $fontSize = [float]($sz * 0.54)
    $font = New-Object System.Drawing.Font('Arial', $fontSize, [System.Drawing.FontStyle]::Bold)
    $sf = New-Object System.Drawing.StringFormat
    $sf.Alignment = [System.Drawing.StringAlignment]::Center
    $sf.LineAlignment = [System.Drawing.StringAlignment]::Center
    $textBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
    $textRect = New-Object System.Drawing.RectangleF(0.0, [float]($sz * 0.03), [float]$sz, [float]$sz)
    $g.DrawString('R', $font, $textBrush, $textRect, $sf)

    $brush.Dispose()
    $flameBrush.Dispose()
    $textBrush.Dispose()
    $font.Dispose()
    $g.Dispose()
    return $bmp
}

$sizes = @(16, 32, 48, 64, 128, 256)
$entries = @()

foreach ($s in $sizes) {
    $bmp = Draw-RustiumIcon -sz $s
    if ($s -ge 256) {
        $ms = New-Object System.IO.MemoryStream
        $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
        $data = $ms.ToArray()
        $ms.Dispose()
        $entries += ,@($s, $data)
    } else {
        $ms = New-Object System.IO.MemoryStream
        $entryBw = New-Object System.IO.BinaryWriter($ms)

        # BITMAPINFOHEADER (40 bytes)
        $entryBw.Write([uint32]40)           # biSize
        $entryBw.Write([int32]$s)            # biWidth
        $entryBw.Write([int32]($s * 2))      # biHeight (XOR mask + AND mask)
        $entryBw.Write([uint16]1)            # biPlanes
        $entryBw.Write([uint16]32)           # biBitCount (32 bpp ARGB)
        $entryBw.Write([uint32]0)            # biCompression (BI_RGB)
        
        $andRowBytes = [int]([Math]::Ceiling($s / 32.0) * 4)
        $andMaskSize = $andRowBytes * $s
        $xorSize = $s * $s * 4
        $entryBw.Write([uint32]($xorSize + $andMaskSize)) # biSizeImage
        $entryBw.Write([int32]0)             # biXPelsPerMeter
        $entryBw.Write([int32]0)             # biYPelsPerMeter
        $entryBw.Write([uint32]0)            # biClrUsed
        $entryBw.Write([uint32]0)            # biClrImportant

        # XOR mask: 32bpp BGRA from bottom row to top row
        for ($y = $s - 1; $y -ge 0; $y--) {
            for ($x = 0; $x -lt $s; $x++) {
                $px = $bmp.GetPixel($x, $y)
                $entryBw.Write([byte]$px.B)
                $entryBw.Write([byte]$px.G)
                $entryBw.Write([byte]$px.R)
                $entryBw.Write([byte]$px.A)
            }
        }

        # AND mask: 1bpp, all 0s
        $andBytes = New-Object byte[] $andMaskSize
        $entryBw.Write($andBytes)

        $entryBw.Flush()
        $data = $ms.ToArray()
        $ms.Dispose()
        $entries += ,@($s, $data)
    }
    $bmp.Dispose()
}

$outPath = Join-Path (Get-Location) "assets\icon.ico"
$fs = [System.IO.File]::Create($outPath)
$fileBw = New-Object System.IO.BinaryWriter($fs)

$fileBw.Write([uint16]0) # Reserved
$fileBw.Write([uint16]1) # Type: 1 = ICO
$fileBw.Write([uint16]$entries.Count)

$offset = 6 + ($entries.Count * 16)
foreach ($entry in $entries) {
    $s = $entry[0]
    $data = $entry[1]
    $entryWidthByte = if ($s -ge 256) { [byte]0 } else { [byte]$s }
    $entryHeightByte = if ($s -ge 256) { [byte]0 } else { [byte]$s }

    $fileBw.Write($entryWidthByte)
    $fileBw.Write($entryHeightByte)
    $fileBw.Write([byte]0)   # Colors
    $fileBw.Write([byte]0)   # Reserved
    $fileBw.Write([uint16]1) # Planes
    $fileBw.Write([uint16]32)# BitCount
    $fileBw.Write([uint32]$data.Length)
    $fileBw.Write([uint32]$offset)

    $offset += $data.Length
}

foreach ($entry in $entries) {
    $fileBw.Write($entry[1])
}

$fileBw.Flush()
$fs.Close()

Write-Output "SUCCESS: Icon written to $outPath ($((Get-Item $outPath).Length) bytes)"

# Generate raw 32x32 RGBA for tao window icon
$bmp32 = Draw-RustiumIcon -sz 32
$rgbaBytes = New-Object byte[] (32 * 32 * 4)
$idx = 0
for ($y = 0; $y -lt 32; $y++) {
    for ($x = 0; $x -lt 32; $x++) {
        $pixel = $bmp32.GetPixel($x, $y)
        $rgbaBytes[$idx++] = $pixel.R
        $rgbaBytes[$idx++] = $pixel.G
        $rgbaBytes[$idx++] = $pixel.B
        $rgbaBytes[$idx++] = $pixel.A
    }
}
[System.IO.File]::WriteAllBytes((Join-Path (Get-Location) "assets\icon_32.rgba"), $rgbaBytes)
$bmp32.Dispose()
Write-Output "SUCCESS: assets/icon_32.rgba written"

# Verification with System.Drawing.Icon
$testIcon = New-Object System.Drawing.Icon($outPath)
Write-Output "VERIFIED: System.Drawing.Icon loaded successfully! Size = $($testIcon.Width)x$($testIcon.Height)"
$testIcon.Dispose()
