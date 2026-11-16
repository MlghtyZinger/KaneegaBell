; KaneegaBell keeps its data in a "data" folder next to the program (library,
; covers, keys, WebView2's files). Updates and a normal uninstall leave it
; alone; ticking "Delete the application data" when uninstalling removes it.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
  ${AndIf} ${FileExists} "$INSTDIR\data\library.db"
    RmDir /r "$INSTDIR\data"
    RmDir "$INSTDIR"
  ${EndIf}
!macroend
