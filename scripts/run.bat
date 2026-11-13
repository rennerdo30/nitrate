@echo off
rem Build and start Nitrate from this checkout.
rem
rem   scripts\run.bat                      start with the empty player
rem   scripts\run.bat E:\Videos\clip.mkv   open a file
rem   scripts\run.bat -Release ...         optimized build (the real speed)
rem   scripts\run.bat -Pull ...            update to the latest main first (fast-forward only)
rem
rem libmpv-2.dll is copied next to nitrate.exe: from NITRATE_LIBMPV if set,
rem otherwise the pinned build (scripts\fetch-libmpv.ps1 downloads it once).
setlocal
cd /d "%~dp0.."
call "%~dp0_setup.bat" || exit /b 1
pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0run.ps1" %*
if errorlevel 1 (
    echo Nitrate could not start. See the messages above.
    pause
    exit /b 1
)
exit /b 0
