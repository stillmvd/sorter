!macro SORTER_VERB KEY ARG
  WriteRegStr SHCTX "Software\Classes\${KEY}\shell\Sorter" "MUIVerb" "Разобрать с Sorter"
  WriteRegStr SHCTX "Software\Classes\${KEY}\shell\Sorter" "Icon" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\",0"
  WriteRegStr SHCTX "Software\Classes\${KEY}\shell\Sorter" "MultiSelectModel" "Single"
  WriteRegStr SHCTX "Software\Classes\${KEY}\shell\Sorter\command" "" "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" $\"${ARG}$\""
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro SORTER_VERB "Directory" "%1"
  !insertmacro SORTER_VERB "Directory\Background" "%V"
  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey SHCTX "Software\Classes\Directory\shell\Sorter"
  DeleteRegKey SHCTX "Software\Classes\Directory\Background\shell\Sorter"
  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)"
!macroend
