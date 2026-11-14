# Retakes the screenshots in docs/images (README and website) from the real player.
#
#   pwsh scripts/screenshots.ps1
#
# Renders Nitrate's own demo footage once (scripts/demo-footage.py, outside Git), builds the
# release player, and takes muted, offscreen --snapshot captures: playing with the timeline
# hover, paused, and the empty window. Then converts them to 1600x900 WebP.
# Needs Python with numpy and Pillow, and ffmpeg on PATH. Each run has a 60 s timeout.
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$cache = 'E:/Development/_cache/nitrate-media'
$video = "$cache/demo/Ember Ridge.mp4"
$raw = "$cache/shots"
$out = Join-Path $root 'docs/images'
New-Item -ItemType Directory -Force $raw, $out | Out-Null

if (-not (Test-Path $video)) {
    python (Join-Path $PSScriptRoot 'demo-footage.py') "$cache/demo"
    if ($LASTEXITCODE) { throw 'demo footage failed' }
}

Push-Location $root
try {
    cargo build --release -p nitrate
    if ($LASTEXITCODE) { throw 'build failed' }
    $meta = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
    $exe = Join-Path $meta.target_directory 'release/nitrate.exe'
} finally { Pop-Location }

function Shot([string]$name, [string[]]$extra) {
    $png = "$raw/$name.png"
    Remove-Item $png -ErrorAction SilentlyContinue
    $argv = @('--mute') + $extra + @('--snapshot', $png)
    $p = Start-Process -FilePath $exe -ArgumentList ($argv | ForEach-Object { if ($_ -match ' ') { "`"$_`"" } else { $_ } }) -PassThru -NoNewWindow
    if (-not $p.WaitForExit(60000)) { $p.Kill(); throw "${name} timed out" }
    if (-not (Test-Path $png)) { throw "${name}: no snapshot" }
}

Shot 'playing' @('--start', '383', '--hover', '0.71', $video)
Shot 'paused' @('--pause', '--start', '211', $video)
Shot 'idle' @()

$py = @'
import sys
from PIL import Image
raw, out = sys.argv[1], sys.argv[2]
for n in ("playing", "paused", "idle"):
    im = Image.open(f"{raw}/{n}.png").convert("RGB").resize((1600, 900), Image.LANCZOS)
    im.save(f"{out}/{n}.webp", "WEBP", quality=86, method=6)
'@
python -c $py $raw $out
if ($LASTEXITCODE) { throw 'conversion failed' }
Write-Host "Screenshots written to $out"
# The website keeps its own copy (it deploys website/ only).
Copy-Item "$out/*.webp" (Join-Path $root 'website/assets/shots') -Force
