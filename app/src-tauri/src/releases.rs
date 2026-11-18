use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::model::Version;

/// 一次拉够全部历史版本，避免已下载的旧版本因为分页被裁掉。
const API: &str = "https://api.github.com/repos/apex-compute/ANR/releases?per_page=100";

/// GitHub API 的镜像前缀（把完整 URL 拼在后面），只作兜底线路。
/// gh-proxy 确认能转发 api.github.com（自带 token，配额 5000/h），
/// 而 ghfast 只认 github.com / raw 路径，对 api 直接回 403，所以不列它。
const API_MIRRORS: &[(&str, &str)] = &[("gh-proxy", "https://gh-proxy.com/")];

/// 单条线路的超时：要多线路重试，单次就不能拖太久，否则整页转圈半分钟。
const API_TIMEOUT: u64 = 12;

/// 镜像线路的超时更短：它只是加速选项，不值得排在前面等太久。
const MIRROR_TIMEOUT: u64 = 8;

/// 多线路重试的总等待上限。全都连不上时宁可早点报错，也不要让用户干等。
const RETRY_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);

/// 版本页默认只展示最近 N 个版本；更早的只在已下载 / 使用中时保留。
pub const VISIBLE_RECENT: usize = 5;

#[derive(Clone)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone)]
pub struct Release {
    pub tag: String,
    pub published_at: String,
    pub body: String,
    pub assets: Vec<Asset>,
}

/// 一条候选线路：去哪拿数据、要不要走系统代理。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    pub label: String,
    pub url: String,
    pub proxy: bool,
}

/// 按「下载源」排出尝试顺序：
/// - github（直连）：先真的直连（不走系统代理），失败再试代理、镜像；
/// - mirror（镜像）：先镜像，再直连、代理；
/// - auto：先跟随系统代理，再直连、镜像。
///
/// 以前这两个选项只影响安装包下载，版本列表永远走 api.github.com + 系统代理，
/// 所以「开着代理时选直连还是镜像都一样加载不出来」。现在直连是真不吃代理，
/// 镜像也把 API 请求一起镜像了。
pub fn routes(mirror: &str, proxy: Option<&str>) -> Vec<Route> {
    let proxy_ok = proxy.is_some() && !crate::net::bypassed(&crate::net::host_of(API));
    let proxy_label = match proxy {
        Some(p) => format!("系统代理 {p}"),
        None => "系统代理".to_string(),
    };

    let mut out: Vec<Route> = Vec::new();
    let push = |out: &mut Vec<Route>, label: String, url: String, proxy: bool| {
        if out.iter().any(|r| r.url == url && r.proxy == proxy) {
            return;
        }
        out.push(Route { label, url, proxy });
    };

    match mirror {
        "mirror" => {
            for (name, base) in API_MIRRORS {
                push(&mut out, format!("镜像 {name}"), format!("{base}{API}"), false);
            }
            push(&mut out, "直连 GitHub".into(), API.into(), false);
            if proxy_ok {
                push(&mut out, proxy_label, API.into(), true);
            }
        }
        "github" => {
            push(&mut out, "直连 GitHub".into(), API.into(), false);
            if proxy_ok {
                push(&mut out, proxy_label, API.into(), true);
            }
            for (name, base) in API_MIRRORS {
                push(&mut out, format!("镜像 {name}"), format!("{base}{API}"), false);
            }
        }
        _ => {
            if proxy_ok {
                push(&mut out, proxy_label, API.into(), true);
            }
            push(&mut out, "直连 GitHub".into(), API.into(), false);
            for (name, base) in API_MIRRORS {
                push(&mut out, format!("镜像 {name}"), format!("{base}{API}"), false);
            }
        }
    }
    out
}

fn net_error(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "连接超时".into()
    } else if e.is_connect() {
        "连不上".into()
    } else if e.is_decode() {
        "响应解析失败".into()
    } else {
        format!("请求失败（{e}）")
    }
}

fn fetch(route: &Route) -> Result<Vec<Release>, String> {
    // 镜像只是加速选项，不值得等太久；api.github.com 慢一点也忍
    let timeout = if route.url == API { API_TIMEOUT } else { MIRROR_TIMEOUT };
    let client = crate::net::client_with(timeout, crate::net::API_UA, route.proxy)?;
    let resp = client
        .get(&route.url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .map_err(|e| net_error(&e))?;

    let status = resp.status();
    if !status.is_success() {
        let limited = resp
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim() == "0")
            .unwrap_or(false);
        return Err(if limited {
            format!("HTTP {}（这个出口 IP 的 GitHub API 配额用完了）", status.as_u16())
        } else {
            format!("HTTP {}", status.as_u16())
        });
    }

    let text = resp.text().map_err(|e| format!("读取响应失败：{e}"))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| "返回的不是版本列表（可能被网络中间层拦了）".to_string())?;
    // 限流/报错时 GitHub 回的是 {"message": ...} 这种对象，不能当成「0 个版本」放过去
    if !json.is_array() {
        let msg = json.get("message").and_then(|x| x.as_str()).unwrap_or("");
        return Err(if msg.is_empty() {
            "返回的不是版本列表（可能被网络中间层拦了）".to_string()
        } else {
            format!("GitHub 报错：{}", truncate(msg, 60))
        });
    }
    Ok(parse_releases(json))
}

/// 拉版本列表。按下载源依次尝试多条线路，第一条成功就返回。
///
/// 之前只有一条「api.github.com + 系统代理」的硬路：开着代理时节点拒掉
/// api.github.com、或者代理出口 IP 的 API 配额被用光，整页就直接空了。
pub fn list_releases(mirror: &str) -> Result<Vec<Release>, String> {
    let proxy = crate::net::proxy_url();
    let plan = routes(mirror, proxy.as_deref());
    let mut errors: Vec<String> = Vec::new();
    let started = std::time::Instant::now();

    for route in &plan {
        // 已经试过至少一条、且总时长超预算时收手，避免整页卡一分钟
        if !errors.is_empty() && started.elapsed() > RETRY_BUDGET {
            errors.push("其余线路已跳过（等待超时）".into());
            break;
        }
        match fetch(route) {
            Ok(mut list) => {
                sort_by_date(&mut list);
                return Ok(list);
            }
            Err(e) => errors.push(format!("{} → {e}", route.label)),
        }
    }

    Err(format!(
        "{} 条线路都失败了（{}）。可以在设置里切换下载源，或关掉代理再试",
        plan.len(),
        errors.join("；")
    ))
}

fn parse_releases(json: serde_json::Value) -> Vec<Release> {
    let arr = json.as_array().cloned().unwrap_or_default();

    let mut out = Vec::new();
    for r in arr {
        let tag = r.get("tag_name").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if tag.is_empty() {
            continue;
        }
        let prerelease = r.get("prerelease").and_then(|x| x.as_bool()).unwrap_or(false);
        if prerelease {
            continue;
        }
        let published_at = r
            .get("published_at")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let body = r.get("body").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let mut assets = Vec::new();
        if let Some(list) = r.get("assets").and_then(|x| x.as_array()) {
            for a in list {
                let name = a.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let size = a.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
                let url = a
                    .get("browser_download_url")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                assets.push(Asset { name, size, url });
            }
        }
        out.push(Release { tag, published_at, body, assets });
    }
    // GitHub 的 /releases 按 created_at 排序；批量发布时多个版本 created_at 完全相同，
    // 返回顺序并不保证「最新发布」在最前。统一按 published_at 降序，
    // 同一时间再按 tag 里的版本号兜底，保证 first() 永远是真正的最新版。
    sort_by_date(&mut out);
    out
}

/// 按发布时间倒序排列；同一时间按 tag 里的版本号（B9 / B10）而不是字符串序。
pub fn sort_by_date(out: &mut [Release]) {
    out.sort_by(|a, b| {
        b.published_at
            .cmp(&a.published_at)
            .then_with(|| version_no(&b.tag).cmp(&version_no(&a.tag)))
            .then_with(|| b.tag.cmp(&a.tag))
    });
}

/// 从 FG-V.B18 / V.B18 里取出 18，用于同一发布时间的版本排序。
fn version_no(tag: &str) -> u64 {
    let bytes = tag.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'B' || bytes[i] == b'b' {
            let mut n = 0u64;
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                n = n * 10 + u64::from(bytes[j] - b'0');
                j += 1;
            }
            if j > i + 1 {
                return n;
            }
        }
        i += 1;
    }
    0
}

pub fn find_asset<'a>(rel: &'a Release, kind: &str) -> Option<&'a Asset> {
    let suffix = format!("-{kind}.zip");
    rel.assets
        .iter()
        .find(|a| a.name.starts_with("anr-fg-") && a.name.ends_with(&suffix))
        .or_else(|| rel.assets.iter().find(|a| a.name.starts_with("anr-fg-") && a.name.ends_with(".zip")))
}

pub fn sums_asset(rel: &Release) -> Option<&Asset> {
    rel.assets.iter().find(|a| a.name.eq_ignore_ascii_case("SHA256SUMS.txt"))
}

/// 只要某个版本的任一包型（full / addon / 其它）解压完成，就算「下载过」。
/// 之前只认 full/.ok，用 addon 包装的游戏会一直显示成未下载。
fn is_cached(cache: &Path, tag: &str) -> bool {
    let dir = cache.join(tag);
    for kind in ["full", "addon"] {
        if dir.join(kind).join(".ok").exists() {
            return true;
        }
    }
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().join(".ok").exists() {
                return true;
            }
        }
    }
    false
}

/// 生成版本页数据。
///
/// 列表规则：按发布时间倒序，只保留最近 `VISIBLE_RECENT` 个版本，
/// 外加所有已下载（缓存）和已安装到游戏里的版本。历史版本不会把页面撑爆，
/// 而用户用过的旧版本仍然可见、可回退。
pub fn to_versions(
    releases: &[Release],
    cache: &Path,
    catalog: &HashMap<String, String>,
    installed_tags: &[String],
) -> Vec<Version> {
    let mut all = Vec::new();
    for r in releases {
        let Some(full) = find_asset(r, "full") else { continue };
        let cached = is_cached(cache, &r.tag);
        let fingerprint = catalog
            .iter()
            .find(|(_, v)| v.eq_ignore_ascii_case(&r.tag))
            .map(|(k, _)| k.chars().take(12).collect::<String>())
            .unwrap_or_default();
        let notes = r
            .body
            .lines()
            .find(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
            .map(|l| l.trim().trim_start_matches(['-', '*', ' ']).to_string())
            .unwrap_or_default();
        all.push(Version {
            tag: r.tag.clone(),
            product: "ANR FG Beta".into(),
            published_at: r.published_at.clone(),
            size_mb: (full.size as f64 / 1_048_576.0 * 10.0).round() / 10.0,
            cached,
            latest: false,
            fingerprint,
            notes: truncate(&notes, 90),
        });
    }

    // 不依赖调用方给的顺序，这里再排一次。
    all.sort_by(|a, b| {
        b.published_at
            .cmp(&a.published_at)
            .then_with(|| version_no(&b.tag).cmp(&version_no(&a.tag)))
    });

    let mut out: Vec<Version> = all
        .into_iter()
        .enumerate()
        .filter(|(i, v)| {
            *i < VISIBLE_RECENT
                || v.cached
                || installed_tags.iter().any(|t| t.eq_ignore_ascii_case(&v.tag))
        })
        .map(|(_, v)| v)
        .collect();
    // latest 落在输出列表里真正的最新版上：即使首个 release 没有 full 包被跳过，
    // 也不会出现「整页没有最新版」。
    if let Some(first) = out.first_mut() {
        first.latest = true;
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

pub fn mirror_url(url: &str, mirror: &str) -> String {
    if mirror == "mirror" {
        format!("https://gh-proxy.com/{url}")
    } else {
        url.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn rel(tag: &str, published_at: &str) -> Release {
        Release {
            tag: tag.into(),
            published_at: published_at.into(),
            body: "## Changes\n\n- Something new.\n".into(),
            assets: vec![Asset {
                name: format!("anr-fg-{tag}-full.zip"),
                size: 10 * 1_048_576,
                url: String::new(),
            }],
        }
    }

    fn tmp(label: &str) -> PathBuf {
        let n = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p = std::env::temp_dir().join(format!("anr-studio-{label}-{}-{n}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn sample(n: usize) -> Vec<Release> {
        (0..n)
            .map(|i| rel(&format!("FG-V.B{n:02}", n = n - i), &format!("2026-11-{:02}T00:00:00Z", n - i + 1)))
            .collect()
    }

    #[test]
    fn version_number_parsing() {
        assert_eq!(version_no("FG-V.B18"), 18);
        assert_eq!(version_no("V.B7"), 7);
        assert_eq!(version_no("no-number"), 0);
    }

    #[test]
    fn sort_puts_newest_first_even_with_identical_timestamps() {
        let mut rels = vec![
            rel("FG-V.B16", "2026-11-14T09:54:33Z"),
            rel("FG-V.B18", "2026-11-14T23:54:24Z"),
            rel("FG-V.B9", "2026-11-12T00:00:00Z"),
            rel("FG-V.B10", "2026-11-12T00:00:00Z"),
            rel("FG-V.B17", "2026-11-14T21:17:21Z"),
        ];
        sort_by_date(&mut rels);
        let tags: Vec<&str> = rels.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, vec!["FG-V.B18", "FG-V.B17", "FG-V.B16", "FG-V.B10", "FG-V.B9"]);
    }

    #[test]
    fn latest_is_newest_by_publish_time_not_input_order() {
        let dir = tmp("order");
        // 故意把旧版本放在数组最前面
        let mut rels = sample(8);
        rels.reverse();
        let out = to_versions(&rels, &dir, &HashMap::new(), &[]);
        assert_eq!(out.len(), 5);
        assert_eq!(out[0].tag, "FG-V.B08");
        assert!(out[0].latest);
        assert_eq!(out.iter().filter(|v| v.latest).count(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn keeps_downloaded_and_installed_old_versions() {
        let dir = tmp("keep");
        let rels = sample(12);
        // 第 9、12 个版本（按发布时间倒序）分别是已下载、使用中
        fs::create_dir_all(dir.join("FG-V.B04").join("addon")).unwrap();
        fs::write(dir.join("FG-V.B04").join("addon").join(".ok"), b"ok").unwrap();
        let out = to_versions(&rels, &dir, &HashMap::new(), &["FG-V.B01".into()]);
        let tags: Vec<&str> = out.iter().map(|v| v.tag.as_str()).collect();
        assert_eq!(out.len(), 7, "5 个最新 + 1 个已下载 + 1 个使用中");
        assert!(tags.contains(&"FG-V.B04"), "addon 缓存应被识别为已下载");
        assert!(tags.contains(&"FG-V.B01"), "使用中的旧版本应保留");
        assert!(out[0].latest);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn full_cache_marker_is_still_recognized() {
        let dir = tmp("full");
        fs::create_dir_all(dir.join("FG-V.B02").join("full")).unwrap();
        fs::write(dir.join("FG-V.B02").join("full").join(".ok"), b"ok").unwrap();
        let out = to_versions(&sample(8), &dir, &HashMap::new(), &[]);
        assert!(out.iter().any(|v| v.tag == "FG-V.B02" && v.cached));
        let _ = fs::remove_dir_all(dir);
    }

    fn labels(rs: &[Route]) -> Vec<String> {
        rs.iter().map(|r| format!("{}{}", r.label, if r.proxy { "+代理" } else { "" })).collect()
    }

    #[test]
    fn direct_mode_really_goes_direct_even_with_proxy() {
        let rs = routes("github", Some("http://127.0.0.1:7890"));
        assert!(!rs.is_empty());
        assert_eq!(rs[0].url, API, "直连模式下第一条必须是不走代理的 api.github.com");
        assert!(!rs[0].proxy, "直连模式下第一条不能套系统代理");
        // 后面仍要有代理 / 镜像兜底
        assert!(rs.iter().any(|r| r.proxy));
        assert!(rs.iter().any(|r| r.url != API));
    }

    #[test]
    fn mirror_mode_tries_mirror_first_and_covers_api() {
        let rs = routes("mirror", Some("http://127.0.0.1:7890"));
        assert!(rs[0].url.starts_with("https://gh-proxy.com/https://api.github.com/"));
        assert!(!rs[0].proxy);
        // 镜像模式下也要有直连 / 代理兜底，否则镜像挂了就完全没救
        assert!(rs.iter().any(|r| r.url == API && !r.proxy));
        assert!(rs.iter().any(|r| r.url == API && r.proxy));
    }

    #[test]
    fn auto_mode_follows_system_proxy_first() {
        let rs = routes("auto", Some("http://127.0.0.1:7890"));
        assert!(rs[0].proxy, "auto 下开着代理就先用代理");
        assert!(rs.iter().any(|r| !r.proxy), "代理失败后要能自动退回直连");
        let none = routes("auto", None);
        assert!(none.iter().all(|r| !r.proxy), "没有代理时不该出现代理线路");
        assert!(!labels(&none).iter().any(|l| l.contains("代理")));
    }

    /// 需要真联网，默认不跑：`cargo test --lib -- --ignored live`
    #[test]
    #[ignore]
    fn live_fetch_works_on_some_route() {
        for mode in ["auto", "github", "mirror"] {
            match list_releases(mode) {
                Ok(list) => {
                    assert!(!list.is_empty(), "{mode} 拿到了空列表");
                    println!("{mode}: {} 个版本，最新 {}", list.len(), list[0].tag);
                }
                Err(e) => panic!("{mode} 全线路失败：{e}"),
            }
        }
    }
}
