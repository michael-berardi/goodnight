; Windows limits how far an app may change the gamma ramp. Good Night needs the full
; range for deep warmth and dimming to black, the same setting f.lux relies on.
!macro NSIS_HOOK_POSTINSTALL
  SetRegView 64
  WriteRegDWORD HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM" "GdiIcmGammaRange" 256
!macroend

; Uninstalling removes the update cache and log next to settings.json, and any installer an
; update left in the temp folder. Settings go only with "Delete the application data".
; Updates run this uninstaller too; they keep everything.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    SetShellVarContext current
    Delete "$APPDATA\${BUNDLEID}\updates.json"
    Delete "$APPDATA\${BUNDLEID}\updates.json.tmp"
    Delete "$APPDATA\${BUNDLEID}\updates.log"
    SetShellVarContext all
    Push $0
    Push $1
    FindFirst $0 $1 "$TEMP\${PRODUCTNAME}-*-updater-*"
    ${DoWhile} $1 != ""
      RMDir /r "$TEMP\$1"
      FindNext $0 $1
    ${Loop}
    FindClose $0
    Pop $1
    Pop $0
  ${EndIf}
!macroend
