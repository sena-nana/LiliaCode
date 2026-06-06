# Lilia 应用图标生成器
#
# 设计：从 apps/desktop/src-tauri/icons/icon.svg（设计稿，PNG 嵌入式 SVG 容器）
#       栅格化出全套尺寸：1024 / 256 / 128 / 32 PNG + 多尺寸 ICO。
#       SVG 中的 <image href="data:image/png;base64,..."> 会被解码后用 GDI+
#       高质量重采样到目标尺寸。
# 用法：
#   yarn icons:generate
# 若想要全平台图标集（含 .icns 等）：
#   yarn icons:tauri

Add-Type -AssemblyName System.Drawing

$ErrorActionPreference = "Stop"

$iconsDir = Join-Path $PSScriptRoot "..\apps\desktop\src-tauri\icons"
if (-not (Test-Path $iconsDir)) {
    throw "Icons directory not found: $iconsDir"
}
$iconsDir = (Resolve-Path $iconsDir).Path

$svgPath = Join-Path $iconsDir "icon.svg"
if (-not (Test-Path $svgPath)) {
    throw "Source SVG not found: $svgPath"
}

# ---------- 从 SVG 容器里提取嵌入的 base64 位图 ----------
function Get-SvgEmbeddedImage([string]$path) {
    $content = Get-Content -LiteralPath $path -Raw
    $pattern = 'data:image/(?<fmt>png|jpeg|jpg|webp);base64,(?<data>[A-Za-z0-9+/=]+)'
    $match = [regex]::Match($content, $pattern)
    if (-not $match.Success) {
        throw "icon.svg 没有内嵌的 base64 位图（data:image/...;base64,...）。当前脚本只支持 PNG/JPEG/WebP-embedded SVG 容器；如需渲染纯矢量 SVG 请改走 sharp / inkscape / resvg 等工具链。"
    }
    $bytes = [Convert]::FromBase64String($match.Groups['data'].Value)
    $ms = New-Object System.IO.MemoryStream(, $bytes)
    return [System.Drawing.Bitmap]::FromStream($ms)
}

# ---------- 高质量重采样 ----------
function Resize-Bitmap([System.Drawing.Bitmap]$src, [int]$w, [int]$h) {
    $dst = New-Object System.Drawing.Bitmap($w, $h, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($dst)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.DrawImage($src, (New-Object System.Drawing.Rectangle(0, 0, $w, $h)))
    $g.Dispose()
    return $dst
}

function Save-Png([System.Drawing.Bitmap]$bmp, [string]$path) {
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Output "  -> $path"
}

Write-Output "Generating Lilia icons from $svgPath"

$source = Get-SvgEmbeddedImage $svgPath
Write-Output "  source: $($source.Width) x $($source.Height)"

# 1024：icon-source.png + icon.png
$icon1024 = Resize-Bitmap $source 1024 1024
Save-Png $icon1024 (Join-Path $iconsDir "icon-source.png")
Save-Png $icon1024 (Join-Path $iconsDir "icon.png")

$icon256 = Resize-Bitmap $source 256 256
Save-Png $icon256 (Join-Path $iconsDir "128x128@2x.png")

$icon128 = Resize-Bitmap $source 128 128
Save-Png $icon128 (Join-Path $iconsDir "128x128.png")

$icon32 = Resize-Bitmap $source 32 32
Save-Png $icon32 (Join-Path $iconsDir "32x32.png")

# ---- 手写一个多尺寸 ICO（嵌入 PNG 项）----
$icoSizes = @(16, 32, 48, 64, 128, 256)
$icoBitmaps = @{}
foreach ($sz in $icoSizes) {
    $icoBitmaps[$sz] = Resize-Bitmap $source $sz $sz
}

$pngBytesPerSize = @{}
foreach ($sz in $icoSizes) {
    $ms = New-Object System.IO.MemoryStream
    $icoBitmaps[$sz].Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $pngBytesPerSize[$sz] = $ms.ToArray()
    $ms.Dispose()
}

$icoPath = Join-Path $iconsDir "icon.ico"
$fs = [System.IO.File]::Open($icoPath, [System.IO.FileMode]::Create)
$bw = New-Object System.IO.BinaryWriter($fs)

# ICONDIR
$bw.Write([UInt16]0)              # reserved
$bw.Write([UInt16]1)              # type = ICO
$bw.Write([UInt16]$icoSizes.Count)# count

# 各项数据偏移
$headerSize = 6 + 16 * $icoSizes.Count
$offsets = @{}
$running = $headerSize
foreach ($sz in $icoSizes) {
    $offsets[$sz] = $running
    $running += $pngBytesPerSize[$sz].Length
}

# ICONDIRENTRY × N
foreach ($sz in $icoSizes) {
    $w = if ($sz -ge 256) { 0 } else { $sz }   # 256 → 0 (ICO 约定)
    $h = $w
    $bw.Write([byte]$w)
    $bw.Write([byte]$h)
    $bw.Write([byte]0)
    $bw.Write([byte]0)
    $bw.Write([UInt16]1)
    $bw.Write([UInt16]32)
    $bw.Write([UInt32]$pngBytesPerSize[$sz].Length)
    $bw.Write([UInt32]$offsets[$sz])
}

# 图像数据
foreach ($sz in $icoSizes) {
    $bw.Write($pngBytesPerSize[$sz])
}

$bw.Flush()
$bw.Dispose()
$fs.Dispose()
Write-Output "  -> $icoPath"

# 清理
$source.Dispose()
$icon1024.Dispose()
$icon256.Dispose()
$icon128.Dispose()
$icon32.Dispose()
foreach ($sz in $icoSizes) { $icoBitmaps[$sz].Dispose() }

Write-Output "Done."
