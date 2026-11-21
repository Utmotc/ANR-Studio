//! P1-7 系统信息采集与脱敏。
//!
//! 不为了显卡型号引一个 WMI crate：直接调 PowerShell 的 `Get-CimInstance`，
//! 失败就退回 `std::env` 能拿到的少量信息——诊断包宁可少写几行，也不能整体生成失败。

use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct GpuInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub driver_version: String,
    #[serde(default)]
    pub driver_date: String,
    #[serde(default)]
    pub adapter_ram: Option<u64>,
    #[serde(default)]
    pub refresh_rate: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct CpuInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub cores: Option<u32>,
    #[serde(default)]
    pub logical: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct OsInfo {
    #[serde(default)]
    pub caption: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub build: String,
    #[serde(default)]
    pub arch: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExeInfo {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub product: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    #[serde(default)]
    pub gpus: Vec<GpuInfo>,
    #[serde(default)]
    pub cpu: CpuInfo,
    #[serde(default)]
    pub os: OsInfo,
    #[serde(default)]
    pub memory: Option<u64>,
    #[serde(default)]
    pub exe: Option<ExeInfo>,
    /// 采集失败时的原因（降级展示用，不影响诊断包生成）
    #[serde(default)]
    pub error: Option<String>,
}

const SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
try { [Console]::OutputEncoding = [Text.Encoding]::UTF8 } catch {}
$gpus = @()
foreach ($v in @(Get-CimInstance Win32_VideoController)) {
  $dd = ''
  if ($v.DriverDate) { $dd = $v.DriverDate.ToString('yyyy-MM-dd') }
  $gpus += [pscustomobject]@{
    name = [string]$v.Name
    driverVersion = [string]$v.DriverVersion
    driverDate = [string]$dd
    adapterRam = $v.AdapterRAM
    refreshRate = $v.CurrentRefreshRate
  }
}
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$cs = Get-CimInstance Win32_ComputerSystem
$os = Get-CimInstance Win32_OperatingSystem
$exe = $null
if ($env:ANR_EXE -and (Test-Path -LiteralPath $env:ANR_EXE)) {
  $fi = Get-Item -LiteralPath $env:ANR_EXE
  $exe = [pscustomobject]@{
    path = [string]$env:ANR_EXE
    size = $fi.Length
    version = [string]$fi.VersionInfo.FileVersion
    product = [string]$fi.VersionInfo.ProductName
  }
}
[pscustomobject]@{
  gpus = $gpus
  cpu = @{ name = [string]$cpu.Name; cores = $cpu.NumberOfCores; logical = $cpu.NumberOfLogicalProcessors }
  os = @{ caption = [string]$os.Caption; version = [string]$os.Version; build = [string]$os.BuildNumber; arch = [string]$os.OSArchitecture }
  memory = $cs.TotalPhysicalMemory
  exe = $exe
} | ConvertTo-Json -Depth 6 -Compress
"#;

/// 采集一次系统信息。在 blocking 线程里调用（PowerShell 冷启动约 1–2 秒）。
pub fn collect(exe: Option<&Path>) -> SystemInfo {
    let mut cmd = Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", SCRIPT]);
    if let Some(e) = exe {
        cmd.env("ANR_EXE", e);
    }
    crate::scan::hidden(&mut cmd);
    let out = cmd.output();
    match out {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            parse_json(text.trim())
        }
        Ok(o) => {
            let mut info = fallback();
            info.error = Some(format!(
                "PowerShell 退出码 {}：{}",
                o.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&o.stderr).trim()
            ));
            info
        }
        Err(e) => {
            let mut info = fallback();
            info.error = Some(format!("无法启动 PowerShell：{e}"));
            info
        }
    }
}

fn parse_json(text: &str) -> SystemInfo {
    // PowerShell 输出前面可能混进控制字符；取第一个 '{' 到最后
    let start = text.find('{');
    let end = text.rfind('}');
    let (Some(start), Some(end)) = (start, end) else {
        let mut info = fallback();
        info.error = Some("没有解析到系统信息".into());
        return info;
    };
    match serde_json::from_str::<SystemInfo>(&text[start..=end]) {
        Ok(mut info) => {
            if info.os.caption.is_empty() {
                info.os = fallback().os;
            }
            if info.cpu.name.is_empty() && info.gpus.is_empty() {
                info.error = Some("WMI 没有返回显卡与 CPU".into());
            }
            info
        }
        Err(e) => {
            let mut info = fallback();
            info.error = Some(format!("解析系统信息失败：{e}"));
            info
        }
    }
}

fn fallback() -> SystemInfo {
    SystemInfo {
        os: OsInfo {
            caption: std::env::consts::OS.to_string(),
            version: String::new(),
            build: String::new(),
            arch: std::env::consts::ARCH.to_string(),
        },
        ..Default::default()
    }
}

fn gb(bytes: u64) -> f64 {
    bytes as f64 / 1024.0 / 1024.0 / 1024.0
}

/// 给 `system.txt` 用的可读文本；没有采集到的项写「未采集到」，不伪造。
pub fn render_text(info: &SystemInfo) -> String {
    let mut s = String::new();
    s.push_str("== 系统 ==\n");
    let os = &info.os;
    if !os.caption.is_empty() {
        s.push_str(&format!(
            "OS: {} {} build {}\n",
            os.caption.trim(),
            os.version.trim(),
            os.build.trim()
        ));
    } else {
        s.push_str(&format!("OS: {} {}\n", std::env::consts::OS, std::env::consts::ARCH));
    }
    s.push_str(&format!("架构: {}\n", os.arch));
    if info.cpu.name.is_empty() {
        s.push_str("CPU: 未采集到\n");
    } else {
        let cores = info
            .cpu
            .cores
            .map(|c| format!("{c} 核"))
            .unwrap_or_else(|| "核数未知".into());
        let logical = info
            .cpu
            .logical
            .map(|c| format!(" / {c} 线程"))
            .unwrap_or_default();
        s.push_str(&format!("CPU: {cores}{logical} · {}\n", info.cpu.name.trim()));
    }
    match info.memory {
        Some(m) => s.push_str(&format!("内存: {:.1} GB\n", gb(m))),
        None => s.push_str("内存: 未采集到\n"),
    }

    s.push_str("\n== 显卡 ==\n");
    if info.gpus.is_empty() {
        s.push_str("未采集到显卡信息\n");
    } else {
        for (i, g) in info.gpus.iter().enumerate() {
            let mut line = format!("{}. {}", i + 1, g.name.trim());
            if !g.driver_version.trim().is_empty() {
                line.push_str(&format!(" · 驱动 {}", g.driver_version.trim()));
            }
            if !g.driver_date.trim().is_empty() {
                line.push_str(&format!(" ({})", g.driver_date.trim()));
            }
            if let Some(ram) = g.adapter_ram {
                if ram > 0 {
                    line.push_str(&format!(" · 显存 {:.1} GB", gb(ram)));
                }
            }
            if let Some(hz) = g.refresh_rate {
                if hz > 0 {
                    line.push_str(&format!(" · {hz} Hz"));
                }
            }
            s.push_str(&line);
            s.push('\n');
        }
    }

    if let Some(e) = &info.exe {
        s.push_str("\n== 游戏主程序 ==\n");
        s.push_str(&format!("路径: {}\n", e.path));
        if let Some(size) = e.size {
            s.push_str(&format!("大小: {:.1} MB\n", size as f64 / 1_048_576.0));
        }
        if !e.product.trim().is_empty() {
            s.push_str(&format!("产品: {}\n", e.product.trim()));
        }
        s.push_str(&format!(
            "版本: {}\n",
            if e.version.trim().is_empty() { "未采集到" } else { e.version.trim() }
        ));
    }

    if let Some(err) = &info.error {
        s.push_str(&format!("\n采集降级：{err}\n"));
    }
    s
}

fn is_sep(b: u8) -> bool {
    matches!(b, b'\\' | b'/' | b'"' | b'\'' | b' ' | b'\t' | b'\r' | b'\n')
}

fn find_ci(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    let last = hay.len() - needle.len();
    let mut i = from;
    while i <= last {
        if hay[i..i + needle.len()].eq_ignore_ascii_case(needle) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn utf8_len_at(bytes: &[u8], i: usize) -> usize {
    let b = bytes[i];
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        1
    }
}

fn replace_ci(text: &str, needle: &str, with: &str) -> String {
    if needle.is_empty() {
        return text.to_string();
    }
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut last = 0;
    while let Some(pos) = find_ci(bytes, needle.as_bytes(), i) {
        out.push_str(&text[last..pos]);
        out.push_str(with);
        last = pos + needle.len();
        i = last;
    }
    out.push_str(&text[last..]);
    out
}

/// 脱敏：
/// - `C:\Users\<name>` / `C:/Users/<name>` → `%USERPROFILE%`
/// - 当前 Windows 用户名（≥3 字符）出现的任何位置 → `%USERNAME%`
/// - 17 位 SteamID64 → 保留首尾，中间打码
///
/// 只用于诊断包里的文本文件；二进制原样打包。
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // \Users\ 或 /Users/（大小写不敏感）。一次找到下一个候选，不逐字符重扫
        let Some(rel) = find_ci(bytes, b"users", i) else {
            out.push_str(&text[i..]);
            break;
        };
        // 「users」前必须是分隔符，后面必须跟分隔符
        let before_ok = rel == 0 || is_sep(bytes[rel - 1]);
        let after = rel + 5;
        if before_ok && after < bytes.len() && is_sep(bytes[after]) {
            let seg_start = after + 1;
            let mut seg_end = seg_start;
            while seg_end < bytes.len() && !is_sep(bytes[seg_end]) {
                seg_end += utf8_len_at(bytes, seg_end);
            }
            if seg_end > seg_start {
                // 尽量连前面的盘符一起吞掉：C:\Users\x → %USERPROFILE%
                let mut cut = rel;
                let mut full = false;
                if rel >= 3
                    && bytes[rel - 2] == b':'
                    && is_sep(bytes[rel - 1])
                    && bytes[rel - 3].is_ascii_alphabetic()
                {
                    cut = rel - 3;
                    full = true;
                }
                out.push_str(&text[i..cut]);
                if full {
                    out.push_str("%USERPROFILE%");
                } else {
                    out.push_str("Users");
                    out.push(std::path::MAIN_SEPARATOR);
                    out.push_str("%USERNAME%");
                }
                // seg 已整体替换；分隔符留给下一轮复制
                i = seg_end;
                continue;
            }
        }
        // 不是用户目录（例如 "Users" 出现在说明文本里）：原样复制到词尾，跳过它
        let skip = after.min(bytes.len());
        out.push_str(&text[i..skip]);
        i = skip;
    }

    // 当前用户名（路径之外也可能出现）
    if let Ok(user) = std::env::var("USERNAME") {
        let user = user.trim().to_string();
        if user.chars().count() >= 3 && user.chars().all(|c| !c.is_whitespace()) {
            out = replace_ci(&out, &user, "%USERNAME%");
        }
    }

    mask_steam_ids(&out)
}

fn mask_steam_ids(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            let run = &text[i..j];
            // SteamID64 是 17 位；也兼容 16 位的历史账号
            if run.len() == 17 || run.len() == 16 {
                out.push_str(&run[..4]);
                out.push_str(&"*".repeat(run.len() - 6));
                out.push_str(&run[run.len() - 2..]);
            } else {
                out.push_str(run);
            }
            i = j;
        } else {
            let len = utf8_len_at(bytes, i);
            out.push_str(&text[i..i + len]);
            i += len;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_windows_user_paths() {
        let s = sanitize(r"C:\Users\张伟\AppData\Local\anr-studio\settings.json");
        assert!(!s.contains("张伟"), "{s}");
        assert!(s.contains("%USERPROFILE%"), "{s}");
        assert!(s.ends_with(r"AppData\Local\anr-studio\settings.json"), "{s}");
    }

    #[test]
    fn masks_forward_slash_paths() {
        let s = sanitize("file:///D:/Users/Alice/game/nr_fg.log");
        assert!(!s.contains("Alice"), "{s}");
        assert!(s.contains("%USERPROFILE%"), "{s}");
    }

    #[test]
    fn masks_steam_id_but_keeps_other_numbers() {
        let s = sanitize("steamid=76561198012345678 fps=120 size=12345");
        assert!(!s.contains("76561198012345678"), "{s}");
        assert!(s.contains("7656***********78"), "{s}");
        assert!(s.contains("fps=120"), "{s}");
        assert!(s.contains("size=12345"), "{s}");
    }

    #[test]
    fn leaves_unrelated_paths_alone() {
        let s = sanitize(r"C:\Games\Steam\steamapps\common\Game\nr_fg.ini");
        assert_eq!(s, r"C:\Games\Steam\steamapps\common\Game\nr_fg.ini");
    }

    #[test]
    fn renders_missing_gpu_gracefully() {
        let info = SystemInfo::default();
        let text = render_text(&info);
        assert!(text.contains("未采集到显卡信息"));
        assert!(text.contains("CPU: 未采集到"));
    }
}