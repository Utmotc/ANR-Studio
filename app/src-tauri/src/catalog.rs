//! 指纹表（provider_sha256 → 版本 tag）的维护。
//!
//! 设计文档 4 / Q2 / R7：版本识别完全靠这张表。表里没有的记录有三个来源：
//! 1. 工具内置（paths.rs 的 KNOWN）
//! 2. 下载/导入安装包时，从包内 `nr_fg_package.json` 抽取（lib.rs::record_fingerprint）
//! 3. 用户手动标记某个游戏装的是哪一版（mark_version）
//!
//! 这里负责「刷新」：重扫本地所有已缓存的包补齐遗漏，再尝试拉一份远程表。

use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 远程指纹表（不存在时静默忽略，不算刷新失败）
const REMOTE: &str = "https://raw.githubusercontent.com/apex-compute/ANR/main/catalog.json";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshResult {
    /// 本次新增的指纹条数
    pub added: usize,
    /// 表内总条数
    pub total: usize,
    /// 是否拉到了远程表
    pub remote: bool,
}

pub fn from_package_json(pkg: &Path) -> Option<(String, String)> {
    let text = fs::read_to_string(pkg).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let sha = v.get("provider_sha256")?.as_str()?.to_string();
    if sha.len() != 64 {
        return None;
    }
    Some((sha, v.get("tag").and_then(|x| x.as_str()).unwrap_or("").to_string()))
}

/// 扫描 cache/<tag>/<kind>/nr_fg_package.json（含用户手动导入的包）
pub fn from_cache(cache: &Path) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(tags) = fs::read_dir(cache) else {
        return out;
    };
    for tag in tags.flatten() {
        let tag_dir = tag.path();
        if !tag_dir.is_dir() {
            continue;
        }
        let tag_name = tag.file_name().to_string_lossy().to_string();
        let Ok(kinds) = fs::read_dir(&tag_dir) else {
            continue;
        };
        for k in kinds.flatten() {
            let kind_dir = k.path();
            let meta = kind_dir.join("nr_fg_package.json");
            if !kind_dir.is_dir() || !meta.exists() {
                continue;
            }
            if let Some((sha, _)) = from_package_json(&meta) {
                out.insert(sha, tag_name.clone());
            }
            // 有些包把 json 放在一层子目录里
            if let Ok(inner) = fs::read_dir(&kind_dir) {
                for e in inner.flatten() {
                    let p = e.path().join("nr_fg_package.json");
                    if p.exists() {
                        if let Some((sha, _)) = from_package_json(&p) {
                            out.insert(sha, tag_name.clone());
                        }
                    }
                }
            }
        }
    }
    out
}

/// 尝试拉远程指纹表；仓库没有提供时返回 None，不阻断本地刷新
pub fn from_remote() -> Option<HashMap<String, String>> {
    let client = crate::net::api_client(10).ok()?;
    let resp = client.get(REMOTE).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().ok()?;
    let obj = v.as_object()?;
    let mut out = HashMap::new();
    for (sha, tag) in obj {
        if sha.len() == 64 {
            if let Some(t) = tag.as_str() {
                out.insert(sha.clone(), t.to_string());
            }
        }
    }
    Some(out)
}

/// 合并所有来源并落盘，返回新增数量
pub fn refresh(
    data: &Path,
    cache: &Path,
    catalog: &mut HashMap<String, String>,
) -> Result<RefreshResult, String> {
    let before = catalog.len();
    for (sha, tag) in from_cache(cache) {
        catalog.entry(sha).or_insert(tag);
    }
    let remote = from_remote();
    if let Some(map) = &remote {
        for (sha, tag) in map {
            catalog.entry(sha.clone()).or_insert_with(|| tag.clone());
        }
    }
    crate::paths::save_catalog(data, catalog)?;
    Ok(RefreshResult {
        added: catalog.len().saturating_sub(before),
        total: catalog.len(),
        remote: remote.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_json() {
        let dir = crate::paths::test_dir("catalog-parse");
        let p = dir.join("nr_fg_package.json");
        let sha = "a".repeat(64);
        fs::write(&p, format!("{{\"provider_sha256\":\"{sha}\",\"tag\":\"FG-V.B18\"}}")).unwrap();
        let got = from_package_json(&p).unwrap();
        assert_eq!(got.0, sha);
        assert_eq!(got.1, "FG-V.B18");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_short_hash() {
        let dir = crate::paths::test_dir("catalog-bad");
        let p = dir.join("nr_fg_package.json");
        fs::write(&p, "{\"provider_sha256\":\"abc\"}").unwrap();
        assert!(from_package_json(&p).is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
