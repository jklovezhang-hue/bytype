; ByType installer hooks (referenced by tauri.conf.json bundle.windows.nsis.installerHooks).
; After uninstall:
;   1. Remove the autostart registry entry written at runtime by tauri-plugin-autostart.
;      (Two casings in case the registry key name ever changes.)
;   2. When the user checks "Delete app data", also remove runtime-written files that the
;      NSIS uninstaller does not track: config.toml and the models directory (228 MB).
;      The Tauri template already removes %LOCALAPPDATA%\com.bytype.app (WebView2 cache)
;      when $DeleteAppDataCheckboxState == 1, so we do not duplicate that here.
;      After removing those files $INSTDIR should be empty; RMDir (no /r) removes it
;      silently if it is empty and is a no-op if anything still remains.
;      $DeleteAppDataCheckboxState and $UpdateMode are declared in the Tauri NSIS template.
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "ByType"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "bytype"

  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    Delete "$INSTDIR\config.toml"
    RMDir /r "$INSTDIR\models"
    RMDir "$INSTDIR"
  ${EndIf}
!macroend
