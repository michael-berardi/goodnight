; Windows limits how far an app may change the gamma ramp. Good Night needs the full
; range for deep warmth and dimming to black, the same setting f.lux relies on.
!macro NSIS_HOOK_POSTINSTALL
  SetRegView 64
  WriteRegDWORD HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM" "GdiIcmGammaRange" 256
!macroend

; Tauri's NSIS template forwards the updater's `/UPDATE` flag to the uninstaller as $UpdateMode.
; A real uninstall removes only what this installer wrote and the uninstalling profile's data;
; an in-app update must preserve settings, cache and gamma value, because the user and the app
; come straight back.
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    SetRegView 64
    ReadRegDWORD $0 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM" "GdiIcmGammaRange"
    IntCmp $0 256 0 goodnight_gamma_kept goodnight_gamma_kept
    DeleteRegValue HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM" "GdiIcmGammaRange"
    goodnight_gamma_kept:

    ; This installer is perMachine, so its shell context normally points at all users.
    ; Profile data belongs to the account running the uninstaller, not to that shared folder.
    SetShellVarContext current
    ; Good Night is often still running: the app and its WebView2 child processes keep writing
    ; into the folders below. Tauri's own running-app check runs after this hook and stops only
    ; goodnight.exe, which leaves the WebView2 processes behind. Stop the whole tree first, then
    ; wait for the folders to become removable. Silent uninstalls get this too, so nothing has to
    ; be clicked.
    ExecWait '"$SYSDIR\taskkill.exe" /F /T /IM "${MAINBINARYNAME}.exe"'
    Push $R1
    Push $R2
    StrCpy $R2 0
    goodnight_data_retry:
      IntOp $R2 $R2 + 1
      RMDir /r "$APPDATA\com.libertydesignstudio.goodnight"
      RMDir /r "$LOCALAPPDATA\com.libertydesignstudio.goodnight"
      ${IfNot} ${FileExists} "$APPDATA\com.libertydesignstudio.goodnight\*.*"
      ${AndIfNot} ${FileExists} "$LOCALAPPDATA\com.libertydesignstudio.goodnight\*.*"
        Goto goodnight_data_gone
      ${EndIf}
      ${If} $R2 < 20
        Sleep 250
        Goto goodnight_data_retry
      ${EndIf}
      ; Still held after five seconds: the next start removes whatever is left.
      RMDir /r /REBOOTOK "$APPDATA\com.libertydesignstudio.goodnight"
      RMDir /r /REBOOTOK "$LOCALAPPDATA\com.libertydesignstudio.goodnight"
    goodnight_data_gone:
    Pop $R2
    Pop $R1
    ; Restore the installer's context for Tauri's own shortcut and registry cleanup.
    !insertmacro SetContext
  ${EndIf}
!macroend

; A real uninstall also deletes the update cache and log next to settings.json (normally gone
; with the folder above already) and every updater staging folder left in %TEMP%. During an
; in-app update both hooks skip their cleanup; the new version's own sweep (updates.rs,
; remove_staged_installers) deletes the staged installer a minute after it starts.
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
