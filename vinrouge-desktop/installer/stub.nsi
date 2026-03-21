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

  DetailPrint "Downloading VinRouge..."
  Var /GLOBAL ZIP_PATH
  StrCpy $ZIP_PATH "$TEMP\VinRouge-windows-x64.zip"

  ExecWait 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Invoke-WebRequest -Uri ''${DOWNLOAD_URL}'' -OutFile ''$ZIP_PATH'' -UseBasicParsing"'

  DetailPrint "Installing..."
  ExecWait 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "Expand-Archive -Path ''$ZIP_PATH'' -DestinationPath ''$INSTDIR'' -Force"'

  Delete "$ZIP_PATH"

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
