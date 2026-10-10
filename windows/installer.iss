; Earth Studio installer (Inno Setup 6).
;
; Build on Windows, or on Linux with Wine:
;   cargo build --release -p earth-studio --target x86_64-pc-windows-gnu
;   iscc /DAppVersion=0.5.0 /DStudioExe=..\target\x86_64-pc-windows-gnu\release\earth-studio.exe windows\installer.iss
; The result is windows\Output\EarthStudio-Kurulum.exe.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef StudioExe
  #define StudioExe "..\target\release\earth-studio.exe"
#endif

[Setup]
AppId={{6F0E7C52-8D1B-4E6A-9C3F-EA27B5D4E1A9}
AppName=Earth Studio
AppVersion={#AppVersion}
AppVerName=Earth Studio {#AppVersion}
AppPublisher=Mahmut Sibal
AppPublisherURL=https://github.com/MahmutSibal/earth-lang
AppSupportURL=https://github.com/MahmutSibal/earth-lang/issues
AppCopyright=Copyright (c) 2026 Mahmut Sibal
VersionInfoVersion={#AppVersion}
VersionInfoDescription=Earth Studio Kurulumu

; Per-user install into %LOCALAPPDATA%\Programs: no administrator rights.
PrivilegesRequired=lowest
DefaultDirName={autopf}\Earth Studio
DefaultGroupName=Earth Studio
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0

DisableWelcomePage=no
SetupIconFile=earth.ico
UninstallDisplayIcon={app}\earth-studio.exe
UninstallDisplayName=Earth Studio
WizardStyle=modern
WizardImageFile=installer\wizard-100.bmp,installer\wizard-125.bmp,installer\wizard-150.bmp,installer\wizard-200.bmp
WizardSmallImageFile=installer\wizard-small-100.bmp,installer\wizard-small-125.bmp,installer\wizard-small-150.bmp,installer\wizard-small-200.bmp
WizardImageStretch=no
WizardImageBackColor=$040404

OutputDir=Output
OutputBaseFilename=EarthStudio-Kurulum
Compression=lzma2/max
SolidCompression=yes
ChangesAssociations=yes
CloseApplications=yes
; Turkish Windows gets Turkish, English Windows English, others are asked.
ShowLanguageDialog=auto

[Languages]
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
turkish.EarthFiles=Earth programı
english.EarthFiles=Earth program

[Messages]
turkish.WelcomeLabel2=Earth Studio, Türkçe ve İngilizce yazılabilen Earth programlama dili için editördür.%n%nKurulum, Earth Studio'yu bilgisayarınıza kuracak. Yönetici izni gerekmez.%n%nEarth ücretsiz ve açık kaynaklıdır (MIT lisansı).
english.WelcomeLabel2=Earth Studio is the editor for Earth, a programming language you can write in Turkish or English.%n%nSetup will install Earth Studio on your computer. No administrator rights are needed.%n%nEarth is free and open source (MIT license).

[Tasks]
; Checked by default: the user asked for Earth Studio on the desktop.
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#StudioExe}"; DestDir: "{app}"; DestName: "earth-studio.exe"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Earth Studio"; Filename: "{app}\earth-studio.exe"
Name: "{autodesktop}\Earth Studio"; Filename: "{app}\earth-studio.exe"; Tasks: desktopicon

[Registry]
; Double-clicking a .earth file opens it in Earth Studio.
Root: HKA; Subkey: "Software\Classes\.earth"; ValueType: string; ValueName: ""; ValueData: "EarthStudio.Program"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\EarthStudio.Program"; ValueType: string; ValueName: ""; ValueData: "{cm:EarthFiles}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\EarthStudio.Program\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\earth-studio.exe"",0"
Root: HKA; Subkey: "Software\Classes\EarthStudio.Program\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\earth-studio.exe"" ""%1"""

[Run]
Filename: "{app}\earth-studio.exe"; Description: "{cm:LaunchProgram,Earth Studio}"; Flags: nowait postinstall skipifsilent
; Earth Studio's own updater passes /UPDATE: reopen Studio after updating.
Filename: "{app}\earth-studio.exe"; Flags: nowait; Check: IsStudioUpdate

[Code]
function IsStudioUpdate: Boolean;
begin
  Result := Pos('/UPDATE', Uppercase(GetCmdTail)) > 0;
end;
