; Nitrate per-user Windows setup (NSIS 3). Built by scripts/package.ps1:
;   makensis /DVERSION=0.1.0 /DVI_VERSION=0.1.0.0 /DSRC=<dir> /DOUTFILE=<file> installer\nitrate.nsi
; SRC holds nitrate.exe, LICENSE.txt and THIRD-PARTY-NOTICES.txt.
;
; Installs to %LOCALAPPDATA%\Programs\nitrate without admin rights, with a Start
; menu shortcut and an Installed apps entry. No PATH or file-type changes.
; The in-app updater (planned) runs this setup with /S /UPDATE: it waits for Nitrate to close,
; installs, and starts Nitrate again. Uninstall removes only installed files;
; settings in %APPDATA%\nitrate stay.

!macro REQUIRE name
  !ifndef ${name}
    !error "Pass /D${name}=..."
  !endif
!macroend
!insertmacro REQUIRE VERSION
!insertmacro REQUIRE VI_VERSION
!insertmacro REQUIRE SRC
!insertmacro REQUIRE OUTFILE

Unicode true
ManifestDPIAware true
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include MUI2.nsh
!include LogicLib.nsh
!include FileFunc.nsh

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\nitrate"

Name "Nitrate"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\nitrate"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"
BrandingText "Nitrate ${VERSION}"

VIProductVersion "${VI_VERSION}"
VIAddVersionKey "ProductName" "Nitrate"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "Nitrate ${VERSION} setup"
VIAddVersionKey "LegalCopyright" "MIT License"

!define MUI_ICON "..\assets\logo\nitrate.ico"
!define MUI_UNICON "..\assets\logo\nitrate.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\nitrate.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start Nitrate"

!insertmacro MUI_PAGE_LICENSE "${SRC}\LICENSE.txt"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Var Update

Function .onInit
  StrCpy $Update 0
  ${GetParameters} $0
  ClearErrors
  ${GetOptions} $0 "/UPDATE" $1
  ${IfNot} ${Errors}
    StrCpy $Update 1
  ${EndIf}
FunctionEnd

; A running nitrate.exe cannot be replaced. Interactive runs ask to close it;
; silent runs (the updater) wait up to 30 s for nitrate to exit, then fail.
!macro WAIT_FOR_NITRATE un
Function ${un}WaitForNitrate
  StrCpy $2 0
  retry:
  ${IfNot} ${FileExists} "$INSTDIR\nitrate.exe"
    Return
  ${EndIf}
  ClearErrors
  FileOpen $0 "$INSTDIR\nitrate.exe" a
  ${IfNot} ${Errors}
    FileClose $0
    Return
  ${EndIf}
  IfSilent wait
  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Nitrate is running. Close every Nitrate window, then choose Retry." /SD IDCANCEL IDRETRY retry
  SetErrorLevel 2
  Quit
  wait:
  ${If} $2 >= 60
    SetErrorLevel 2
    Quit
  ${EndIf}
  Sleep 500
  IntOp $2 $2 + 1
  Goto retry
FunctionEnd
!macroend
!insertmacro WAIT_FOR_NITRATE ""
!insertmacro WAIT_FOR_NITRATE "un."

Section "nitrate"
  Call WaitForNitrate
  SetOutPath "$INSTDIR"
  File "${SRC}\nitrate.exe"
  File "${SRC}\LICENSE.txt"
  File "${SRC}\THIRD-PARTY-NOTICES.txt"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Nitrate.lnk" "$INSTDIR\nitrate.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "Nitrate"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "rennerdo30"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "URLInfoAbout" "https://github.com/rennerdo30/nitrate"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\nitrate.exe,0"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" "$0"
  ${If} $Update == 1
    Exec '"$INSTDIR\nitrate.exe"'
  ${EndIf}
SectionEnd

Section "Uninstall"
  Call un.WaitForNitrate
  Delete "$INSTDIR\nitrate.exe"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Nitrate.lnk"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
