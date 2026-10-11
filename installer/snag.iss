; Katch installer (Inno Setup 6). Build with installer\build.sh, which passes the version.
; Installs for the current user only (no admin prompt) into %LOCALAPPDATA%\Programs\Katch.
;
; Katch was called Snag up to 1.1.3. Kept from then, so this installer updates a Snag install in
; place (its folder, settings, sign-in entry and taskbar pins): the AppId, the program's name
; (snag.exe), the sign-in entry's value name and the data folder (%APPDATA%\Snag).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
; Never change: it's how Setup finds the Snag or Katch already installed.
AppId={{6F0D2B8E-4C1A-4E7B-9C55-3A1F2E9B7D41}
AppName=Katch
AppVersion={#AppVersion}
AppVerName=Katch {#AppVersion}
AppPublisher=Kuldeep Saini
AppPublisherURL=https://github.com/kuldeepsaini23/katch
AppSupportURL=https://github.com/kuldeepsaini23/katch/issues
AppUpdatesURL=https://github.com/kuldeepsaini23/katch/releases
PrivilegesRequired=lowest
; An update keeps the folder it has (a Snag install stays in Programs\Snag).
DefaultDirName={autopf}\Katch
DisableProgramGroupPage=yes
DisableDirPage=auto
LicenseFile=..\LICENSE
SetupIconFile=..\crates\app\assets\logo\snag.ico
UninstallDisplayIcon={app}\snag.exe
UninstallDisplayName=Katch
OutputDir=..\target\installer
OutputBaseFilename=Katch-Setup-{#AppVersion}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0

[Tasks]
Name: "desktopicon"; Description: "Put a Katch icon on the desktop"; Flags: unchecked
Name: "autostart"; Description: "Start Katch when I sign in to Windows (it waits quietly in the tray)"; Flags: unchecked

[Files]
Source: "..\target\release\snag.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
; The browser extension, for "Load unpacked" until it is in the stores.
Source: "..\extension\dist\chrome\*"; DestDir: "{app}\browser-extension"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{autoprograms}\Katch"; Filename: "{app}\snag.exe"
Name: "{autodesktop}\Katch"; Filename: "{app}\snag.exe"; Tasks: desktopicon

[InstallDelete]
; The shortcuts from before the rename (Snag 1.1.3 and older): the ones above take their place.
Type: files; Name: "{autoprograms}\Snag.lnk"
Type: files; Name: "{autodesktop}\Snag.lnk"

[Registry]
; The value keeps its name from before the rename, so an update leaves the one already there (and
; its switch in Windows' Startup apps) as it is. Windows lists it by the program's description: Katch.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Snag"; ValueData: """{app}\snag.exe"" --background"; Tasks: autostart; Flags: uninsdeletevalue

[Run]
; Through Explorer: a program started by Setup itself inherits Setup's "redirection guard", and
; then yt-dlp can't follow linked folders on PATH (e.g. nvm's Node.js) — WinError 448.
Filename: "{win}\explorer.exe"; Parameters: """{app}\snag.exe"""; Description: "Open Katch now"; Flags: nowait postinstall skipifsilent
; An update started from inside Katch or Snag (/RELAUNCH=1, see crates/app/src/updater.rs): open it again.
Filename: "{win}\explorer.exe"; Parameters: """{app}\snag.exe"""; Flags: nowait; Check: Relaunch

[Code]
function Relaunch: Boolean;
begin
  Result := ExpandConstant('{param:relaunch|0}') = '1';
end;

// Asks a running Katch (or Snag) to pause its downloads, save and quit, then waits for it to be
// gone, so its files can be replaced (update) or removed (uninstall).
procedure QuitApp(Exe: String);
var
  Code, Waited: Integer;
begin
  if not FileExists(Exe) then
    Exit;
  Exec(Exe, '--quit', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Waited := 0;
  // The exe can be replaced once it has exited.
  while Waited < 10000 do
  begin
    if RenameFile(Exe, Exe + '.check') then
    begin
      RenameFile(Exe + '.check', Exe);
      Exit;
    end;
    Sleep(250);
    Waited := Waited + 250;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  QuitApp(ExpandConstant('{app}\snag.exe'));
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  QuitApp(ExpandConstant('{app}\snag.exe'));
  Result := True;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Data: String;
begin
  if CurUninstallStep <> usPostUninstall then
    Exit;
  // The data folder keeps its name from before the rename.
  Data := ExpandConstant('{userappdata}\Snag');
  if not DirExists(Data) then
    Exit;
  // Downloaded files are never touched: they live in your Downloads folder.
  if SuppressibleMsgBox('Also remove your Katch settings and download history?' + #13#10 + #13#10 +
       'Your downloaded files stay where they are either way.', mbConfirmation, MB_YESNO or MB_DEFBUTTON2, IDNO) = IDYES then
    DelTree(Data, True, True, True);
end;
