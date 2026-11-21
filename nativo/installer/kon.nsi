; Instalador do Kon nativo (Rust), por usuário, sem pedir administrador.
; Compilado no CI com: makensis -DVERSION=x.y.z installer\kon.nsi  (gera installer\KonSetup.exe)
Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel user
ManifestDPIAware true

!include "MUI2.nsh"

!define APP "Kon"
!define EXE "Kon.exe"
!define UNKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\KonNativo"

Name "${APP}"
OutFile "KonSetup.exe"
InstallDir "$LOCALAPPDATA\Programs\KonNativo"
BrandingText "Kon ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Kon"
VIAddVersionKey "FileDescription" "Instalador do Kon"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Kon"

!define MUI_ICON "..\assets\icon.ico"
!define MUI_UNICON "..\assets\icon.ico"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Instalar o Kon ${VERSION}"
!define MUI_WELCOMEPAGE_TEXT "O Kon é uma ilha dinâmica para o Windows: música, Claude Code, arquivos, clima e IA no topo da tela.$\r$\n$\r$\nEsta é a versão nativa (Rust): leve, rápida e sem navegador embutido.$\r$\n$\r$\nA instalação é só para o seu usuário e não pede permissão de administrador."
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Abrir o Kon agora"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "PortugueseBR"

!macro CloseKon
  ; fecha só o Kon desta pasta (o Kon antigo, em Electron, fica em outra pasta e não é tocado aqui)
  nsExec::Exec `powershell -NoProfile -NonInteractive -Command "Get-Process | Where-Object { $$_.Path -eq '$INSTDIR\${EXE}' } | Stop-Process -Force -ErrorAction SilentlyContinue"`
  Sleep 700
!macroend

Section "Kon" SecMain
  SectionIn RO
  !insertmacro CloseKon
  SetOutPath "$INSTDIR"
  Delete "$INSTDIR\Kon.old.exe"
  File "/oname=${EXE}" "..\target\release\kon.exe"
  WriteUninstaller "$INSTDIR\Desinstalar.exe"
  CreateShortcut "$SMPROGRAMS\Kon (nativo).lnk" "$INSTDIR\${EXE}" "" "$INSTDIR\${EXE}" 0
  WriteRegStr HKCU "${UNKEY}" "DisplayName" "Kon (nativo)"
  WriteRegStr HKCU "${UNKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNKEY}" "Publisher" "jvruiz7"
  WriteRegStr HKCU "${UNKEY}" "DisplayIcon" "$INSTDIR\${EXE}"
  WriteRegStr HKCU "${UNKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNKEY}" "UninstallString" '"$INSTDIR\Desinstalar.exe"'
  WriteRegDWORD HKCU "${UNKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNKEY}" "NoRepair" 1
  ; abre com o Windows (pode desligar em Configurações do Kon)
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KonNativo" '"$INSTDIR\${EXE}"'
SectionEnd

Section "Uninstall"
  !insertmacro CloseKon
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\Kon.old.exe"
  Delete "$INSTDIR\Desinstalar.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Kon (nativo).lnk"
  DeleteRegKey HKCU "${UNKEY}"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KonNativo"
  ; seus ajustes ficam em %APPDATA%\KonNativo caso instale de novo
SectionEnd
