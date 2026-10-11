# Gera assets\app.ico (256, 48, 32 e 16 px): quadrado roxo arredondado com um mouse branco,
# o mesmo desenho do ícone da janela (ui::app_icon_rgba).
Add-Type -AssemblyName System.Drawing
$out = Join-Path $PSScriptRoot '..\assets'
New-Item -ItemType Directory -Force $out | Out-Null

function Draw([int]$s) {
    $bmp = New-Object Drawing.Bitmap($s, $s, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.Clear([Drawing.Color]::Transparent)
    $r = $s * 0.22
    $path = New-Object Drawing.Drawing2D.GraphicsPath
    $path.AddArc(0, 0, 2 * $r, 2 * $r, 180, 90)
    $path.AddArc($s - 2 * $r, 0, 2 * $r, 2 * $r, 270, 90)
    $path.AddArc($s - 2 * $r, $s - 2 * $r, 2 * $r, 2 * $r, 0, 90)
    $path.AddArc(0, $s - 2 * $r, 2 * $r, 2 * $r, 90, 90)
    $path.CloseFigure()
    $g.FillPath((New-Object Drawing.SolidBrush ([Drawing.Color]::FromArgb(255, 0x61, 0x33, 0xc9))), $path)
    $cx = $s * 0.5; $cy = $s * 0.52; $ex = $s * 0.23; $ey = $s * 0.32
    $g.FillEllipse([Drawing.Brushes]::White, [single]($cx - $ex), [single]($cy - $ey), [single](2 * $ex), [single](2 * $ey))
    $pen = New-Object Drawing.Pen ([Drawing.Color]::FromArgb(255, 0x61, 0x33, 0xc9)), ([single]($s * 0.05))
    $g.DrawLine($pen, [single]$cx, [single]($cy - $ey), [single]$cx, [single]($cy - $ey * 0.15))
    $g.DrawLine($pen, [single]($cx - $ex), [single]($cy - $ey * 0.15), [single]($cx + $ex), [single]($cy - $ey * 0.15))
    $g.Dispose()
    $ms = New-Object IO.MemoryStream
    $bmp.Save($ms, [Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    return , $ms.ToArray()
}

$sizes = 256, 48, 32, 16
$pngs = foreach ($s in $sizes) { , (Draw $s) }
$ico = New-Object IO.MemoryStream
$w = New-Object IO.BinaryWriter($ico)
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $s = $sizes[$i]; $len = $pngs[$i].Length
    $w.Write([byte]($(if ($s -ge 256) { 0 } else { $s }))); $w.Write([byte]($(if ($s -ge 256) { 0 } else { $s })))
    $w.Write([byte]0); $w.Write([byte]0); $w.Write([uint16]1); $w.Write([uint16]32)
    $w.Write([uint32]$len); $w.Write([uint32]$offset)
    $offset += $len
}
foreach ($p in $pngs) { $w.Write($p) }
$w.Flush()
[IO.File]::WriteAllBytes((Join-Path $out 'app.ico'), $ico.ToArray())
"app.ico: $($ico.Length) bytes"
