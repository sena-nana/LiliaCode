# Lilia 应用图标生成器
#
# 设计：六臂雪花叠在 6 角星底纹上，淡蓝色调，1024×1024 透明背景。
# 用法：
#   pwsh -File scripts/generate-icon.ps1
# 之后想要全平台图标集时（含 icns 等）：
#   yarn tauri icon apps/desktop/src-tauri/icons/icon-source.png

Add-Type -AssemblyName System.Drawing

$ErrorActionPreference = "Stop"

$iconsDir = Join-Path $PSScriptRoot "..\apps\desktop\src-tauri\icons"
if (-not (Test-Path $iconsDir)) {
    New-Item -ItemType Directory -Path $iconsDir | Out-Null
}
$iconsDir = (Resolve-Path $iconsDir).Path

# ---------- 颜色 ----------
$colDeep   = [System.Drawing.Color]::FromArgb(255,  73, 145, 215)  # #4991D7
$colMid    = [System.Drawing.Color]::FromArgb(255, 123, 185, 240)  # #7BB9F0
$colLight  = [System.Drawing.Color]::FromArgb(255, 185, 219, 247)  # #B9DBF7
$colGlow   = [System.Drawing.Color]::FromArgb(180, 185, 219, 247)  # 半透明描边

# ---------- 画一张源图 ----------
function New-LiliaIcon([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)

    $cx = [single]($size / 2.0)
    $cy = [single]($size / 2.0)
    $s  = $size / 1024.0  # scale factor

    # ---- 底层 6 角星（"星星"语义）----
    $starOuter = 380.0 * $s
    $starInner = 150.0 * $s
    $pts = New-Object 'System.Collections.Generic.List[System.Drawing.PointF]'
    for ($i = 0; $i -lt 12; $i++) {
        $r = if ($i % 2 -eq 0) { $starOuter } else { $starInner }
        $angle = -90.0 + $i * 30.0
        $rad = $angle * [Math]::PI / 180.0
        $px = $cx + [single]($r * [Math]::Cos($rad))
        $py = $cy + [single]($r * [Math]::Sin($rad))
        $pts.Add((New-Object System.Drawing.PointF($px, $py)))
    }
    $starPath = New-Object System.Drawing.Drawing2D.GraphicsPath
    $starPath.AddPolygon($pts.ToArray())

    $starBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        (New-Object System.Drawing.PointF(0, 0)),
        (New-Object System.Drawing.PointF([single]$size, [single]$size)),
        $colLight,
        $colMid
    )
    $g.FillPath($starBrush, $starPath)

    # 星形的柔和描边，让边缘更立体
    $starPen = New-Object System.Drawing.Pen($colGlow, [single](14.0 * $s))
    $starPen.LineJoin = [System.Drawing.Drawing2D.LineJoin]::Round
    $g.DrawPath($starPen, $starPath)

    # ---- 上层 6 臂雪花（"雪花"语义）----
    $armLength = 420.0 * $s
    $armWidth  = 30.0 * $s
    $barbWidth = 18.0 * $s

    $penArm = New-Object System.Drawing.Pen($colDeep, [single]$armWidth)
    $penArm.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $penArm.EndCap   = [System.Drawing.Drawing2D.LineCap]::Round

    $penBarb = New-Object System.Drawing.Pen($colDeep, [single]$barbWidth)
    $penBarb.StartCap = [System.Drawing.Drawing2D.LineCap]::Round
    $penBarb.EndCap   = [System.Drawing.Drawing2D.LineCap]::Round

    $barbAngle = 55.0 * [Math]::PI / 180.0
    $barbCos = [Math]::Cos($barbAngle)
    $barbSin = [Math]::Sin($barbAngle)

    for ($i = 0; $i -lt 6; $i++) {
        $state = $g.Save()
        $g.TranslateTransform($cx, $cy)
        # 让臂指向星形的尖角：星起始角 -90，6 角间隔 60
        $g.RotateTransform([single]($i * 60.0 - 90.0))

        # 主臂
        $g.DrawLine($penArm, [single]0, [single]0, [single]$armLength, [single]0)

        # 外侧分叉
        $b1Pos = $armLength * 0.58
        $b1Len = 130.0 * $s
        $b1ex = $b1Pos + [single]($b1Len * $barbCos)
        $b1ey = [single]($b1Len * $barbSin)
        $g.DrawLine($penBarb, [single]$b1Pos, [single]0, [single]$b1ex, [single]$b1ey)
        $g.DrawLine($penBarb, [single]$b1Pos, [single]0, [single]$b1ex, [single](-$b1ey))

        # 内侧分叉（更短）
        $b2Pos = $armLength * 0.32
        $b2Len = 90.0 * $s
        $b2ex = $b2Pos + [single]($b2Len * $barbCos)
        $b2ey = [single]($b2Len * $barbSin)
        $g.DrawLine($penBarb, [single]$b2Pos, [single]0, [single]$b2ex, [single]$b2ey)
        $g.DrawLine($penBarb, [single]$b2Pos, [single]0, [single]$b2ex, [single](-$b2ey))

        # 臂尖一个亮点，像星芒
        $tipR = 22.0 * $s
        $tipBrush = New-Object System.Drawing.SolidBrush($colLight)
        $g.FillEllipse($tipBrush, [single]($armLength - $tipR), [single](-$tipR), [single]($tipR * 2), [single]($tipR * 2))
        $tipBrush.Dispose()

        $g.Restore($state)
    }

    $penArm.Dispose()
    $penBarb.Dispose()
    $starPen.Dispose()
    $starBrush.Dispose()
    $starPath.Dispose()

    # ---- 中心 hub ----
    $hubR = 55.0 * $s
    $hubBrush = New-Object System.Drawing.SolidBrush($colLight)
    $g.FillEllipse($hubBrush, [single]($cx - $hubR), [single]($cy - $hubR), [single]($hubR * 2), [single]($hubR * 2))
    $hubBrush.Dispose()

    $dotR = 22.0 * $s
    $dotBrush = New-Object System.Drawing.SolidBrush($colDeep)
    $g.FillEllipse($dotBrush, [single]($cx - $dotR), [single]($cy - $dotR), [single]($dotR * 2), [single]($dotR * 2))
    $dotBrush.Dispose()

    $g.Dispose()
    return $bmp
}

function Save-Png([System.Drawing.Bitmap]$bmp, [string]$path) {
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Output "  -> $path"
}

# ---- 缩放工具：高质量降采样 ----
function Resize-Bitmap([System.Drawing.Bitmap]$src, [int]$w, [int]$h) {
    $dst = New-Object System.Drawing.Bitmap($w, $h, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($dst)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.DrawImage($src, (New-Object System.Drawing.Rectangle(0, 0, $w, $h)))
    $g.Dispose()
    return $dst
}

Write-Output "Generating Lilia icons -> $iconsDir"

# 直接按目标尺寸重绘，描边比例更自然
$source = New-LiliaIcon -size 1024
Save-Png $source (Join-Path $iconsDir "icon-source.png")
Save-Png $source (Join-Path $iconsDir "icon.png")

$icon256 = New-LiliaIcon -size 256
Save-Png $icon256 (Join-Path $iconsDir "128x128@2x.png")

$icon128 = New-LiliaIcon -size 128
Save-Png $icon128 (Join-Path $iconsDir "128x128.png")

$icon32 = New-LiliaIcon -size 32
Save-Png $icon32 (Join-Path $iconsDir "32x32.png")

# ---- 手写一个多尺寸 ICO（嵌入 PNG 项）----
$icoSizes = @(16, 32, 48, 64, 128, 256)
$icoBitmaps = @{}
foreach ($sz in $icoSizes) {
    $icoBitmaps[$sz] = New-LiliaIcon -size $sz
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

# 计算各项数据偏移
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
    $bw.Write([byte]$w)            # width
    $bw.Write([byte]$h)            # height
    $bw.Write([byte]0)             # color count
    $bw.Write([byte]0)             # reserved
    $bw.Write([UInt16]1)           # color planes
    $bw.Write([UInt16]32)          # bits per pixel
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
$icon256.Dispose()
$icon128.Dispose()
$icon32.Dispose()
foreach ($sz in $icoSizes) { $icoBitmaps[$sz].Dispose() }

Write-Output "Done."
