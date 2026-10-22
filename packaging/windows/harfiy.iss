#ifndef MyAppVersion
#define MyAppVersion "0.1.0"
#endif

#ifndef MyBinaryDir
#define MyBinaryDir "..\..\target\release"
#endif

#ifndef MyOutputDir
#define MyOutputDir "..\..\dist"
#endif

[Setup]
AppId={{8B45462D-8D59-4E87-BA3F-C4D8A9F1A4E7}
AppName=Harfiy
AppVersion={#MyAppVersion}
AppVerName=Harfiy {#MyAppVersion}
AppPublisher=Abdumannon Shamsiyev
AppPublisherURL=https://www.linkedin.com/in/abdumannon-shamsiyev
AppSupportURL=https://t.me/Abdumannon_Shamsiyev
AppUpdatesURL=https://github.com/harfiy
DefaultDirName={autopf}\Harfiy
DefaultGroupName=Harfiy
DisableProgramGroupPage=yes
LicenseFile=..\..\README.md
OutputDir={#MyOutputDir}
OutputBaseFilename=harfiy-v{#MyAppVersion}-windows-x86_64-setup
SetupIconFile=..\..\assets\icon.ico
UninstallDisplayIcon={app}\harfiy.exe
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64
PrivilegesRequired=lowest
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "autostart"; Description: "Windows yuklanganda avtomatik ishga tushirish (Autostart)"; GroupDescription: "Qo'shimcha:"

[Files]
Source: "{#MyBinaryDir}\harfiy.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Harfiy"; Filename: "{app}\harfiy.exe"
Name: "{group}\Harfiy ni o'chirish"; Filename: "{uninstallexe}"
Name: "{userdesktop}\Harfiy"; Filename: "{app}\harfiy.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Harfiy"; ValueData: """{app}\harfiy.exe"""; Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\harfiy.exe"; Description: "Harfiy dasturini ishga tushirish"; Flags: nowait postinstall skipifsilent
