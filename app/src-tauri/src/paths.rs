use crate::model::Settings;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// 已知的 provider 指纹 → 版本（用 GitHub Release 的原始 tag，保证与版本列表一致）
const KNOWN: &[(&str, &str)] = &[(
    "ae7424b87d13a0e3a9a99dd02d1ea35b9de96c2caa9cd4300d73bac5bb0cf4e8",
    "FG-V.B15",
)];

pub fn settings_path(data: &Path) -> PathBuf {
    data.join("settings.json")
}

pub fn catalog_path(data: &Path) -> PathBuf {
    data.join("catalog.json")
}

pub fn snapshots_dir(data: &Path) -> PathBuf {
    data.join("snapshots")
}

pub fn default_cache_dir(data: &Path) -> PathBuf {
    data.join("cache")
}

pub fn load_settings(data: &Path) -> Settings {
    let cache = default_cache_dir(data).to_string_lossy().to_string();
    let p = settings_path(data);
    let mut s = match fs::read_to_string(&p) {
        Ok(text) => serde_json::from_str::<Settings>(&text).unwrap_or_else(|_| Settings::with_defaults(cache.clone())),
        Err(_) => Settings::with_defaults(cache.clone()),
    };
    if s.cache_dir.trim().is_empty() {
        s.cache_dir = cache;
    }
    s
}

pub fn save_settings(data: &Path, s: &Settings) -> Result<(), String> {
    let p = settings_path(data);
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(&p, text).map_err(|e| format!("写入设置失败：{e}"))
}

pub fn load_catalog(data: &Path) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for (sha, tag) in KNOWN {
        map.insert((*sha).to_string(), (*tag).to_string());
    }
    if let Ok(text) = fs::read_to_string(catalog_path(data)) {
        if let Ok(saved) = serde_json::from_str::<HashMap<String, String>>(&text) {
            map.extend(saved);
        }
    }
    map
}

pub fn save_catalog(data: &Path, map: &HashMap<String, String>) -> Result<(), String> {
    let text = serde_json::to_string_pretty(map).map_err(|e| e.to_string())?;
    fs::write(catalog_path(data), text).map_err(|e| format!("写入指纹表失败：{e}"))
}

pub fn ensure_dir(p: &Path) -> Result<(), String> {
    fs::create_dir_all(p).map_err(|e| format!("创建目录失败 {}：{e}", p.display()))
}

pub fn now_iso() -> String {
    chrono::Local::now().to_rfc3339()
}

pub fn file_mtime_iso(p: &Path) -> String {
    fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            dt.to_rfc3339()
        })
        .unwrap_or_else(now_iso)
}
