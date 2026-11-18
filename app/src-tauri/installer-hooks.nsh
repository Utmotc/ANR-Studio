; ANR Studio 的 NSIS 安装钩子
;
; 两个真实场景：
; 1. 应用点关闭只是「收进托盘」，进程还在，anr-studio.exe 被占用。
;    Tauri 自带的 Restart Manager 检查在「旧版本以管理员身份运行」时看不到进程，
;    于是 NSIS 直接甩一句 "Error opening file for writing"，用户根本不知道要关软件。
; 2. 安装目录（比如 D:\SoftWare\ANR Studio）由管理员创建 / 当前用户没有写权限，
;    这时只能靠「以管理员身份运行」安装包解决，同样需要一句人话提示。
;
; 所以拷文件之前先探测目标 exe 能否写入：不能写就引导用户关闭进程；
; 关掉了还写不进去，就明确让他去以管理员身份运行。静默安装时不能弹框卡住脚本，
; 一律直接结束进程后继续。

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
      "安装目录不可写：$\r$\n$INSTDIR$\r$\n$\r$\n通常是这个目录由管理员创建（或当前用户没有写权限）。$\r$\n请右键安装包选择「以管理员身份运行」，或在向导里换成你自己能写的目录。"
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
            "还是写不进安装目录：$\r$\n${ANR_EXE}$\r$\n$\r$\n可能有一个以管理员身份运行的 ANR Studio 还没退出。$\r$\n请彻底退出它，然后右键安装包选择「以管理员身份运行」。"
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
