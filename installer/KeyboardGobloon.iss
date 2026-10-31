; Keyboard Gobloon installer (Inno Setup 6).
; Build: iscc /DAppVersion=0.1.0 installer\KeyboardGobloon.iss  (after `cargo build --release`)
; Output: target\installer\KeyboardGobloon-<version>-setup.exe

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "Keyboard Gobloon"
#define AppExe "KeyboardGobloon.exe"

[Setup]
AppId={{89526287-E0D6-4D1A-AE8F-54B75C8933D4}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=h0use
AppPublisherURL=https://github.com/h0useRus/KeyboardGobloon
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
LicenseFile=..\LICENSE
OutputDir=..\target\installer
OutputBaseFilename=KeyboardGobloon-{#AppVersion}-setup
#if FileExists(SourcePath + "..\assets\icon\KeyboardGobloon.ico")
SetupIconFile=..\assets\icon\KeyboardGobloon.ico
#endif
UninstallDisplayIcon={app}\{#AppExe}
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; No drivers: installs for the current user without administrator rights (the dialog offers
; all users too).
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
; The running app holds this mutex (kbg-platform instance.rs): setup and uninstall ask to
; close it first.
AppMutex=KeyboardGobloon.instance

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ru"; MessagesFile: "compiler:Languages\Russian.isl"

[CustomMessages]
en.DesktopIcon=Create a desktop shortcut
ru.DesktopIcon=Создать ярлык на рабочем столе
en.Autostart=Start with Windows
ru.Autostart=Запускать вместе с Windows

[Tasks]
Name: "autostart"; Description: "{cm:Autostart}"
Name: "desktopicon"; Description: "{cm:DesktopIcon}"; Flags: unchecked

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\lang\*.ftl"; DestDir: "{app}\lang"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; "Start with Windows". Later the app sets this value itself; uninstall always removes it.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "KeyboardGobloon"; ValueData: """{app}\{#AppExe}"""; Tasks: autostart
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "KeyboardGobloon"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
