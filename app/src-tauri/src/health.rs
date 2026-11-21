//! 安装体检与一键修复。
//!
//! 这里只做「能本地安全修好」的事：补写 ini 关键项、从备份还原代理 DLL/ini。
//! 需要重新下载安装包的情况（provider 目录整块没了）不假装能修，返回 `needs_reinstall`，
//! 由界面引导用户重装当前版本。

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::ini::IniDoc;
use crate::inspect;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub code: String,
    /// error = ANR 现在肯定不生效；warn = 能跑但状态不完整
    pub level: String,
    pub title: String,
    pub detail: String,
    /// 本地就能修好（会出现在「一键修复」里）
    pub fixable: bool,
}

impl Issue {
    fn new(
        code: &str,
        level: &str,
        title: impl Into<String>,
        detail: String,
        fixable: bool,
    ) -> Self {
        Self {
            code: code.to_string(),
            level: level.to_string(),
            title: title.into(),
            detail,
            fixable,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    pub fixed: Vec<String>,
    pub remaining: Vec<Issue>,
    pub needs_reinstall: bool,
}

fn record(dir: &Path) -> serde_json::Value {
    fs::read_to_string(inspect::record_path(dir))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .unwrap_or_default()
}

fn ini_bak(dir: &Path) -> std::path::PathBuf {
    inspect::ini_path(dir).with_extension("ini.anr-bak")
}

fn backup_dll(dir: &Path, proxy: &str) -> Option<std::path::PathBuf> {
    let p = inspect::opti_dir(dir).join("nr_fg_backup").join(proxy);
    if p.is_file() {
        Some(p)
    } else {
        None
    }
}

/// 记录文件里原始文件的哈希，用于拒绝还原一个被改过的备份
fn original_hash(dir: &Path, proxy: &str) -> Option<String> {
    record(dir)
        .get("originals")
        .and_then(|o| o.get(proxy))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub fn check(dir: &Path) -> Vec<Issue> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        out.push(Issue::new(
            "no_dir",
            "error",
            "游戏目录不存在",
            "这个路径已经不在磁盘上了，先用「更改游戏目录」指到正确位置。".into(),
            false,
        ));
        return out;
    }

    let provider = inspect::provider_dir(dir);
    let dll = inspect::provider_dll(dir);
    let ini = inspect::ini_path(dir);
    let has_record = inspect::record_path(dir).exists();
    let rec = record(dir);
    let is_addon = rec.get("proxy").is_none() && has_record;

    if !provider.exists() || !dll.exists() {
        out.push(Issue::new(
            "provider_missing",
            "error",
            "provider 文件缺失",
            "OptiScaler/custom/Apex Neural Rendering FG 里的 nvngx_dlssg.dll 不在了，ANR 不会生效。这个只能重新安装当前版本恢复。".into(),
            false,
        ));
    }
    if !ini.exists() {
        let fixable = ini_bak(dir).is_file();
        out.push(Issue::new(
            "ini_missing",
            "error",
            "OptiScaler.ini 缺失",
            if fixable {
                "配置丢失，但同目录还留着本工具写入前的备份，可以直接从备份恢复。".into()
            } else {
                "OptiScaler 启动时要读这个文件；它不在了，ANR 不会生效。需要重新安装。".into()
            },
            fixable,
        ));
    } else {
        let doc = IniDoc::load(&ini);
        if doc.is_lossy() {
            out.push(Issue::new(
                "ini_encoding",
                "warn",
                "OptiScaler.ini 不是 UTF-8 编码",
                "为避免写坏里面的中文注释，本工具会拒绝修改这个文件。用支持 UTF-8 的编辑器另存一次即可。".into(),
                false,
            ));
        }
        let rep = doc.get("FrameGen", "FGNvngxReplacement").unwrap_or_default();
        if !rep.eq_ignore_ascii_case(inspect::PROVIDER) {
            out.push(Issue::new(
                "replacement",
                "warn",
                "帧生成实现没指向 ANR",
                format!(
                    "[FrameGen] FGNvngxReplacement 现在是「{}」，ANR 不会被调用。可以一键写回 Apex Neural Rendering FG。",
                    if rep.is_empty() { "（空）" } else { rep.as_str() }
                ),
                true,
            ));
        }
    }

    // 代理 DLL：全量安装才有记录，插件安装不动代理
    if has_record && !is_addon {
        let proxy = rec
            .get("proxy")
            .and_then(|v| v.as_str())
            .unwrap_or("dxgi.dll")
            .to_string();
        if !dir.join(&proxy).is_file() {
            let fixable = backup_dll(dir, &proxy).is_some();
            out.push(Issue::new(
                "proxy_missing",
                "error",
                format!("代理 DLL（{proxy}）不在了"),
                if fixable {
                    "安装时的备份还在，可以直接还原。".into()
                } else {
                    "没有找到安装时的备份，需要重新安装。".into()
                },
                fixable,
            ));
        }
    }

    if has_record && !inspect::nrf_g_ini(dir).exists() && provider.exists() {
        out.push(Issue::new(
            "nrf_missing",
            "warn",
            "缺少 nr_fg.ini",
            "provider 在，但参数文件不在；ANR 会用内置默认值，调参页会缺项。重新安装可以恢复。".into(),
            false,
        ));
    }
    if !has_record && provider.exists() {
        out.push(Issue::new(
            "no_record",
            "warn",
            "缺少安装记录",
            "找不到 nr_fg_install.json，卸载时无法按哈希校验还原原文件。".into(),
            false,
        ));
    }
    out
}

pub fn repair(dir: &Path) -> RepairReport {
    let mut fixed: Vec<String> = Vec::new();
    if !dir.is_dir() {
        return RepairReport {
            fixed,
            remaining: check(dir),
            needs_reinstall: true,
        };
    }

    // 1) ini 丢失：有备份就从备份还原
    let ini = inspect::ini_path(dir);
    let bak = ini_bak(dir);
    if !ini.exists() && bak.is_file() {
        if fs::copy(&bak, &ini).is_ok() {
            fixed.push("OptiScaler.ini（从安装前备份恢复）".into());
        }
    }

    // 2) ini 关键项
    if ini.exists() {
        let mut doc = IniDoc::load(&ini);
        if !doc.is_lossy() {
            let mut touched = false;
            let rep = doc.get("FrameGen", "FGNvngxReplacement").unwrap_or_default();
            if !rep.eq_ignore_ascii_case(inspect::PROVIDER) {
                doc.set("FrameGen", "FGNvngxReplacement", inspect::PROVIDER);
                fixed.push("FGNvngxReplacement = Apex Neural Rendering FG".into());
                touched = true;
            }
            if doc.get("FrameGen", "Enabled").is_none() {
                // 缺省就是 auto（= 开），显式写出来更清楚
                doc.set("FrameGen", "Enabled", "auto");
                fixed.push("FrameGen Enabled = auto".into());
                touched = true;
            }
            if touched {
                IniDoc::backup_once(&ini);
                if let Err(e) = doc.save(&ini) {
                    fixed.push(format!("写入 OptiScaler.ini 失败：{e}"));
                }
            }
        }
    }

    // 3) 代理 DLL：按记录里的哈希校验备份，再还原
    let rec = record(dir);
    if rec.get("proxy").is_some() {
        let proxy = rec
            .get("proxy")
            .and_then(|v| v.as_str())
            .unwrap_or("dxgi.dll")
            .to_string();
        let target = dir.join(&proxy);
        if !target.is_file() {
            if let Some(backup) = backup_dll(dir, &proxy) {
                let ok = match original_hash(dir, &proxy) {
                    Some(want) => inspect::sha256_file(&backup)
                        .map(|got| got.eq_ignore_ascii_case(&want))
                        .unwrap_or(false),
                    None => true,
                };
                if ok && fs::copy(&backup, &target).is_ok() {
                    fixed.push(format!("{proxy}（从安装前备份还原）"));
                }
            }
        }
    }

    let remaining = check(dir);
    // 只有「本地修不了的 error」才需要重装；编码警告、缺安装记录之类不影响运行
    let needs_reinstall = remaining
        .iter()
        .any(|i| !i.fixable && i.level == "error");
    RepairReport {
        fixed,
        remaining,
        needs_reinstall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn missing_dir() -> PathBuf {
        PathBuf::from("Z:\\__anr_studio_health_test__\\definitely\\missing")
    }

    #[test]
    fn missing_dir_is_reported_and_not_fixable() {
        let issues = check(&missing_dir());
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "no_dir");
        assert!(!issues[0].fixable);
    }

    #[test]
    fn empty_directory_reports_provider_and_ini() {
        // CARGO_MANIFEST_DIR/src 一定存在，且里面不会有 OptiScaler
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let issues = check(&dir);
        let codes: Vec<&str> = issues.iter().map(|i| i.code.as_str()).collect();
        assert!(codes.contains(&"provider_missing"), "codes = {codes:?}");
        assert!(codes.contains(&"ini_missing"), "codes = {codes:?}");
        assert!(issues.iter().all(|i| i.level == "error" || i.level == "warn"));
    }

    #[test]
    fn repair_on_empty_directory_claims_needs_reinstall() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let report = repair(&dir);
        assert!(report.fixed.is_empty());
        assert!(report.needs_reinstall);
    }
}