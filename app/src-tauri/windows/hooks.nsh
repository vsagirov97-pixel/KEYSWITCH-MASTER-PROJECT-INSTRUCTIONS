; NSIS installer hooks (Tauri `bundle.windows.nsis.installerHooks`).
; The app's autostart is the per-user HKCU Run value "KeySwitch"
; (`ks_platform_win::autostart`, name = APP_NAME in src/os.rs). A real uninstall removes
; exactly that value, so an uninstalled app is never started at sign-in; nothing else in
; the Run key is touched. When the installer runs the old uninstaller during an upgrade
; (`/UPDATE`), the value is kept: the user's autostart choice survives the update.
; Verified by the installer round trip in the `Windows build` CI job.
!macro NSIS_HOOK_POSTUNINSTALL
  Push $R0
  ClearErrors
  ${GetOptions} $CMDLINE "/UPDATE" $R0
  ${If} ${Errors}
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KeySwitch"
  ${EndIf}
  Pop $R0
!macroend
