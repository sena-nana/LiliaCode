!macro LILIA_WRITE_PATH_SCRIPT SCRIPT_PATH MODE
  FileOpen $0 "${SCRIPT_PATH}" w
  FileWrite $0 "$$installDir = @'$\r$\n"
  FileWrite $0 "$INSTDIR$\r$\n"
  FileWrite $0 "'@$\r$\n"
  FileWrite $0 "$$target = [IO.Path]::GetFullPath($$installDir).TrimEnd('\')$\r$\n"
  FileWrite $0 "$$current = [Environment]::GetEnvironmentVariable('Path', 'User')$\r$\n"
  FileWrite $0 "$$parts = @()$\r$\n"
  FileWrite $0 "if ($$current) {$\r$\n"
  FileWrite $0 "  $$parts = $$current -split ';' | Where-Object {$\r$\n"
  FileWrite $0 "    $$_ -and ([IO.Path]::GetFullPath($$_).TrimEnd('\') -ine $$target)$\r$\n"
  FileWrite $0 "  }$\r$\n"
  FileWrite $0 "}$\r$\n"
  !if "${MODE}" == "install"
    FileWrite $0 "$$parts += $$target$\r$\n"
  !endif
  FileWrite $0 "[Environment]::SetEnvironmentVariable('Path', ($$parts -join ';'), 'User')$\r$\n"
  FileClose $0
!macroend

!macro LILIA_RUN_PATH_SCRIPT MODE
  StrCpy $1 "$TEMP\liliacode-path-${MODE}.ps1"
  !insertmacro LILIA_WRITE_PATH_SCRIPT "$1" "${MODE}"
  ExecWait 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$1"'
  Delete "$1"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  FileOpen $0 "$INSTDIR\liliacode.cmd" w
  FileWrite $0 "@echo off$\r$\n"
  FileWrite $0 "$\"%~dp0LiliaCode.exe$\" %*$\r$\n"
  FileClose $0
  !insertmacro LILIA_RUN_PATH_SCRIPT "install"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro LILIA_RUN_PATH_SCRIPT "uninstall"
  Delete "$INSTDIR\liliacode.cmd"
!macroend
