# Builds Nitrate from this checkout, puts libmpv-2.dll next to the exe and
# starts it. Used by run.bat; see there for the options.
[CmdletBinding()]
param(
    [switch]$Release,
    [switch]$Pull,
    [switch]$Wait,
    [Parameter(ValueFromRemainingArguments)][string[]]$Rest
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $root
if ($Pull) {
    git pull --ff-only origin main
    if ($LASTEXITCODE) { throw 'Pull failed. Fix the checkout first.' }
}
$jobs = [Math]::Max(1, [Math]::Min(8, [int]([Environment]::ProcessorCount / 2)))
$cargoArgs = @('build', '-j', "$jobs", '--quiet', '-p', 'nitrate')
if ($Release) { $cargoArgs += '--release' }
Write-Host "Building Nitrate $(if ($Release) { '(release)' } else { '(debug)' })..."
& cargo @cargoArgs
if ($LASTEXITCODE) { throw 'Build failed.' }
$meta = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json
$bin = Join-Path $meta.target_directory $(if ($Release) { 'release' } else { 'debug' })
$exe = Join-Path $bin 'nitrate.exe'

# libmpv: NITRATE_LIBMPV, else the pinned build (downloaded once).
$src = $env:NITRATE_LIBMPV
if (!$src) { $src = & (Join-Path $PSScriptRoot 'fetch-libmpv.ps1') }
$dst = Join-Path $bin 'libmpv-2.dll'
if (!(Test-Path -LiteralPath $dst) -or (Get-Item -LiteralPath $src).Length -ne (Get-Item -LiteralPath $dst).Length) {
    Copy-Item -LiteralPath $src -Destination $dst -Force
}

if ($env:NITRATE_RUN_DRY_RUN) { Write-Host "Would start: $exe $Rest (libmpv: $dst)"; return }
$p = Start-Process -FilePath $exe -ArgumentList $Rest -PassThru
Write-Host "Started Nitrate (PID $($p.Id))."
if ($Wait) { $p.WaitForExit(); exit $p.ExitCode }
