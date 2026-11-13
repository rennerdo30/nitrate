# Installer lifecycle test. CI ONLY: it installs Nitrate for the current user,
# writes HKCU and %LOCALAPPDATA%, and uninstalls again. It refuses to run
# outside GitHub Actions so it can never touch a real profile.
#   scripts/test-installer.ps1 -Setup <nitrate-x.y.z-setup.exe> -Version <x.y.z>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Setup,
    [Parameter(Mandatory)][string]$Version
)
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'test-installer.ps1 runs only on disposable CI runners.' }

$dir = Join-Path $env:LOCALAPPDATA 'Programs\nitrate'
$exe = Join-Path $dir 'nitrate.exe'
$key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\nitrate'
# Keep in step with TYPES in installer/nitrate.nsi.
$types = 'mkv mp4 m4v mov webm avi wmv flv ts m2ts mts mpg mpeg ogv 3gp mp3 flac m4a ogg opus wav'.Split(' ')
$progid = 'HKCU:\Software\Classes\Nitrate.Media'
$caps = 'HKCU:\Software\Nitrate\Capabilities\FileAssociations'
$registered = 'HKCU:\Software\RegisteredApplications'
$mkv = 'HKCU:\Software\Classes\.mkv\OpenWithProgids'
$other = 'Other.Player'

function Run([string]$File, [string[]]$Arguments, [int]$Seconds = 120) {
    $p = Start-Process -FilePath $File -ArgumentList $Arguments -PassThru
    if (!$p.WaitForExit($Seconds * 1000)) { $p.Kill(); throw "$File $Arguments timed out" }
    return $p.ExitCode
}

# Runs the installed exe with one option; returns (exit code, stdout).
function Query([string]$Option) {
    $out = Join-Path $env:RUNNER_TEMP "nitrate$Option.txt"
    $p = Start-Process -FilePath $exe -ArgumentList $Option -RedirectStandardOutput $out -PassThru -NoNewWindow
    if (!$p.WaitForExit(30000)) { $p.Kill(); throw "nitrate $Option timed out" }
    return $p.ExitCode, (Get-Content -LiteralPath $out -Raw).Trim()
}

function Value([string]$Path, [string]$Name) {
    $item = Get-ItemProperty -LiteralPath $Path -ErrorAction SilentlyContinue
    if ($null -eq $item) { return $null }
    return $item.$Name
}

function Assert-Installed {
    if (!(Test-Path -LiteralPath $exe)) { throw "nitrate.exe missing in $dir" }
    foreach ($f in 'libmpv-2.dll', 'LICENSE.txt', 'THIRD-PARTY-NOTICES.txt', 'libmpv-NOTICE.txt', 'LGPL-2.1.txt', 'uninstall.exe') {
        if (!(Test-Path -LiteralPath (Join-Path $dir $f))) { throw "$f missing" }
    }
    if ((Value $key 'DisplayVersion') -ne $Version) { throw "DisplayVersion is '$(Value $key 'DisplayVersion')', expected $Version" }
    if (!(Test-Path -LiteralPath (Join-Path ([Environment]::GetFolderPath('Programs')) 'Nitrate.lnk'))) { throw 'Start menu shortcut missing' }
    # Release builds are GUI-subsystem; redirected stdout still carries the output.
    $code, $line = Query '--version'
    if ($code -ne 0 -or $line -ne "nitrate $Version") { throw "nitrate --version printed '$line' ($code)" }
    # The bundled libmpv loads and has the client API Nitrate needs.
    $code, $line = Query '--mpv-info'
    if ($code -ne 0) { throw "nitrate --mpv-info failed ($code)" }
    Write-Host "   $line"
    # File types: the ProgID, "Open with" for every type, the Default apps entry.
    $cmd = Value "$progid\shell\open\command" '(default)'
    if ($cmd -ne "`"$exe`" `"%1`"") { throw "ProgID open command is '$cmd'" }
    foreach ($t in $types) {
        if ($null -eq (Value "HKCU:\Software\Classes\.$t\OpenWithProgids" 'Nitrate.Media')) { throw ".$t is not registered" }
        if ((Value $caps ".$t") -ne 'Nitrate.Media') { throw ".$t missing from the Default apps capabilities" }
    }
    if ((Value $registered 'Nitrate') -ne 'Software\Nitrate\Capabilities') { throw 'RegisteredApplications entry missing' }
    if ($null -eq (Value $mkv $other)) { throw "Setup removed another app's .mkv entry" }
}

# Another player's "Open with" entry, which install and uninstall must keep.
New-Item -Path $mkv -Force | Out-Null
New-ItemProperty -Path $mkv -Name $other -Value '' -Force | Out-Null

Write-Host '1. Silent install'
if ((Run $Setup @('/S'))) { throw 'Setup failed' }
Assert-Installed

Write-Host '2. Silent reinstall over the existing copy (what an update does)'
if ((Run $Setup @('/S'))) { throw 'Reinstall failed' }
Assert-Installed

Write-Host '3. A locked nitrate.exe (as while Nitrate runs) makes a silent install give up after 30 s'
# Hosted runners have no interactive desktop for a real Nitrate window, so the
# test holds the same lock a running exe has: no write sharing.
$lock = [IO.File]::Open($exe, 'Open', 'Read', 'Read')
try {
    $started = Get-Date
    $code = Run $Setup @('/S') 90
    $waited = ((Get-Date) - $started).TotalSeconds
} finally { $lock.Dispose() }
if ($code -eq 0) { throw 'Setup replaced a locked nitrate.exe' }
if ($waited -lt 25) { throw "Setup gave up after $([int]$waited) s instead of waiting about 30 s" }
Write-Host ("   setup refused with exit code {0} after {1:N0} s" -f $code, $waited)
Assert-Installed

Write-Host '4. Silent uninstall'
if ((Run (Join-Path $dir 'uninstall.exe') @('/S'))) { throw 'Uninstall failed' }
# The NSIS uninstaller copies itself to TEMP and finishes there.
$deadline = (Get-Date).AddSeconds(30)
while ((Test-Path -LiteralPath $exe) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 250 }
if (Test-Path -LiteralPath $exe) { throw 'nitrate.exe still present after uninstall' }
if (Test-Path -LiteralPath (Join-Path $dir 'libmpv-2.dll')) { throw 'libmpv-2.dll still present after uninstall' }
if (Test-Path -LiteralPath $key) { throw 'Installed apps entry still present' }
if (Test-Path -LiteralPath $progid) { throw 'ProgID still present after uninstall' }
if ($null -ne (Value $registered 'Nitrate')) { throw 'RegisteredApplications entry still present' }
foreach ($t in $types) {
    if ($null -ne (Value "HKCU:\Software\Classes\.$t\OpenWithProgids" 'Nitrate.Media')) { throw ".$t still lists Nitrate after uninstall" }
}
if ($null -eq (Value $mkv $other)) { throw "Uninstall removed another app's .mkv entry" }
Write-Host 'Installer lifecycle passed.'
