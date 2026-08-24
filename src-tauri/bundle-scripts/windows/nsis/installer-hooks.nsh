; 卸载前清理当前用户的自启动注册表项和管理员计划任务。
!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-cleanup' $0
  ${EndIf}
!macroend
