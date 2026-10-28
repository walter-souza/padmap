; Instalador do padmap para Windows (Inno Setup 6). O workflow de release o
; compila com a versão da tag:
;   iscc /DAppVersion=0.2.0 /DNumericVersion=0.2.0 installer\padmap.iss
; Os caminhos são relativos a esta pasta.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
; Só números (sem "-beta.1"), para a versão do arquivo.
#ifndef NumericVersion
  #define NumericVersion "0.0.0"
#endif

[Setup]
; O AppId identifica o padmap entre versões: não mude, ou a atualização vira
; uma segunda instalação.
AppId={{928758CF-EA61-4766-AE50-12D6C094C7ED}
AppName=padmap
AppVersion={#AppVersion}
AppVerName=padmap {#AppVersion}
AppPublisher=walter-souza
AppPublisherURL=https://github.com/walter-souza/padmap
AppSupportURL=https://github.com/walter-souza/padmap/issues
AppUpdatesURL=https://github.com/walter-souza/padmap/releases
VersionInfoVersion={#NumericVersion}
DefaultDirName={autopf}\padmap
DefaultGroupName=padmap
DisableProgramGroupPage=yes
; Instala só para o usuário, sem pedir administrador. A janela inicial deixa
; escolher instalar para todos.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
LicenseFile=..\LICENSE
OutputDir=..\dist
OutputBaseFilename=padmap-v{#AppVersion}-windows-x64-setup
UninstallDisplayIcon={app}\padmap.exe
SetupIconFile=..\assets\padmap.ico
UninstallDisplayName=padmap
; Fecha o padmap aberto antes de atualizar.
CloseApplications=yes
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "ptbr"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\target\release\padmap.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\padmap"; Filename: "{app}\padmap.exe"
Name: "{autodesktop}\padmap"; Filename: "{app}\padmap.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\padmap.exe"; Description: "{cm:LaunchProgram,padmap}"; Flags: nowait postinstall skipifsilent

; Os perfis ficam na pasta de dados do padmap, fora de {app}: desinstalar não
; apaga nada do usuário.
