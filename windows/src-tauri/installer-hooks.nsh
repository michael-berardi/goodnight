; Windows limits how far an app may change the gamma ramp. Good Night needs the full
; range for deep warmth and dimming to black, the same setting f.lux relies on.
!macro NSIS_HOOK_POSTINSTALL
  SetRegView 64
  WriteRegDWORD HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion\ICM" "GdiIcmGammaRange" 256
!macroend
