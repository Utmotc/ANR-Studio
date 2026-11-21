# v0.4 代码签名入口（Tauri bundle.windows.signCommand 调用）。
#
# Tauri 在打包时对「主程序 exe」和「NSIS 安装包」分别调用一次本脚本，
# 参数是待签文件路径。私钥/密码只从 CI secrets 注入，绝不进仓库：
#   ANR_WINDOWS_CERT_PFX_BASE64  PFX 证书的 base64（推荐）
#   ANR_WINDOWS_CERT_PASSWORD    PFX 密码
#   ANR_WINDOWS_CERT_THUMBPRINT  已装进证书存储时按指纹签（二选一）
#
# 没配证书时直接返回 0：本地产物保持未签名，CI 产物带签名，不会把本地构建卡死。
param(
  [Parameter(Mandatory = $true, Position = 0)][string]$File,
  [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest
)

$ErrorActionPreference = 'Stop'
if ($Rest) { $File = (@($File) + $Rest) -join ' ' }

if (-not (Test-Path -LiteralPath $File)) {
  Write-Error "sign-windows: 文件不存在：$File"
  exit 1
}

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
  throw '找不到 signtool.exe，请安装 Windows SDK。'
}

$signtool = Find-SignTool
$timestamp = 'http://timestamp.digicert.com'

if ($env:ANR_WINDOWS_CERT_PFX_BASE64) {
  if (-not $env:ANR_WINDOWS_CERT_PASSWORD) {
    Write-Error 'sign-windows: 配置了 PFX 但没有 ANR_WINDOWS_CERT_PASSWORD'
    exit 1
  }
  $pfx = Join-Path $env:RUNNER_TEMP ("anr-sign-" + [guid]::NewGuid().ToString('N') + '.pfx')
  [System.IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:ANR_WINDOWS_CERT_PFX_BASE64))
  try {
    & $signtool sign /fd sha256 /td sha256 /tr $timestamp /f $pfx /p $env:ANR_WINDOWS_CERT_PASSWORD $File
    if ($LASTEXITCODE -ne 0) { Write-Error "sign-windows: signtool 失败（$LASTEXITCODE）"; exit $LASTEXITCODE }
  } finally {
    Remove-Item -LiteralPath $pfx -Force -ErrorAction SilentlyContinue
  }
} elseif ($env:ANR_WINDOWS_CERT_THUMBPRINT) {
  & $signtool sign /fd sha256 /td sha256 /tr $timestamp /sha1 $env:ANR_WINDOWS_CERT_THUMBPRINT $File
  if ($LASTEXITCODE -ne 0) { Write-Error "sign-windows: signtool 失败（$LASTEXITCODE）"; exit $LASTEXITCODE }
} else {
  Write-Host "sign-windows: 未配置证书，跳过签名（$([System.IO.Path]::GetFileName($File))）"
  exit 0
}

& $signtool verify /pa $File
if ($LASTEXITCODE -ne 0) { Write-Error 'sign-windows: 签名校验失败'; exit $LASTEXITCODE }
Write-Host "sign-windows: 已签名 $File"