; Windows limits how far an app may change the gamma ramp. Good Night needs the full
; range for deep warmth and dimming to black, the same setting f.lux relies on.
!macro NSIS_HOOK_POSTINSTALL
  SetRegView 64
  WriteRegDWORD HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM" "GdiIcmGammaRange" 256
!macroend

; Tauri's NSIS template forwards the updater's `/UPDATE` flag to the uninstaller as $UpdateMode.
; A real uninstall (Settings -> Apps) leaves it unset; an in-app update must preserve this
; user's settings, cache and gamma value, because the user and the app come straight back.
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
    RMDir /r "$APPDATA\com.libertydesignstudio.goodnight"
    RMDir /r "$LOCALAPPDATA\com.libertydesignstudio.goodnight"
    ; Restore the installer's context for Tauri's own shortcut and registry cleanup.
    !insertmacro SetContext
  ${EndIf}
!macroend
