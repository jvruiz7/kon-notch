; Instalador do Kon (por usuário, sem precisar de administrador)
; Compilado com: makensis -DVERSION=x.y.z -DSRC=<win-unpacked> -DOUT=<Setup.exe> build/installer.nsi
Unicode true
SetCompressor /SOLID lzma
SetCompressorDictSize 64
RequestExecutionLevel user
ManifestDPIAware true

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!define APP "Kon"
!define EXE "Kon.exe"
!define UNKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\app.kon.desktop"

Name "${APP}"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\Programs\Kon"
InstallDirRegKey HKCU "Software\Kon" "InstallDir"
BrandingText "Kon ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Kon"
VIAddVersionKey "FileDescription" "Instalador do Kon"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "Kon"

!define MUI_ICON "icon.ico"
!define MUI_UNICON "icon.ico"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Instalar o Kon ${VERSION}"
!define MUI_WELCOMEPAGE_TEXT "O Kon é uma ilha dinâmica para o Windows: música, Claude Code, arquivos, clima e IA no topo da tela.$\r$\n$\r$\nA instalação é só para o seu usuário e não pede permissão de administrador."
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "Abrir o Kon agora"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "PortugueseBR"

Var Params
Var Updating
Function .onInit
  ${GetParameters} $Params
  StrCpy $Updating "0"
  ClearErrors
  ${GetOptions} $Params "--updated" $0
  ${IfNot} ${Errors}
    StrCpy $Updating "1"
  ${EndIf}
FunctionEnd
Function .onInstSuccess
  ClearErrors
  ${GetOptions} $Params "--force-run" $0
  ${IfNot} ${Errors}
    Exec '"$INSTDIR\${EXE}"'
  ${EndIf}
FunctionEnd

; Fecha o Kon SEM "/T": numa atualização automática o instalador é filho do Kon, e "/T" (árvore de
; processos) derrubava o próprio instalador antes de instalar.
Function CloseKon
  ${If} $Updating == "1"
    Sleep 2000 ; o Kon fecha sozinho depois de abrir o instalador
  ${EndIf}
  nsExec::Exec 'taskkill /F /IM "${EXE}"'
  Pop $0
  Sleep 800
FunctionEnd
Function un.CloseKon
  nsExec::Exec 'taskkill /F /IM "${EXE}"'
  Pop $0
  Sleep 800
FunctionEnd

Section "Kon (obrigatório)" SecMain
  SectionIn RO
  Call CloseKon
  SetOutPath "$INSTDIR"
  RMDir /r "$INSTDIR\resources"
  File /r "${SRC}\*.*"
  WriteUninstaller "$INSTDIR\Desinstalar Kon.exe"
  CreateShortcut "$SMPROGRAMS\Kon.lnk" "$INSTDIR\${EXE}" "" "$INSTDIR\${EXE}" 0
  WriteRegStr HKCU "Software\Kon" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNKEY}" "DisplayName" "Kon"
  WriteRegStr HKCU "${UNKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNKEY}" "Publisher" "Kon"
  WriteRegStr HKCU "${UNKEY}" "DisplayIcon" "$INSTDIR\${EXE},0"
  WriteRegStr HKCU "${UNKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNKEY}" "UninstallString" '"$INSTDIR\Desinstalar Kon.exe"'
  WriteRegStr HKCU "${UNKEY}" "QuietUninstallString" '"$INSTDIR\Desinstalar Kon.exe" /S'
  WriteRegDWORD HKCU "${UNKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNKEY}" "NoRepair" 1
SectionEnd

Section "Atalho na área de trabalho" SecDesk
  ; numa atualização silenciosa, só recria o atalho se ele já existia
  ${If} ${Silent}
  ${AndIfNot} ${FileExists} "$DESKTOP\Kon.lnk"
    Return
  ${EndIf}
  CreateShortcut "$DESKTOP\Kon.lnk" "$INSTDIR\${EXE}" "" "$INSTDIR\${EXE}" 0
SectionEnd

Section "Iniciar com o Windows" SecRun
  ${If} ${Silent}
    Return ; atualização: respeita o que já está configurado no app
  ${EndIf}
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "app.kon.desktop" '"$INSTDIR\${EXE}"'
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "Os arquivos do Kon e o atalho no Menu Iniciar."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesk} "Cria um atalho do Kon na área de trabalho."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecRun} "Abre o Kon automaticamente quando você entra no Windows."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Call un.CloseKon
  ; remove os hooks do Kon em ~/.claude/settings.json (mantém o resto da configuração)
  IfFileExists "$INSTDIR\${EXE}" 0 +2
    ExecWait '"$INSTDIR\${EXE}" --remove-hooks'
  Delete "$SMPROGRAMS\Kon.lnk"
  Delete "$DESKTOP\Kon.lnk"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kon"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "app.kon.desktop"
  DeleteRegKey HKCU "${UNKEY}"
  DeleteRegKey HKCU "Software\Kon"
  RMDir /r "$INSTDIR"
SectionEnd
