#ifndef DownloadUrl
  #define DownloadUrl "https://github.com/nefdtie101/VinRougeData/releases/latest/download/VinRouge-windows-x64.zip"
#endif
#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

[Setup]
AppName=VinRouge
AppVersion={#AppVersion}
AppPublisher=VinRouge
DefaultDirName={autopf64}\VinRouge
OutputBaseFilename=VinRouge-{#AppVersion}-Setup
OutputDir=.
PrivilegesRequired=admin
DisableProgramGroupPage=yes
UninstallDisplayName=VinRouge

[Icons]
Name: "{autodesktop}\VinRouge";            Filename: "{app}\vinrouge-desktop.exe"
Name: "{autoprograms}\VinRouge\VinRouge";  Filename: "{app}\vinrouge-desktop.exe"

[UninstallDelete]
Type: filesandordirs; Name: "{app}"

[Code]
procedure CurStepChanged(CurStep: TSetupStep);
var
  ScriptFile, ZipFile, AppDir: String;
  ResultCode: Integer;
begin
  if CurStep = ssInstall then begin
    AppDir     := ExpandConstant('{app}');
    ScriptFile := ExpandConstant('{tmp}\install.ps1');
    ZipFile    := ExpandConstant('{tmp}\vinrouge.zip');

    SaveStringToFile(ScriptFile,
      'Invoke-WebRequest -Uri "{#DownloadUrl}" -OutFile "' + ZipFile + '" -UseBasicParsing' + #13#10 +
      'Expand-Archive -Path "' + ZipFile + '" -DestinationPath "' + AppDir + '" -Force' + #13#10 +
      'Remove-Item "' + ZipFile + '" -Force',
      False);

    if not Exec('powershell.exe',
      '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "' + ScriptFile + '"',
      '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
      MsgBox('Download failed. Please install manually from https://github.com/nefdtie101/VinRougeData/releases',
             mbError, MB_OK);

    DeleteFile(ScriptFile);
  end;
end;
