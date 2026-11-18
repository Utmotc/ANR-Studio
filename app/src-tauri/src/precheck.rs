//! 安装前置检查。设计文档 5.2 要求「全部通过才继续」，
//! 这里把能自动判断的都判断掉，剩下的用 warn 交给用户确认。

use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::scan;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub code: String,
    /// block = 直接不让装，warn = 提示风险，用户可继续
    pub level: String,
    pub title: String,
    pub detail: String,
}

fn issue(code: &str, level: &str, title: &str, detail: String) -> Issue {
    Issue {
        code: code.to_string(),
        level: level.to_string(),
        title: title.to_string(),
        detail,
    }
}

/// 常见反作弊组件（命中后提醒，不阻断——是否介意由用户决定）
const AC_MARKERS: &[(&str, &str)] = &[
    ("easyanticheat", "Easy Anti-Cheat"),
    ("battleye", "BattlEye"),
    ("ace-guard", "ACE-Guard（腾讯）"),
    ("ace-game", "ACE（腾讯）"),
    ("sguard", "SGuard"),
    ("anticheat", "反作弊组件"),
    ("nprotect", "nProtect GameGuard"),
    ("xigncode", "XignCode3"),
    ("tp3_helper", "TenProtect"),
    ("gameguard", "GameGuard"),
];

/// 加壳 / 虚拟机保护的特征文件，这类游戏注入第三方 DLL 大概率失败
const SHELL_MARKERS: &[&str] = &[
    "artifact.dll",
    "artifact.json",
    "coldloader.dll",
    "vmp.dll",
    "vmprotect",
    "themida",
    "winlicense",
];

const CRT: &[&str] = &[
    "MSVCP140.dll",
    "MSVCP140_ATOMIC_WAIT.dll",
    "VCRUNTIME140.dll",
    "VCRUNTIME140_1.dll",
];

/// 安装包约 30MB，解压 + 写入留点余量
const NEED_BYTES: u64 = 300 * 1024 * 1024;

#[cfg(windows)]
fn free_bytes(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            dir: *const u16,
            free: *mut u64,
            total: *mut u64,
            total_free: *mut u64,
        ) -> i32;
    }

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let (mut free, mut total, mut total_free) = (0u64, 0u64, 0u64);
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, &mut total_free) };
    if ok != 0 {
        Some(free)
    } else {
        None
    }
}

#[cfg(not(windows))]
fn free_bytes(_path: &Path) -> Option<u64> {
    None
}

fn find_shell_marker(dir: &Path) -> Option<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return None;
    };
    for e in rd.filter_map(|x| x.ok()) {
        let name = e.file_name().to_string_lossy().to_lowercase();
        if SHELL_MARKERS.iter().any(|m| name == *m || name.contains(m)) {
            return Some(e.file_name().to_string_lossy().to_string());
        }
    }
    None
}

fn find_anticheat(dir: &Path) -> Option<&'static str> {
    let mut stack: Vec<(PathBuf, usize)> = vec![(dir.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        if depth > 3 {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.filter_map(|x| x.ok()) {
            let name = e.file_name().to_string_lossy().to_lowercase();
            for (m, label) in AC_MARKERS {
                if name.contains(m) {
                    return Some(label);
                }
            }
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                stack.push((e.path(), depth + 1));
            }
        }
    }
    None
}

fn crt_missing() -> Vec<String> {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    CRT.iter()
        .filter(|f| !Path::new(&root).join("System32").join(f).exists())
        .map(|f| f.to_string())
        .collect()
}

/// R9：放在 Steam 库目录里的游戏，「验证文件完整性」可能把装进去的文件覆盖掉
pub fn steamapps_warning(dir: &Path) -> Option<String> {
    let normalized = dir
        .to_string_lossy()
        .to_ascii_lowercase()
        .replace('/', "\\");
    if !normalized.contains("\\steamapps\\") && !normalized.ends_with("\\steamapps") {
        return None;
    }
    let leaf = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if leaf == "common" || leaf == "steamapps" {
        return Some(
            "这是 Steam 库的公共目录，不是某个具体游戏的主程序目录。Steam 校验可能覆盖安装的文件，请选择 steamapps\\common\\<游戏名> 里的目录。"
                .into(),
        );
    }
    Some(
        "这个目录位于 Steam 库（steamapps）内，Steam 的「验证文件完整性」可能覆盖 ANR 安装的文件；验证后需要重新安装。"
            .into(),
    )
}

/// `warn_anticheat` 对应设置里的「反作弊风险提示」开关
pub fn run(dir: &Path, warn_anticheat: bool) -> Vec<Issue> {
    let mut blocks: Vec<Issue> = Vec::new();
    let mut warns: Vec<Issue> = Vec::new();

    if !dir.is_dir() {
        blocks.push(issue(
            "no_dir",
            "block",
            "游戏目录不存在",
            "这个路径已经不在磁盘上了，先用「更改游戏目录」指向正确的位置。".into(),
        ));
        return blocks;
    }

    let Some(exe) = scan::find_exe(dir, None) else {
        blocks.push(issue(
            "no_exe",
            "block",
            "目录里没有可执行文件",
            "请选择游戏主程序（.exe）所在的文件夹，而不是上一级目录。".into(),
        ));
        return blocks;
    };

    // 游戏在跑
    let exe_name = exe
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if scan::running_names().contains(&exe_name) {
        blocks.push(issue(
            "running",
            "block",
            "游戏正在运行",
            format!("{exe_name} 还没退出。安装要替换目录里的文件，请先关掉游戏。"),
        ));
    }

    // 目录可写
    let probe = dir.join(".anr-write-test");
    match std::fs::write(&probe, b"ok") {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
        }
        Err(e) => blocks.push(issue(
            "readonly",
            "block",
            "目录不可写",
            format!("写入测试失败：{e}。可能是只读、被占位，或需要管理员权限。"),
        )),
    }

    // 磁盘空间
    if let Some(free) = free_bytes(dir) {
        if free < NEED_BYTES {
            blocks.push(issue(
                "disk",
                "block",
                "磁盘空间不足",
                format!(
                    "这个分区只剩 {} MB，安装加解压大约需要 300 MB。",
                    free / 1024 / 1024
                ),
            ));
        }
    }

    // VC++ 运行库（官方脚本也会提示，但那时游戏已经装不上了）
    let missing = crt_missing();
    if !missing.is_empty() {
        warns.push(issue(
            "crt",
            "warn",
            "缺少 VC++ 运行库",
            format!(
                "OptiScaler 依赖 Microsoft Visual C++ 2015-2022 (x64)，缺少：{}。\
                 不装的话游戏能启动，但 ANR 不会生效。",
                missing.join("、")
            ),
        ));
    }

    // 加壳 / 虚拟机保护
    if let Some(marker) = find_shell_marker(dir) {
        let size = std::fs::metadata(&exe).map(|m| m.len()).unwrap_or(0);
        warns.push(issue(
            "packed",
            "warn",
            "这个游戏疑似加壳保护",
            format!(
                "目录里有 {marker}，主程序 {} MB。这类保护通常会在启动时校验模块，\
                 装进去的代理 DLL 可能被拦——表现为游戏直接打不开。装完如果启动失败，\
                 用「卸载」还原即可。",
                size / 1024 / 1024
            ),
        ));
    }

    // 反作弊
    if warn_anticheat {
        if let Some(label) = find_anticheat(dir) {
            warns.push(issue(
                "anticheat",
                "warn",
                "检测到反作弊组件",
                format!(
                    "目录里有 {label}。注入类工具在带反作弊的联机游戏里有封号风险，\
                     建议只在单机模式下使用。"
                ),
            ));
        }
    }

    // R9：只有把整个 steamapps / common 公共目录当成游戏加进来才需要确认。
    // 具体游戏目录本来就在 steamapps 下，每次安装都弹确认太吵（添加时已 toast 提示过一次）。
    let leaf = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if leaf == "common" || leaf == "steamapps" {
        if let Some(msg) = steamapps_warning(dir) {
            warns.push(issue("steamapps", "warn", "这不是具体的游戏目录", msg));
        }
    }

    blocks.extend(warns);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steamapps_warning_only_matches_steam_paths() {
        assert!(steamapps_warning(Path::new(r"C:\Steam\steamapps\common\Cyberpunk 2077")).is_some());
        assert!(steamapps_warning(Path::new(r"C:\Steam\steamapps\common")).is_some());
        assert!(steamapps_warning(Path::new(r"D:\Games\SomeGame")).is_none());
    }
}
