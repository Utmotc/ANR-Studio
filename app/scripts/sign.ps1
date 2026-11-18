# 给 ANR Studio 的 exe / 安装包做 Authenticode 签名。
#
# SmartScreen 弹「Windows 已保护你的电脑 / 发布者未知」的唯一根治办法就是签名：
# 无证书的文件发布者永远是「未知」，点多少次「仍要运行」也不会变成可信。
#
# 用法（PFX 文件）：
#   .\sign.ps1 -File ..\src-tauri\target\release\anr-studio.exe -Pfx D:\cert\anr.pfx -Password '******'
# 用法（证书已装在证书存储里，按指纹签）：
#   .\sign.ps1 -File ..\src-tauri\target\release\anr-studio.exe -Thumbprint 1A2B3C...
#
# 签完可以用 .\sign.ps1 -Verify <文件> 检查。

param(
  [string]$File,
  [string]$Pfx,
  [string]$Password,
  [string]$Thumbprint,
  [string]$TimestampUrl = "http://timestamp.digicert.com",
  [switch]$Verify
)

$ErrorActionPreference = 'Stop'

function Find-SignTool {
  $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
  if (Test-Path $kits) {
    $hits = Get-ChildItem $kits -Directory |
      Sort-Object Name -Descending |
      ForEach-Object { Join-Path $_.FullName 'x64\signtool.exe' } |
      Where-Object { Test-Path $_ }
    if ($hits) { return $hits[0] }
  }
  $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
  if ($cmd) { return $cmd.Source }
  throw '找不到 signtool.exe，请先安装 Windows SDK（或 Visual Studio 的「Windows 应用开发」工作负载）。'
}

$signtool = Find-SignTool

if ($Verify) {
  & $signtool verify /pa /v $File
  exit $LASTEXITCODE
}

if (-not $File) { throw '缺少 -File 参数' }

if ($Pfx) {
  if (-not (Test-Path $Pfx)) { throw "找不到证书文件：$Pfx" }
  & $signtool sign /f $Pfx /p $Password /fd sha256 /tr $TimestampUrl /td sha256 $File
}
elseif ($Thumbprint) {
  & $signtool sign /sha1 $Thumbprint /fd sha256 /tr $TimestampUrl /td sha256 $File
}
else {
  throw '需要 -Pfx 或 -Thumbprint 其中之一'
}

if ($LASTEXITCODE -ne 0) { throw "签名失败，signtool 退出码 $LASTEXITCODE" }

Write-Host "已签名：$File"
& $signtool verify /pa $File
