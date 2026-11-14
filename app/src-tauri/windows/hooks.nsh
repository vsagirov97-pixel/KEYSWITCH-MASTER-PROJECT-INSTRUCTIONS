; NSIS installer hooks (Tauri `bundle.windows.nsis.installerHooks`).
; Runs after the Tauri 2 template's own uninstall steps; `$DeleteAppDataCheckboxState` is the
; template's "delete app data" checkbox on the confirm page.
;
; Autostart: the per-user HKCU Run value "KeySwitch" (`ks_platform_win::autostart`,
; name = APP_NAME in src/os.rs). A real uninstall removes exactly that value, so an
; uninstalled app is never started at sign-in; nothing else in the Run key is touched.
; When the installer runs the old uninstaller during an upgrade (`/UPDATE`), the value is
; kept: the user's autostart choice survives the update.
;
; App data (QA-WF-04): the settings, including the Macro Studio text, live in
; `%APPDATA%\KeySwitch` (src/settings.rs), which the template does not know (it removes
; `$APPDATA\<bundle id>`). On a real uninstall (never with `/UPDATE`) with the checkbox
; ticked - or, for a silent uninstall that has no checkbox, with `/DELETEAPPDATA` - exactly
; that folder is removed. Nothing else in %APPDATA% is touched.
; Verified by the installer round trip in the `Windows build` CI job.
!macro NSIS_HOOK_POSTUNINSTALL
  Push $R0
  Push $R1
  ClearErrors
  ${GetOptions} $CMDLINE "/UPDATE" $R0
  ${If} ${Errors}
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KeySwitch"
    StrCpy $R1 0
    ${If} $DeleteAppDataCheckboxState = 1
      StrCpy $R1 1
    ${EndIf}
    ${If} ${Silent}
      ClearErrors
      ${GetOptions} $CMDLINE "/DELETEAPPDATA" $R0
      ${IfNot} ${Errors}
        StrCpy $R1 1
      ${EndIf}
    ${EndIf}
    ${If} $R1 = 1
      SetShellVarContext current
      ; Never "\KeySwitch" at the drive root if the folder could not be resolved.
      ${If} $APPDATA != ""
        RMDir /r "$APPDATA\KeySwitch"
      ${EndIf}
    ${EndIf}
  ${EndIf}
  Pop $R1
  Pop $R0
!macroend
