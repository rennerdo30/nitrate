# Writes assets/logo/nitrate.ico (16, 24, 32, 48, 64 and 256 px, PNG entries)
# from assets/logo/nitrate-mark.svg, so the installer and window icon always
# match the SVG logo. Windows only: renders the SVG once at 1024 px with
# headless Microsoft Edge (bounded, its own PID only), then scales down.
#   scripts\make-icon.ps1
[CmdletBinding()]
param(
    [string]$Svg = (Join-Path (Split-Path -Parent $PSScriptRoot) 'assets/logo/nitrate-mark.svg'),
    [string]$Output = (Join-Path (Split-Path -Parent $PSScriptRoot) 'assets/logo/nitrate.ico'),
    [string]$Edge = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
if (!(Test-Path -LiteralPath $Edge)) { throw "Microsoft Edge not found at $Edge" }
$sizes = 16, 24, 32, 48, 64, 256
$tmp = Join-Path ([IO.Path]::GetTempPath()) ('nitrate-icon-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
    # A temporary copy drawn at 1024 px; the logo file itself is unchanged.
    $text = Get-Content -LiteralPath $Svg -Raw
    $text = $text -replace 'width="512" height="512">', 'width="1024" height="1024">'
    $copy = Join-Path $tmp 'mark-1024.svg'
    [IO.File]::WriteAllText($copy, $text)
    $big = Join-Path $tmp '1024.png'
    $edgeArgs = @('--headless=new', '--disable-gpu', '--hide-scrollbars',
        '--default-background-color=00000000', "--user-data-dir=$tmp\profile", '--window-size=1024,1024',
        "--screenshot=$big", ([Uri]$copy).AbsoluteUri)
    $p = Start-Process -FilePath $Edge -ArgumentList $edgeArgs -PassThru -WindowStyle Hidden
    Write-Host "Started PID $($p.Id): msedge (timeout 60 s)"
    if (!$p.WaitForExit(60000)) { $p.Kill($true); throw 'Edge timed out' }
    # The launcher can exit before its browser process writes the file.
    $deadline = (Get-Date).AddSeconds(60)
    $last = -1
    while ((Get-Date) -lt $deadline) {
        if (Test-Path -LiteralPath $big) {
            $len = (Get-Item -LiteralPath $big).Length
            if ($len -gt 0 -and $len -eq $last) { break }
            $last = $len
        }
        Start-Sleep -Milliseconds 500
    }
    if (!(Test-Path -LiteralPath $big)) { throw 'Edge wrote no screenshot within 60 s' }

    $source = [Drawing.Image]::FromFile($big)
    try {
        $entries = foreach ($size in $sizes) {
            $bmp = [Drawing.Bitmap]::new($size, $size, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
            $g = [Drawing.Graphics]::FromImage($bmp)
            $g.InterpolationMode = [Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $g.PixelOffsetMode = [Drawing.Drawing2D.PixelOffsetMode]::HighQuality
            $g.CompositingQuality = [Drawing.Drawing2D.CompositingQuality]::HighQuality
            $g.Clear([Drawing.Color]::Transparent)
            $g.DrawImage($source, 0, 0, $size, $size)
            $g.Dispose()
            $ms = [IO.MemoryStream]::new()
            $bmp.Save($ms, [Drawing.Imaging.ImageFormat]::Png)
            $bmp.Dispose()
            , $ms.ToArray()
        }
    } finally { $source.Dispose() }

    $ms = [IO.MemoryStream]::new()
    $w = [IO.BinaryWriter]::new($ms)
    $w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
    $offset = 6 + 16 * $sizes.Count
    for ($i = 0; $i -lt $sizes.Count; $i++) {
        $dim = if ($sizes[$i] -ge 256) { 0 } else { $sizes[$i] }
        $w.Write([byte]$dim); $w.Write([byte]$dim); $w.Write([byte]0); $w.Write([byte]0)
        $w.Write([uint16]1); $w.Write([uint16]32)
        $w.Write([uint32]$entries[$i].Length); $w.Write([uint32]$offset)
        $offset += $entries[$i].Length
    }
    foreach ($bytes in $entries) { $w.Write($bytes) }
    $w.Flush()
    [IO.File]::WriteAllBytes($Output, $ms.ToArray())
    Write-Host "Wrote $Output ($($ms.Length) bytes)"
} finally {
    Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
