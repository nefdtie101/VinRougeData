Unicode True
!include "MUI2.nsh"
!include "LogicLib.nsh"

; Injected by the workflow at build time
!ifndef DOWNLOAD_URL
  !define DOWNLOAD_URL "https://github.com/VinRougeData/VinRougeData/releases/latest/download/VinRouge-windows-x64.zip"
!endif
!ifndef APP_VERSION
  !define APP_VERSION "0.1.0"
!endif

!define APPNAME    "VinRouge"
!define PUBLISHER  "VinRouge"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}"

Name    "${APPNAME} ${APP_VERSION}"
OutFile "VinRouge-${APP_VERSION}-Setup.exe"
InstallDir "$PROGRAMFILES64\${APPNAME}"
InstallDirRegKey HKLM "${UNINST_KEY}" "InstallDir"
RequestExecutionLevel admin
BrandingText "${APPNAME} ${APP_VERSION}"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; ── Install ────────────────────────────────────────────────────────────────────
Section "Install" SecMain
  SetOutPath "$INSTDIR"

  ; Write a temp PowerShell script — avoids all NSIS quote-escaping issues.
  ; NSIS expands $TEMP, $INSTDIR, and ${DOWNLOAD_URL} at runtime before writing.
  DetailPrint "Preparing download..."
  FileOpen  $0 "$TEMP\vinrouge_install.ps1" w
  FileWrite $0 "Invoke-WebRequest -Uri '${DOWNLOAD_URL}' -OutFile '$TEMP\VinRouge-windows-x64.zip' -UseBasicParsing$\r$\n"
  FileWrite $0 "Expand-Archive -Path '$TEMP\VinRouge-windows-x64.zip' -DestinationPath '$INSTDIR' -Force$\r$\n"
  FileWrite $0 "Remove-Item '$TEMP\VinRouge-windows-x64.zip' -Force$\r$\n"
  FileClose $0

  DetailPrint "Downloading VinRouge (this may take a few minutes)..."
  ExecWait 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$TEMP\vinrouge_install.ps1"'
  Delete "$TEMP\vinrouge_install.ps1"

  ; Shortcuts
  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\vinrouge-desktop.exe"
  CreateShortcut "$DESKTOP\${APPNAME}.lnk"               "$INSTDIR\vinrouge-desktop.exe"

  ; Add/Remove Programs entry
  WriteRegStr   HKLM "${UNINST_KEY}" "DisplayName"          "${APPNAME}"
  WriteRegStr   HKLM "${UNINST_KEY}" "DisplayVersion"       "${APP_VERSION}"
  WriteRegStr   HKLM "${UNINST_KEY}" "Publisher"            "${PUBLISHER}"
  WriteRegStr   HKLM "${UNINST_KEY}" "InstallDir"           "$INSTDIR"
  WriteRegStr   HKLM "${UNINST_KEY}" "UninstallString"      '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify"             1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair"             1

  WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

; ── Uninstall ──────────────────────────────────────────────────────────────────
Section "Uninstall"
  RMDir /r "$INSTDIR"
  Delete "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk"
  Delete "$DESKTOP\${APPNAME}.lnk"
  RMDir  "$SMPROGRAMS\${APPNAME}"
  DeleteRegKey HKLM "${UNINST_KEY}"
SectionEnd
