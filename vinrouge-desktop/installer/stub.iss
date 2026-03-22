#ifndef DownloadUrl
  #define DownloadUrl "https://github.com/nefdtie101/VinRougeData/releases/download/latest/VinRouge-windows-x64.zip"
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
var
  DownloadPage: TDownloadWizardPage;

procedure InitializeWizard;
begin
  DownloadPage := CreateDownloadPage(
    'Downloading VinRouge',
    'Downloading application files from GitHub. This may take several minutes.',
    nil);
end;

// Download happens when the user clicks Next on the Ready page,
// before any files are installed. Shows a real progress bar with
// percentage, MB downloaded, and download speed.
function NextButtonClick(CurPageID: Integer): Boolean;
begin
  if CurPageID = wpReady then begin
    DownloadPage.Clear;
    DownloadPage.Add('{#DownloadUrl}', 'vinrouge.zip', '');
    DownloadPage.Show;
    try
      try
        DownloadPage.Download;
        Result := True;
      except
        SuppressibleMsgBox(AddPeriod(GetExceptionMessage), mbCriticalError, MB_OK, IDOK);
        Result := False;
      end;
    finally
      DownloadPage.Hide;
    end;
  end else
    Result := True;
end;

// After download, extract the zip into the install directory.
procedure CurStepChanged(CurStep: TSetupStep);
var
  ZipFile, AppDir: String;
  ResultCode: Integer;
begin
  if CurStep = ssInstall then begin
    AppDir  := ExpandConstant('{app}');
    ZipFile := ExpandConstant('{tmp}\vinrouge.zip');

    WizardForm.StatusLabel.Caption := 'Extracting files — please wait...';
    WizardForm.Update;

    if not Exec('powershell.exe',
      '-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command ' +
      '"Expand-Archive -Path \"' + ZipFile + '\" -DestinationPath \"' + AppDir + '\" -Force"',
      '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or (ResultCode <> 0) then
      MsgBox('Extraction failed (exit code: ' + IntToStr(ResultCode) + ').' + #13#10 +
             'Please try re-running the installer.',
             mbError, MB_OK);

    DeleteFile(ZipFile);
  end;
end;
