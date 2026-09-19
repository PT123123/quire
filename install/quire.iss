; Quire Windows installer (M8 / D8). Inno Setup 6 script.
;
; Do not run ISCC by hand - use the wrapper, which builds the release exe
; first and then finds ISCC.exe (winget install JRSoftware.InnoSetup):
;   install\build-installer.ps1              ; build exe + setup program
;   install\build-installer.ps1 -SkipBuild   ; reuse target\release\quire.exe
; The setup program lands in dist\, next to the zip that `just dist` writes.
; The setup icon and the exe icon are the same file: install\quire.ico,
; rendered by install\make_icon.ps1.
;
; Version comes from the exe's own version resource, which build.rs generates
; from [package].version - so Cargo.toml stays the only place a version is
; written by hand.

#define AppExe "..\target\release\quire.exe"
; build.rs writes the resource as "<package version>.0"; drop the last octet.
#define AppVersion Copy(GetFileVersion(AppExe), 1, RPos(".", GetFileVersion(AppExe)) - 1)

[Setup]
; Fixed GUID: upgrading over an old install replaces it instead of adding a
; second entry in "Installed apps". Never regenerate this value.
AppId={{889B1B1B-ADCC-4684-8070-903342F028D3}
AppName=Quire
AppVersion={#AppVersion}
AppVerName=Quire {#AppVersion}
AppPublisher=Quire
DefaultDirName={localappdata}\Programs\Quire
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=Quire-{#AppVersion}-windows-x64-setup
SetupIconFile=quire.ico
UninstallDisplayIcon={app}\quire.exe
UninstallDisplayName=Quire
; Per-user on purpose: the app opens its database relative to the working
; directory (appdata/quire.db), so the install folder must be writable. A
; machine-wide install under Program Files would leave the session running in
; memory. See docs/DECISIONS.md ADR-0017.
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Files]
Source: "{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "assocmd"; Description: "Add Quire to the ""Open with"" list for .md files"; GroupDescription: "File association:"; Flags: unchecked

[Registry]
; Only an "Open with" candidate, never a default handler: double-clicking a
; .md file must keep going to whatever the user already chose.
Root: HKA; Subkey: "Software\Classes\.md\OpenWithProgids"; ValueType: string; ValueName: "Quire.Markdown"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assocmd
Root: HKA; Subkey: "Software\Classes\Quire.Markdown"; ValueType: string; ValueName: ""; ValueData: "Quire Markdown document"; Flags: uninsdeletekey; Tasks: assocmd
Root: HKA; Subkey: "Software\Classes\Quire.Markdown\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\quire.exe,0"; Tasks: assocmd
Root: HKA; Subkey: "Software\Classes\Quire.Markdown\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\quire.exe"" --open ""%1"""; Tasks: assocmd

[Icons]
Name: "{autoprograms}\Quire"; Filename: "{app}\quire.exe"; IconFilename: "{app}\quire.exe"
Name: "{autodesktop}\Quire"; Filename: "{app}\quire.exe"; IconFilename: "{app}\quire.exe"; Tasks: desktopicon
Name: "{autoprograms}\Uninstall Quire"; Filename: "{uninstallexe}"

[Run]
Filename: "{app}\quire.exe"; Description: "{cm:LaunchProgram,Quire}"; Flags: nowait postinstall skipifsilent
