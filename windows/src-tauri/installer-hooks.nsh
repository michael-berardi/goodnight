; Windows limits how far an app may change the gamma ramp. Good Night needs the full
; range for deep warmth and dimming to black, the same setting f.lux relies on.
!macro NSIS_HOOK_POSTINSTALL
  SetRegView 64
  WriteRegDWORD HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM" "GdiIcmGammaRange" 256
!macroend

; Uninstalling leaves nothing behind, so nothing has to be hunted for later (PRIVACY.md lists
; every path and lifetime). The machine-wide value is removed only while it still holds what
; this installer wrote, so a change made by another app survives. The per-user folders are the
; profile that runs the uninstaller; the elevated uninstaller cannot reach other profiles.
!macro NSIS_HOOK_PREUNINSTALL
  SetRegView 64
  ReadRegDWORD $0 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM" "GdiIcmGammaRange"
  IntCmp $0 256 0 goodnight_gamma_kept goodnight_gamma_kept
  DeleteRegValue HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM" "GdiIcmGammaRange"
  goodnight_gamma_kept:
  RMDir /r "$APPDATA\com.libertydesignstudio.goodnight"
  RMDir /r "$LOCALAPPDATA\com.libertydesignstudio.goodnight"
!macroend
