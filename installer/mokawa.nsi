Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
Name "Mokawa"
OutFile "..\dist\Mokawa-0.1.0-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\Mokawa"
RequestExecutionLevel user
VIProductVersion "0.1.0.0"
VIAddVersionKey "ProductName" "Mokawa"
VIAddVersionKey "FileDescription" "Mokawa installer"
VIAddVersionKey "FileVersion" "0.1.0"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK "Mokawa requires Windows x64."
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd
Section "Mokawa"
  SetOutPath "$INSTDIR"
  File "..\target\release\mokawa.exe"
  File "..\target\release\mokawa-auth.exe"
  File "..\LICENSE"
  File "..\README.md"
  File "..\THIRD_PARTY.md"
  File "..\dist\dependency-licenses.json"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateShortcut "$SMPROGRAMS\Mokawa.lnk" "$INSTDIR\mokawa.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Mokawa" "DisplayName" "Mokawa"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Mokawa" "DisplayVersion" "0.1.0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Mokawa" "Publisher" "Mokawa"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Mokawa" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
SectionEnd
Section "Uninstall"
  SetRegView 64
  Delete "$SMPROGRAMS\Mokawa.lnk"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Mokawa"
  Delete "$INSTDIR\mokawa.exe"
  Delete "$INSTDIR\mokawa-auth.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\THIRD_PARTY.md"
  Delete "$INSTDIR\dependency-licenses.json"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
