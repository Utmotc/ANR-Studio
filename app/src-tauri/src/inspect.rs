use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use crate::ini::IniDoc;
use crate::model::{AnrState, Game, NrFgConfig};
use crate::paths::now_iso;
use crate::scan::RawGame;

pub const PROVIDER: &str = "Apex Neural Rendering FG";

pub fn opti_dir(game: &Path) -> PathBuf {
    game.join("OptiScaler")
}
pub fn provider_dir(game: &Path) -> PathBuf {
    opti_dir(game).join("custom").join(PROVIDER)
}
pub fn record_path(game: &Path) -> PathBuf {
    opti_dir(game).join("nr_fg_install.json")
}
pub fn ini_path(game: &Path) -> PathBuf {
    game.join("OptiScaler.ini")
}
pub fn provider_dll(game: &Path) -> PathBuf {
    provider_dir(game).join("dlssg").join("nvngx_dlssg.dll")
}
pub fn nrf_g_ini(game: &Path) -> PathBuf {
    provider_dir(game).join("dlssg").join("nr_fg.ini")
}

pub fn sha256_file(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Some(hex::encode(hasher.finalize()))
}

/// provider DLL 有 30 MB 级，而 `rescan` 每改一次配置都会跑一遍。
/// 指纹只取决于文件内容，所以按 (路径, 大小, mtime) 缓存，Steam 校验/换版本后 mtime 变，自然失效。
pub fn sha256_file_cached(path: &Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let key = path.to_string_lossy().to_string();
    let stamp = (meta.len(), mtime);

    if mtime != 0 {
        if let Some(hit) = hash_cache()
            .lock()
            .ok()
            .and_then(|c| c.get(&key).cloned())
        {
            if hit.0 == stamp {
                return Some(hit.1);
            }
        }
    }

    let hash = sha256_file(path)?;
    if mtime != 0 {
        if let Ok(mut cache) = hash_cache().lock() {
            if cache.len() >= 512 {
                cache.clear();
            }
            cache.insert(key, (stamp, hash.clone()));
        }
    }
    Some(hash)
}

static HASH_CACHE: OnceLock<Mutex<HashMap<String, ((u64, u64), String)>>> = OnceLock::new();

fn hash_cache() -> &'static Mutex<HashMap<String, ((u64, u64), String)>> {
    HASH_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn fnv_id(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.to_ascii_lowercase().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:016x}", h)
}

fn read_record(game: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(record_path(game)).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn newest_log(game: &Path) -> Option<PathBuf> {
    let dir = provider_dir(game).join("dlssg");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if name.starts_with("nr_fg-") && name.ends_with(".log") {
                if let Ok(m) = e.metadata().and_then(|m| m.modified()) {
                    if best.as_ref().map(|(t, _)| m > *t).unwrap_or(true) {
                        best = Some((m, p));
                    }
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

fn parse_config(game: &Path) -> NrFgConfig {
    let mut cfg = NrFgConfig::default();
    let Ok(text) = fs::read_to_string(nrf_g_ini(game)) else {
        return cfg;
    };
    // nr_fg.ini 是扁平结构（没有 section），直接按顶层 key 读；只读一次文件
    for key in ["path", "model_size", "nits_per_unit", "max_generated", "scene_cut"] {
        if let Some(v) = flat_get_str(&text, key) {
            match key {
                "path" => cfg.path = v,
                "model_size" => cfg.model_size = v,
                "nits_per_unit" => cfg.nits = v.parse().unwrap_or(cfg.nits),
                "max_generated" => cfg.max_generated = v.parse().unwrap_or(cfg.max_generated),
                "scene_cut" => cfg.scene_cut = v != "0" && !v.eq_ignore_ascii_case("false"),
                _ => {}
            }
        }
    }
    cfg
}

fn flat_get_str(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with(';') || t.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = t.split_once('=') {
            if k.trim().eq_ignore_ascii_case(key) {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

pub fn inspect_anr(game: &Path, catalog: &HashMap<String, String>) -> Option<AnrState> {
    let provider = provider_dir(game);
    let record = read_record(game);
    if record.is_none() && !provider.exists() {
        return None;
    }

    let kind = match &record {
        Some(v) if v.get("proxy").is_some() => "full".to_string(),
        Some(_) => "addon".to_string(),
        None => "full".to_string(),
    };

    let proxy = record
        .as_ref()
        .and_then(|v| v.get("proxy"))
        .and_then(|x| x.as_str())
        .unwrap_or("dxgi.dll")
        .to_string();

    let installed_at = record
        .as_ref()
        .and_then(|v| v.get("installed_at"))
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            fs::metadata(&provider)
                .and_then(|m| m.modified())
                .map(|t| {
                    let dt: chrono::DateTime<chrono::Local> = t.into();
                    dt.to_rfc3339()
                })
                .unwrap_or_else(|_| now_iso())
        });

    let full_hash = sha256_file_cached(&provider_dll(game));
    let version = full_hash
        .as_ref()
        .and_then(|h| catalog.get(h).cloned())
        .unwrap_or_else(|| "未知".to_string());
    let fingerprint = full_hash
        .as_ref()
        .map(|h| h.chars().take(12).collect::<String>())
        .unwrap_or_default();

    let ini = IniDoc::load(&ini_path(game));
    let enabled_raw = ini.get("FrameGen", "Enabled").unwrap_or_else(|| "auto".into());
    let enabled = !enabled_raw.eq_ignore_ascii_case("false") && enabled_raw != "0";
    let replacement = ini.get("FrameGen", "FGNvngxReplacement").unwrap_or_default();
    let ini_exists = ini_path(game).exists();

    let log = newest_log(game)
        .map(|p| {
            p.strip_prefix(game)
                .map(|r| r.to_string_lossy().to_string())
                .unwrap_or_else(|_| p.to_string_lossy().to_string())
        })
        .unwrap_or_default();

    let (health, note) = if !provider_dll(game).exists() {
        ("error", Some("provider 文件缺失，需要重新安装".to_string()))
    } else if !ini_exists {
        ("error", Some("OptiScaler.ini 缺失，ANR 不会生效".to_string()))
    } else if !replacement.eq_ignore_ascii_case(PROVIDER) {
        (
            "warn",
            Some(format!(
                "OptiScaler.ini 的 FGNvngxReplacement 不是 {PROVIDER}，ANR 不会生效"
            )),
        )
    } else if record.is_none() {
        (
            "warn",
            Some("缺少安装记录，卸载时可能无法完整还原原文件".to_string()),
        )
    } else {
        ("ok", None)
    };

    Some(AnrState {
        kind,
        version,
        proxy,
        installed_at,
        enabled,
        fg_replacement: replacement.clone(),
        fingerprint,
        config: parse_config(game),
        health: health.to_string(),
        health_note: note,
        log_path: log,
    })
}

pub fn third_party_optiscaler(game: &Path) -> bool {
    if provider_dir(game).exists() {
        return false;
    }
    opti_dir(game).exists() || ini_path(game).exists()
}

/// 设计文档 5.1 的异常态：非空时表示「需人工确认后再操作」，
/// 前端弹确认框，后端 install/uninstall 也要求 force=true 兜底。
pub fn detect_conflict(dir: &Path) -> Option<String> {
    let opti = opti_dir(dir).exists();
    let ini = ini_path(dir).exists();
    let record = record_path(dir).exists();
    let provider = provider_dir(dir).exists();
    if opti && ini && !record && provider {
        Some(
            "OptiScaler 目录里有 ANR 的 provider，但没有本工具的安装记录（可能是手动安装或其他工具装的）。继续安装/卸载会覆盖这些文件，请先确认。"
                .into(),
        )
    } else if opti && !ini && record && provider {
        Some("OptiScaler.ini 丢失，但安装记录和 provider 还在，状态不完整。建议先修复配置再操作。".into())
    } else if !opti && ini && !provider {
        Some("目录里只剩 OptiScaler.ini，没有 OptiScaler 目录，无法判断来源。".into())
    } else if opti && !ini && !record && !provider {
        Some("OptiScaler 目录存在，但既没有 ini 也没有安装记录，无法判断来源。".into())
    } else {
        None
    }
}

pub fn build_game(raw: RawGame, catalog: &HashMap<String, String>, procs: &HashSet<String>) -> Game {
    let anr = inspect_anr(&raw.dir, catalog);
    let running = !raw.exe.is_empty() && procs.contains(&raw.exe.to_ascii_lowercase());
    let third = third_party_optiscaler(&raw.dir);
    Game {
        id: fnv_id(&raw.dir.to_string_lossy()),
        anr,
        name: raw.name,
        source: raw.source,
        dir: raw.dir.to_string_lossy().to_string(),
        exe: raw.exe,
        last_used: raw.last_used,
        size_gb: 0.0,
        running,
        third_party_optiscaler: third,
        cover: None,
        conflict: detect_conflict(&raw.dir),
    }
}
