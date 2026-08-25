#ifndef MyAppVersion
  #define MyAppVersion "1.2.1-pre2"
#endif
#ifndef MyVersionInfoVersion
  #define MyVersionInfoVersion "1.2.1"
#endif

[Setup]
AppName=Locoryn
AppVersion={#MyAppVersion}
VersionInfoVersion={#MyVersionInfoVersion}
; Install per-user so setup and the application never require elevation.
DefaultDirName={localappdata}\Programs\Locoryn
DefaultGroupName=Locoryn
PrivilegesRequired=lowest
UsedUserAreasWarning=no
MinVersion=10.0.22000
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=dist
OutputBaseFilename=locoryn-{#MyAppVersion}-windows-11-x64-setup
SetupIconFile=assets\icon.ico
UninstallDisplayIcon={app}\locoryn.exe
Compression=lzma
SolidCompression=yes

[Files]
Source: "target\release\locoryn.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "assets\*"; DestDir: "{app}\assets"; Flags: recursesubdirs createallsubdirs
Source: "config\*"; DestDir: "{app}\config"; Flags: recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Locoryn"; Filename: "{app}\locoryn.exe"; WorkingDir: "{app}"
Name: "{userdesktop}\Locoryn"; Filename: "{app}\locoryn.exe"; WorkingDir: "{app}"

[Run]
Filename: "{app}\locoryn.exe"; Description: "Launch Locoryn"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent
