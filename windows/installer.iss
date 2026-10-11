; Earth Studio installer (Inno Setup 6).
;
; Build on Windows, or on Linux with Wine:
;   cargo build --release --workspace --target x86_64-pc-windows-gnu
;   iscc /DAppVersion=0.7.0 /DBinDir=..\target\x86_64-pc-windows-gnu\release windows\installer.iss
; The result is windows\Output\EarthStudio-Kurulum.exe.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
; The folder holding earth-studio.exe and earth.exe.
#ifndef BinDir
  #define BinDir "..\target\release"
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
ChangesEnvironment=yes
CloseApplications=yes
; Turkish Windows gets Turkish, English Windows English, others are asked.
ShowLanguageDialog=auto

[Languages]
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
turkish.EarthFiles=Earth programı
english.EarthFiles=Earth program
turkish.RunInTerminal=Terminalde çalıştır
english.RunInTerminal=Run in terminal
turkish.Integration=Windows ile bütünleşme:
english.Integration=Windows integration:
turkish.AssociateTask=.earth dosyalarını Earth Studio ile aç
english.AssociateTask=Open .earth files with Earth Studio
turkish.PathTask=earth komutunu PATH'e ekle (komut isteminde "earth" ve "earth dosya.earth" çalışır)
english.PathTask=Add the earth command to PATH (run "earth" and "earth file.earth" in a terminal)
turkish.UpdateTitle=Earth Studio güncellemesi
english.UpdateTitle=Earth Studio update
turkish.UpdateText=Bu bilgisayarda Earth Studio %1 kurulu.%n%nKurulum onu %2 sürümüne güncelleyecek. Kurulum klasörü, seçenekleriniz ve Belgeler\Earth içindeki programlarınız olduğu gibi kalır.%n%nEarth Studio açıksa kurulum onu kapatır.
english.UpdateText=Earth Studio %1 is installed on this computer.%n%nSetup will update it to version %2. The install folder, your choices and your programs in Documents\Earth stay as they are.%n%nIf Earth Studio is open, Setup will close it.
turkish.ReinstallText=Bu bilgisayarda Earth Studio %1 zaten kurulu.%n%nKurulum aynı sürümü yeniden kuracak; programlarınız olduğu gibi kalır.
english.ReinstallText=Earth Studio %1 is already installed on this computer.%n%nSetup will reinstall the same version; your programs stay as they are.

[Messages]
turkish.WelcomeLabel2=Earth Studio, Türkçe ve İngilizce yazılabilen Earth programlama dili için editördür.%n%nKurulum, Earth Studio'yu bilgisayarınıza kuracak. Yönetici izni gerekmez.%n%nEarth ücretsiz ve açık kaynaklıdır (MIT lisansı).
english.WelcomeLabel2=Earth Studio is the editor for Earth, a programming language you can write in Turkish or English.%n%nSetup will install Earth Studio on your computer. No administrator rights are needed.%n%nEarth is free and open source (MIT license).

[Tasks]
; All checked by default. On an update the previous choices are kept.
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "associate"; Description: "{cm:AssociateTask}"; GroupDescription: "{cm:Integration}"
Name: "addtopath"; Description: "{cm:PathTask}"; GroupDescription: "{cm:Integration}"

[Files]
Source: "{#BinDir}\earth-studio.exe"; DestDir: "{app}"; Flags: ignoreversion
; The command-line interpreter: an interactive shell, or runs a file.
Source: "{#BinDir}\earth.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Earth Studio"; Filename: "{app}\earth-studio.exe"
Name: "{autodesktop}\Earth Studio"; Filename: "{app}\earth-studio.exe"; Tasks: desktopicon

[Registry]
; Double-clicking a .earth file opens it in Earth Studio; its right-click
; menu can also run it in a terminal with earth.exe.
Root: HKA; Subkey: "Software\Classes\.earth"; ValueType: string; ValueName: ""; ValueData: "EarthStudio.Program"; Flags: uninsdeletevalue; Tasks: associate
Root: HKA; Subkey: "Software\Classes\EarthStudio.Program"; ValueType: string; ValueName: ""; ValueData: "{cm:EarthFiles}"; Flags: uninsdeletekey; Tasks: associate
Root: HKA; Subkey: "Software\Classes\EarthStudio.Program\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\earth-studio.exe"",0"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\EarthStudio.Program\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\earth-studio.exe"" ""%1"""; Tasks: associate
Root: HKA; Subkey: "Software\Classes\EarthStudio.Program\shell\run"; ValueType: string; ValueName: ""; ValueData: "{cm:RunInTerminal}"; Tasks: associate
Root: HKA; Subkey: "Software\Classes\EarthStudio.Program\shell\run\command"; ValueType: string; ValueName: ""; ValueData: """{app}\earth.exe"" ""%1"""; Tasks: associate

[Run]
Filename: "{app}\earth-studio.exe"; Description: "{cm:LaunchProgram,Earth Studio}"; Flags: nowait postinstall skipifsilent
; Earth Studio's own updater passes /UPDATE: reopen Studio after updating.
Filename: "{app}\earth-studio.exe"; Flags: nowait; Check: IsStudioUpdate

[Code]
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{6F0E7C52-8D1B-4E6A-9C3F-EA27B5D4E1A9}_is1';

function IsStudioUpdate: Boolean;
begin
  Result := Pos('/UPDATE', Uppercase(GetCmdTail)) > 0;
end;

{ The version of an Earth Studio already installed, or '' if there is none. }
function InstalledVersion: String;
begin
  Result := '';
  if not RegQueryStringValue(HKCU, UninstallKey, 'DisplayVersion', Result) then
    RegQueryStringValue(HKLM, UninstallKey, 'DisplayVersion', Result);
end;

{ An update says so on the welcome page instead of describing a fresh install. }
procedure InitializeWizard;
var
  Installed: String;
begin
  Installed := InstalledVersion;
  if Installed = '' then
    Exit;
  WizardForm.WelcomeLabel1.Caption := CustomMessage('UpdateTitle');
  if Installed = '{#AppVersion}' then
    WizardForm.WelcomeLabel2.Caption := FmtMessage(CustomMessage('ReinstallText'), [Installed])
  else
    WizardForm.WelcomeLabel2.Caption := FmtMessage(CustomMessage('UpdateText'), [Installed, '{#AppVersion}']);
end;

{ Path entries are compared without case and without a trailing backslash. }
function SamePath(A, B: String): Boolean;
begin
  Result := CompareText(RemoveBackslashUnlessRoot(Trim(A)), RemoveBackslashUnlessRoot(Trim(B))) = 0;
end;

{ The user's PATH gains the install folder, so "earth" works in any new
  terminal. Uninstalling, or unchecking the task on an update, removes it. }
procedure AddToPath(Dir: String);
var
  Paths, Rest, Entry: String;
  Split: Integer;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', Paths) then
    Paths := '';
  Rest := Paths + ';';
  while Rest <> '' do
  begin
    Split := Pos(';', Rest);
    Entry := Copy(Rest, 1, Split - 1);
    Delete(Rest, 1, Split);
    if SamePath(Entry, Dir) then
      Exit;
  end;
  if (Paths <> '') and (Paths[Length(Paths)] <> ';') then
    Paths := Paths + ';';
  RegWriteExpandStringValue(HKCU, 'Environment', 'Path', Paths + Dir);
end;

procedure RemoveFromPath(Dir: String);
var
  Paths, Entry, Kept: String;
  Split: Integer;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', Paths) then
    Exit;
  Kept := '';
  Paths := Paths + ';';
  while Paths <> '' do
  begin
    Split := Pos(';', Paths);
    Entry := Copy(Paths, 1, Split - 1);
    Delete(Paths, 1, Split);
    if (Trim(Entry) <> '') and not SamePath(Entry, Dir) then
    begin
      if Kept <> '' then
        Kept := Kept + ';';
      Kept := Kept + Entry;
    end;
  end;
  RegWriteExpandStringValue(HKCU, 'Environment', 'Path', Kept);
end;

{ Unchecking an option on an update undoes what an earlier install set up. }
procedure CurStepChanged(CurStep: TSetupStep);
var
  Current: String;
begin
  if CurStep <> ssPostInstall then
    Exit;
  if WizardIsTaskSelected('addtopath') then
    AddToPath(ExpandConstant('{app}'))
  else
    RemoveFromPath(ExpandConstant('{app}'));
  if not WizardIsTaskSelected('associate') then
  begin
    if RegQueryStringValue(HKCU, 'Software\Classes\.earth', '', Current) and (Current = 'EarthStudio.Program') then
      RegDeleteValue(HKCU, 'Software\Classes\.earth', '');
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Classes\EarthStudio.Program');
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RemoveFromPath(ExpandConstant('{app}'));
end;
