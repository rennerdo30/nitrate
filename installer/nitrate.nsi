; Nitrate per-user Windows setup (NSIS 3). Built by scripts/package.ps1:
;   makensis /DVERSION=0.1.0 /DVI_VERSION=0.1.0.0 /DSRC=<dir> /DOUTFILE=<file> installer\nitrate.nsi
; SRC holds nitrate.exe, libmpv-2.dll, LICENSE.txt, THIRD-PARTY-NOTICES.txt,
; libmpv-NOTICE.txt and LGPL-2.1.txt.
;
; Installs to %LOCALAPPDATA%\Programs\nitrate without admin rights, with a Start
; menu shortcut and an Installed apps entry. No PATH changes.
;
; File types (per user, HKCU only): Nitrate is added to "Open with" for the
; media types below and registered under Default apps, so the user can make
; it the default player there. Windows does not let a setup take over the
; default silently, and this setup does not try; the finish page offers to
; open the Default apps page. Uninstall removes every entry it wrote.
;
; The in-app updater runs this setup with /S /UPDATE: it waits for Nitrate to
; close, installs, and starts Nitrate again. Uninstall removes only installed
; files; settings in %APPDATA%\nitrate stay.

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
!define PROGID "Nitrate.Media"
!define CAPS_KEY "Software\Nitrate\Capabilities"
; Media types Nitrate registers for. Keep in step with scripts/test-installer.ps1.
!define TYPES "mkv mp4 m4v mov webm avi wmv flv ts m2ts mts mpg mpeg ogv 3gp mp3 flac m4a ogg opus wav"

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
; The second finish-page checkbox opens Settings > Default apps for Nitrate.
!define MUI_FINISHPAGE_SHOWREADME ""
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Make Nitrate my default player (opens Settings)"
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION OpenDefaultApps

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

Function OpenDefaultApps
  ExecShell "open" "ms-settings:defaultapps?registeredAppUser=Nitrate"
FunctionEnd

; A running nitrate.exe cannot be replaced. Interactive runs ask to close it;
; silent runs (the updater) wait up to 30 s for Nitrate to exit, then fail.
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

; Calls ${FN} with $R0 set to each extension in ${TYPES} (without the dot).
!macro FOR_EACH_TYPE FN
  StrCpy $R1 "${TYPES} "
  ${Do}
    StrCpy $R2 0
    ${Do}
      StrCpy $R3 $R1 1 $R2
      ${If} $R3 == " "
        ${Break}
      ${EndIf}
      IntOp $R2 $R2 + 1
    ${Loop}
    StrCpy $R0 $R1 $R2
    IntOp $R2 $R2 + 1
    StrCpy $R1 $R1 "" $R2
    Call ${FN}
  ${LoopUntil} $R1 == ""
!macroend

Function RegisterType
  WriteRegStr HKCU "Software\Classes\.$R0\OpenWithProgids" "${PROGID}" ""
  WriteRegStr HKCU "${CAPS_KEY}\FileAssociations" ".$R0" "${PROGID}"
  WriteRegStr HKCU "Software\Classes\Applications\nitrate.exe\SupportedTypes" ".$R0" ""
FunctionEnd

Function un.UnregisterType
  ; Only Nitrate's own value: the keys may hold other apps' entries, and
  ; DeleteRegKey /ifempty ignores values, so keys are never deleted here.
  DeleteRegValue HKCU "Software\Classes\.$R0\OpenWithProgids" "${PROGID}"
FunctionEnd

Section "Nitrate"
  Call WaitForNitrate
  SetOutPath "$INSTDIR"
  File "${SRC}\nitrate.exe"
  File "${SRC}\libmpv-2.dll"
  File "${SRC}\LICENSE.txt"
  File "${SRC}\THIRD-PARTY-NOTICES.txt"
  File "${SRC}\libmpv-NOTICE.txt"
  File "${SRC}\LGPL-2.1.txt"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Nitrate.lnk" "$INSTDIR\nitrate.exe"

  ; File types: one ProgID, "Open with" entries, Default apps capabilities.
  WriteRegStr HKCU "Software\Classes\${PROGID}" "" "Media file"
  WriteRegStr HKCU "Software\Classes\${PROGID}" "FriendlyTypeName" "Media file (Nitrate)"
  WriteRegStr HKCU "Software\Classes\${PROGID}\DefaultIcon" "" "$INSTDIR\nitrate.exe,0"
  WriteRegStr HKCU "Software\Classes\${PROGID}\shell" "" "open"
  WriteRegStr HKCU "Software\Classes\${PROGID}\shell\open" "FriendlyAppName" "Nitrate"
  WriteRegStr HKCU "Software\Classes\${PROGID}\shell\open\command" "" '"$INSTDIR\nitrate.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\Applications\nitrate.exe" "FriendlyAppName" "Nitrate"
  WriteRegStr HKCU "Software\Classes\Applications\nitrate.exe\shell\open\command" "" '"$INSTDIR\nitrate.exe" "%1"'
  WriteRegStr HKCU "${CAPS_KEY}" "ApplicationName" "Nitrate"
  WriteRegStr HKCU "${CAPS_KEY}" "ApplicationDescription" "A fast media player."
  WriteRegStr HKCU "${CAPS_KEY}" "ApplicationIcon" "$INSTDIR\nitrate.exe,0"
  WriteRegStr HKCU "Software\RegisteredApplications" "Nitrate" "${CAPS_KEY}"
  !insertmacro FOR_EACH_TYPE RegisterType
  ; SHCNE_ASSOCCHANGED: Explorer picks up the new "Open with" entries.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'

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
  Delete "$INSTDIR\libmpv-2.dll"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\THIRD-PARTY-NOTICES.txt"
  Delete "$INSTDIR\libmpv-NOTICE.txt"
  Delete "$INSTDIR\LGPL-2.1.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Nitrate.lnk"

  !insertmacro FOR_EACH_TYPE un.UnregisterType
  DeleteRegKey HKCU "Software\Classes\${PROGID}"
  DeleteRegKey HKCU "Software\Classes\Applications\nitrate.exe"
  DeleteRegValue HKCU "Software\RegisteredApplications" "Nitrate"
  DeleteRegKey HKCU "Software\Nitrate\Capabilities"
  DeleteRegKey /ifempty HKCU "Software\Nitrate"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'

  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
