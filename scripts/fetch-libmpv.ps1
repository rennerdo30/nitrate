# Downloads the pinned LGPL libmpv build for Windows x64, checks its SHA-256
# and unpacks it into -Destination. Prints the path of libmpv-2.dll.
# Nothing is installed; PATH is unchanged. Windows' own tar.exe unpacks 7z.
#
# Build: zhongfly/mpv-winbuild 2026-11-13-fe753a7824 (mpv fe753a7824), LGPL,
# x86-64-v3 (needs AVX2). It has the D3D11 render API (ADR 0001).
[CmdletBinding()]
param([string]$Destination = 'E:/Development/_cache/tools/libmpv')
$ErrorActionPreference = 'Stop'
$tag = '2026-11-13-fe753a7824'
$name = 'mpv-dev-lgpl-x86_64-v3-20261113-git-fe753a7824.7z'
$url = "https://github.com/zhongfly/mpv-winbuild/releases/download/$tag/$name"
$expected = '3A88988109F251541075A1EC36A82FAA3883A72BD55DB3D811EEAFAC708354C7'
$dir = Join-Path $Destination $tag
$dll = Join-Path $dir 'extracted/libmpv-2.dll'
if (Test-Path -LiteralPath $dll) { Write-Output $dll; return }
New-Item -ItemType Directory -Force -Path $dir | Out-Null
$archive = Join-Path $dir $name
if (!(Test-Path -LiteralPath $archive)) {
    & "$env:SystemRoot\System32\curl.exe" --fail --silent --show-error --location --retry 4 --retry-all-errors --max-time 600 --output $archive $url
    if ($LASTEXITCODE) { throw "libmpv download failed ($LASTEXITCODE)" }
}
$actual = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash
if ($actual -ne $expected) { Remove-Item -LiteralPath $archive; throw "libmpv checksum mismatch: $actual" }
$out = Join-Path $dir 'extracted'
New-Item -ItemType Directory -Force -Path $out | Out-Null
& "$env:SystemRoot\System32\tar.exe" -xf $archive -C $out
if ($LASTEXITCODE -or !(Test-Path -LiteralPath $dll)) { throw 'libmpv-2.dll missing after unpacking' }
Write-Output $dll
