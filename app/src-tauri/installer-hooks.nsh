; ANR Studio 的 NSIS 安装钩子
;
; 三个真实场景：
; 1. 应用点关闭只是「收进托盘」，进程还在，anr-studio.exe 被占用。
;    Tauri 自带的 Restart Manager 检查在「旧版本以管理员身份运行」时看不到进程，
;    于是 NSIS 直接甩一句 "Error opening file for writing"，用户根本不知道要关软件。
;    所以安装前、卸载前都先 taskkill，实在杀不掉再引导用户提权。
; 2. 安装目录（比如 D:\SoftWare\ANR Studio）由管理员创建 / 当前用户没有写权限，
;    这时只能靠「以管理员身份运行」安装包解决，同样需要一句人话提示。
; 3. 以管理员权限装过之后，普通权限的卸载程序看不到 / 杀不掉那个提权进程，
;    结果「App 列表里点了卸载却没反应」。卸载钩子先杀进程，杀不掉/目录写不了
;    就自动用管理员权限重新拉起卸载程序（UAC 一次），而不是让用户自己猜。
;
; 普通用户推荐默认目录（%LOCALAPPDATA%\ANR Studio）：安装和卸载都不需要管理员。

; 安装完成后不要默认勾选「运行 ANR Studio」。
; 如果安装包是以管理员权限运行的，默认勾选会把应用也以管理员权限启动，
; 之后普通权限的卸载程序就看不到、也杀不掉它，于是表现为「卸载没反应」。
; 本文件在 Tauri 模板定义 MUI_PAGE_FINISH 之前被 include，所以这个 define 生效。
!define MUI_FINISHPAGE_RUN_NOTCHECKED

!macro NSIS_HOOK_PREINSTALL
  !define ANR_ID ${__LINE__}
  !define ANR_EXE "$INSTDIR\anr-studio.exe"

  ; 0) 先确认安装目录本身能写。
  ;    首次安装 / 目录被清空时 anr-studio.exe 还不存在，下面那段"文件被占用"的探测
  ;    会整段跳过，结果 NSIS 直到拷文件时才甩一句英文
  ;    "Error opening file for writing" —— 用户根本不知道是目录没写权限。
  ;    所以这里先真写一个探测文件，写不进就直接给中文提示。
  ClearErrors
  CreateDirectory "$INSTDIR"
  FileOpen $9 "$INSTDIR\anr-write-probe.tmp" w
  ${If} ${Errors}
    ; 静默安装不能弹框卡住脚本
    IfSilent anr_dir_abort_${ANR_ID}
    MessageBox MB_ICONSTOP|MB_OK \
      "安装目录不可写：$\r$\n$INSTDIR$\r$\n$\r$\n通常是这个目录由管理员创建，或当前用户没有写权限。$\r$\n$\r$\n推荐做法：退回到默认目录（当前用户目录，不需要管理员）。$\r$\n如果必须装在这里，请右键安装包选择「以管理员身份运行」；$\r$\n但这样以后卸载也会需要管理员权限。"
    anr_dir_abort_${ANR_ID}:
    Abort
  ${EndIf}
  FileClose $9
  Delete "$INSTDIR\anr-write-probe.tmp"

  ${If} ${FileExists} "${ANR_EXE}"
    ClearErrors
    FileOpen $9 "${ANR_EXE}" a
    ${If} ${Errors}
      ; 静默安装直接关进程，不弹框
      IfSilent anr_kill_${ANR_ID}
      MessageBox MB_OKCANCEL|MB_ICONQUESTION \
        "安装前需要关闭正在运行的 ANR Studio（含托盘里的图标）。$\r$\n$\r$\n点「确定」立即关闭并继续安装；点「取消」请先自己退出，再重新运行安装包。" \
        IDOK anr_kill_${ANR_ID} IDCANCEL anr_stop_${ANR_ID}

      anr_stop_${ANR_ID}:
        Abort

      anr_kill_${ANR_ID}:
        DetailPrint "正在关闭 ANR Studio…"
        ; 先正常结束（让它自己收尾），再强杀托盘残留
        nsExec::Exec 'taskkill /IM anr-studio.exe'
        Pop $9
        Sleep 800
        nsExec::Exec 'taskkill /F /IM anr-studio.exe'
        Pop $9
        Sleep 700

        ClearErrors
        FileOpen $9 "${ANR_EXE}" a
        ${If} ${Errors}
          ; 进程可能还在退出、或被杀软短暂占用，再等一次就基本能写
          Sleep 1500
          nsExec::Exec 'taskkill /F /IM anr-studio.exe'
          Pop $9
          ClearErrors
          FileOpen $9 "${ANR_EXE}" a
        ${EndIf}

        ${If} ${Errors}
          IfSilent anr_fail_${ANR_ID}
          MessageBox MB_ICONSTOP|MB_OK \
            "还是写不进安装目录：$\r$\n${ANR_EXE}$\r$\n$\r$\n可能有一个以管理员身份运行的 ANR Studio 还没退出。$\r$\n请从托盘右键退出它；如果它是以管理员身份启动的，需要右键安装包选择「以管理员身份运行」。"
          anr_fail_${ANR_ID}:
          Abort
        ${EndIf}
        FileClose $9
    ${Else}
      FileClose $9
    ${EndIf}
  ${EndIf}

  !undef ANR_EXE
  !undef ANR_ID
!macroend

; 卸载前：先结束进程（含托盘里的、以及以管理员身份启动的），再探测安装目录能否写。
; 普通权限处理不了就自动用管理员权限重新拉起卸载程序；已经提权过还失败就明确报错，
; 不进入「UAC 死循环」。
!macro NSIS_HOOK_PREUNINSTALL
  !define ANR_UN_ID ${__LINE__}

  DetailPrint "正在关闭 ANR Studio…"
  nsExec::Exec 'taskkill /IM anr-studio.exe'
  Pop $9
  Sleep 700
  nsExec::Exec 'taskkill /F /IM anr-studio.exe'
  Pop $9
  Sleep 500

  ; exe 还能以写方式打开 = 进程已经退出、目录也能写
  ClearErrors
  FileOpen $9 "$INSTDIR\anr-studio.exe" a
  ${If} ${Errors}
    ; 进程可能还在退出、或杀软短暂占用，等一次再确认
    Sleep 1500
    nsExec::Exec 'taskkill /F /IM anr-studio.exe'
    Pop $9
    ClearErrors
    FileOpen $9 "$INSTDIR\anr-studio.exe" a
  ${EndIf}

  ${If} ${Errors}
    ; 已经带提权标记重跑过，还是不行：别再弹 UAC，直接给出可操作提示
    ; （不能用 ${StrLoc}：StrFunc 在卸载段必须用 un.StrLoc，这里改用 FileFunc 的 GetOptions）
    ClearErrors
    ${GetOptions} $CMDLINE "/ANR_ELEVATED" $R8
    ${IfNot} ${Errors}
      IfSilent anr_un_abort_${ANR_UN_ID}
      MessageBox MB_ICONSTOP|MB_OK \
        "仍然无法结束 ANR Studio，或安装目录不可写。$\r$\n$\r$\n请从托盘右键退出应用，再重新卸载。$\r$\n如果仍然失败，可以手动删除安装目录，并在注册表里删除：$\r$\nHKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\ANR Studio"
      anr_un_abort_${ANR_UN_ID}:
      Abort
    ${EndIf}

    ; 保留原始卸载意图：更新（/UPDATE）、静默（/S）、被动（/P）
    StrCpy $R0 "/ANR_ELEVATED"
    ${If} $UpdateMode = 1
      StrCpy $R0 "$R0 /UPDATE"
    ${Else}
      ${If} ${Silent}
        StrCpy $R0 "$R0 /S"
      ${ElseIf} $PassiveMode = 1
        StrCpy $R0 "$R0 /P"
      ${EndIf}
    ${EndIf}

    IfSilent anr_un_elevate_${ANR_UN_ID}
    MessageBox MB_ICONINFORMATION|MB_OK \
      "卸载需要管理员权限：ANR Studio 可能还在托盘里运行，或它是以管理员身份安装/启动的。$\r$\n$\r$\n点「确定」后 Windows 会请求管理员权限，并继续卸载。"
    anr_un_elevate_${ANR_UN_ID}:
    ExecShell "runas" "$INSTDIR\uninstall.exe" "$R0"
    Quit
  ${Else}
    FileClose $9
  ${EndIf}

  !undef ANR_UN_ID
!macroend
