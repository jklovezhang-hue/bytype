; ByType installer hooks (referenced by tauri.conf.json bundle.windows.nsis.installerHooks).
; After uninstall, remove the autostart entry that tauri-plugin-autostart writes to
; HKCU\...\Run at runtime. The NSIS uninstaller does not know about this runtime-written
; value, so without this it would be left behind (a dangling entry pointing at the deleted
; exe; Windows silently ignores it, but it is registry cruft). Deleting a missing value is
; a harmless no-op, so both possible name casings are covered.
!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "ByType"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "bytype"
!macroend
