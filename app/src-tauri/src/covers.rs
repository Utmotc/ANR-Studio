use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use base64::Engine;

static PROVIDER: OnceLock<String> = OnceLock::new();

fn client(timeout: u64) -> Result<reqwest::blocking::Client, String> {
    crate::net::web_client(timeout)
}

fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// 判断名字里是否有中日韩字符：决定查哪个语言的商店索引
fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c as u32,
            0x3040..=0x30FF      // 日文假名
            | 0x3400..=0x4DBF    // 扩展 A
            | 0x4E00..=0x9FFF    // 基本汉字
            | 0xAC00..=0xD7AF    // 韩文
            | 0xF900..=0xFAFF)
    })
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>()
        .to_lowercase()
}

fn steam_reachable() -> bool {
    let Ok(c) = client(4) else { return false };
    c.get("https://store.steampowered.com/api/storesearch/?term=portal&cc=CN&l=schinese")
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

fn gog_reachable() -> bool {
    let Ok(c) = client(4) else { return false };
    c.get("https://www.gog.com/games/ajax/filtering/api/zh/games?search=portal&limit=1")
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// 探测结果只缓存"可用"的源；两个都探测失败时不缓存，
/// 60 秒后下一次请求会重试（否则一次网络抖动就会让本次运行永远抓不到封面）。
static PROVIDER_AT: OnceLock<std::sync::Mutex<Option<std::time::Instant>>> =
    OnceLock::new();

fn tried_at() -> &'static std::sync::Mutex<Option<std::time::Instant>> {
    PROVIDER_AT.get_or_init(|| std::sync::Mutex::new(None))
}

/// 优先 Steam 商店，其次 GOG；都不可用时返回 "none"（下次调用会重试）
pub fn provider() -> &'static str {
    if let Some(p) = PROVIDER.get() {
        return p.as_str();
    }

    let mut last = tried_at().lock().unwrap();
    if let Some(at) = *last {
        if at.elapsed() < Duration::from_secs(60) {
            return "none";
        }
    }
    *last = Some(std::time::Instant::now());

    let found = if steam_reachable() {
        Some("steam")
    } else if gog_reachable() {
        Some("gog")
    } else {
        None
    };

    match found {
        Some(p) => PROVIDER.get_or_init(|| p.to_string()).as_str(),
        None => "none",
    }
}

const JUNK: &[&str] = &[
    "dlc",
    "soundtrack",
    "demo",
    "beta",
    "mod",
    "tool",
    "editor",
    "server",
    "benchmark",
    "artbook",
    "bonus content",
    "pack",
];

fn is_junk(name: &str) -> bool {
    let n = name.to_lowercase();
    JUNK.iter().any(|k| n.contains(k))
}

/// 在搜索结果里挑最匹配的一条：先看名字相似度，再用结果顺序（Steam 按相关度排）做次级排序。
fn pick_best<'a>(items: &'a [serde_json::Value], name: &str) -> Option<&'a serde_json::Value> {
    let target = norm(name);
    let mut best: Option<(i32, &serde_json::Value)> = None;

    for it in items {
        let Some(n) = it.get("name").and_then(|x| x.as_str()) else {
            continue;
        };
        if is_junk(n) {
            continue;
        }
        let cand = norm(n);
        if cand.is_empty() {
            continue;
        }
        let score = if cand == target {
            100
        } else if cand.starts_with(&target) || target.starts_with(&cand) {
            let diff = (cand.len() as i32 - target.len() as i32).abs();
            if diff <= 4 {
                80
            } else {
                0
            }
        } else if cand.contains(&target) || target.contains(&cand) {
            let diff = (cand.len() as i32 - target.len() as i32).abs();
            if diff <= 6 {
                60
            } else {
                0
            }
        } else {
            0
        };
        if score > 0 && best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((score, it));
        }
    }
    best.map(|(_, v)| v)
}

/// 名字对不上时的兜底：取第一条非噪音的应用型结果。
/// Steam 的搜索按相关度排序，用户手输的名字通常就在第一条。
fn first_usable(items: &[serde_json::Value]) -> Option<&serde_json::Value> {
    items.iter().find(|it| {
        let kind_ok = it
            .get("type")
            .and_then(|x| x.as_str())
            .map(|t| t == "app")
            .unwrap_or(true);
        let name_ok = it
            .get("name")
            .and_then(|x| x.as_str())
            .map(|n| !is_junk(n))
            .unwrap_or(false);
        let id_ok = it.get("id").and_then(|x| x.as_u64()).is_some();
        kind_ok && name_ok && id_ok
    })
}

/// 查一次 Steam 商店搜索。lang 决定返回的名字语言，cc 决定索引地区——
/// 实测中文查询在 cc=cn 下偶尔返回空，cc=us 更稳。
/// 商店接口地址。debug 构建允许用环境变量指向本地假服务器，方便离线验证整条链路；
/// release 构建永远走官方地址。
fn steam_base() -> String {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("ANR_STEAM_BASE") {
        if !v.trim().is_empty() {
            return v.trim_end_matches('/').to_string();
        }
    }
    "https://store.steampowered.com".to_string()
}

fn steam_cdn() -> String {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("ANR_STEAM_CDN") {
        if !v.trim().is_empty() {
            return v.trim_end_matches('/').to_string();
        }
    }
    "https://cdn.cloudflare.steamstatic.com".to_string()
}

fn steam_search(term: &str, lang: &str, cc: &str) -> Result<Vec<serde_json::Value>, String> {
    let url = format!(
        "{}/api/storesearch/?term={}&cc={cc}&l={lang}",
        steam_base(),
        enc(term)
    );
    let resp = client(6)?
        .get(&url)
        .send()
        .map_err(|e| format!("{e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().map_err(|e| format!("{e}"))?;
    Ok(v.get("items")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default())
}

fn from_steam(name: &str) -> Result<Vec<u8>, String> {
    // 中文名必须用 schinese 查，否则拿到的是英文标题，永远匹配不上；
    // 英文名优先 english，再去中文索引碰运气。
    let order: [(&str, &str); 4] = if has_cjk(name) {
        [
            ("schinese", "cn"),
            ("schinese", "us"),
            ("english", "cn"),
            ("english", "us"),
        ]
    } else {
        [
            ("english", "cn"),
            ("english", "us"),
            ("schinese", "cn"),
            ("schinese", "us"),
        ]
    };

    let mut got_items = false;
    let mut net_fails = 0;
    let mut last_err = String::from("Steam 上没有搜到这个游戏");

    for (lang, cc) in order {
        let items = match steam_search(name, lang, cc) {
            Ok(v) => {
                net_fails = 0;
                v
            }
            Err(e) => {
                net_fails += 1;
                last_err = format!("连不上 Steam：{e}");
                #[cfg(debug_assertions)]
                eprintln!("[cover] steam {lang}/{cc} -> ERROR {e}");
                // 网络断了就不用把四种组合都试一遍，省得让用户干等
                if net_fails >= 2 {
                    break;
                }
                continue;
            }
        };
        #[cfg(debug_assertions)]
        eprintln!(
            "[cover] steam {lang}/{cc} -> {} items, first={:?}",
            items.len(),
            items
                .first()
                .and_then(|i| i.get("name"))
                .and_then(|v| v.as_str())
        );
        if items.is_empty() {
            continue;
        }
        got_items = true;

        let best = pick_best(&items, name).or_else(|| first_usable(&items));
        let Some(best) = best else { continue };
        let Some(appid) = best.get("id").and_then(|x| x.as_u64()) else {
            continue;
        };

        #[cfg(debug_assertions)]
        eprintln!("[cover] picked appid {appid}");

        for file in ["header.jpg", "capsule_231x87.jpg"] {
            let url = format!("{}/steam/apps/{appid}/{file}", steam_cdn());
            let Ok(cli) = client(10) else { continue };
            match cli.get(&url).send() {
                Ok(resp) => {
                    let ok = resp.status().is_success();
                    match resp.bytes() {
                        Ok(bytes) => {
                            #[cfg(debug_assertions)]
                            eprintln!("[cover] {file} -> ok={ok} len={}", bytes.len());
                            if ok && bytes.len() > 2000 {
                                return Ok(bytes.to_vec());
                            }
                        }
                        Err(e) => {
                            // release 构建里没有调试输出，显式标记为已使用，避免 unused 警告
                            let _ = &e;
                            #[cfg(debug_assertions)]
                            eprintln!("[cover] {file} -> read error {e}");
                        }
                    }
                }
                Err(e) => {
                    // release 构建里没有调试输出，显式标记为已使用，避免 unused 警告
                    let _ = &e;
                    #[cfg(debug_assertions)]
                    eprintln!("[cover] {file} -> send error {e}");
                }
            }
        }
    }

    Err(if got_items {
        "搜到了结果但拿不到封面".into()
    } else {
        last_err
    })
}

fn from_gog(name: &str) -> Result<Vec<u8>, String> {
    let url = format!(
        "https://www.gog.com/games/ajax/filtering/api/zh/games?search={}&limit=5",
        enc(name)
    );
    let resp = client(8)?
        .get(&url)
        .send()
        .map_err(|e| format!("搜索失败：{e}"))?;
    let v: serde_json::Value = resp
        .json()
        .map_err(|_| "GOG 接口暂时不可用".to_string())?;
    let products = v
        .get("products")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let best = pick_best(&products, name).ok_or("GOG 上没有搜到这个游戏")?;
    let image = best.get("image").and_then(|x| x.as_str()).unwrap_or("");
    if image.is_empty() {
        return Err("搜索结果里没有图片".into());
    }
    let mut full = if let Some(rest) = image.strip_prefix("//") {
        format!("https://{rest}")
    } else if image.starts_with("http") {
        image.to_string()
    } else {
        format!("https://www.gog.com{image}")
    };
    if !full.rsplit('/').next().unwrap_or("").contains('.') {
        full.push_str(".jpg");
    }
    let r = client(10)?
        .get(&full)
        .send()
        .map_err(|e| format!("封面下载失败：{e}"))?;
    if !r.status().is_success() {
        return Err("封面下载失败".into());
    }
    let bytes = r.bytes().map_err(|e| format!("读取图片失败：{e}"))?;
    if bytes.len() < 2000 {
        return Err("封面太小，判定为失败".into());
    }
    Ok(bytes.to_vec())
}


/// 国内源：360 图片搜索（image.so.com）。JSON 接口、不需要 key，国内网络直连可用。
///
/// 图片搜索最大的风险是"配错游戏"（搜「影之刃零」给你「影之刃3」的图），
/// 所以硬性要求结果标题里出现游戏名，再用横版、标题关键词做排序。
fn from_domestic(name: &str) -> Result<Vec<u8>, String> {
    let url = format!("https://image.so.com/j?q={}&pn=0&rn=30", enc(name));
    let resp = client(8)?
        .get(&url)
        .header("Referer", "https://image.so.com/")
        .send()
        .map_err(|e| format!("连不上图片站：{e}"))?;
    if !resp.status().is_success() {
        return Err(format!("图片站返回 {}", resp.status()));
    }
    let v: serde_json::Value = resp
        .json()
        .map_err(|_| "图片站返回的数据看不懂".to_string())?;
    let list = v
        .get("list")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    if list.is_empty() {
        return Err("国内图片站没有搜到这个游戏".into());
    }

    // 标题里认不出游戏名的一律不要，宁可没有也不给错图
    let target = norm(name);
    let prefix: String = target.chars().take(4).collect();

    let mut cands: Vec<(i32, String)> = Vec::new();
    for it in &list {
        let title = it.get("title").and_then(|x| x.as_str()).unwrap_or("");
        let img = it
            .get("img")
            .and_then(|x| x.as_str())
            .or_else(|| it.get("thumb").and_then(|x| x.as_str()))
            .unwrap_or("");
        if img.is_empty() || title.is_empty() {
            continue;
        }

        let nt = norm(title);
        let name_hit = (!target.is_empty() && nt.contains(&target))
            || (prefix.chars().count() >= 3 && nt.contains(&prefix));
        if !name_hit {
            continue;
        }

        let w = it.get("width").and_then(|x| x.as_u64()).unwrap_or(0) as f64;
        let h = it.get("height").and_then(|x| x.as_u64()).unwrap_or(0) as f64;

        let mut score = 0;
        if w > 0.0 && h > 0.0 {
            if w < 500.0 {
                continue;
            }
            let ratio = w / h;
            if (1.35..=2.1).contains(&ratio) {
                score += 4;
            } else if (1.1..1.35).contains(&ratio) {
                score += 2;
            } else if ratio < 0.9 {
                score -= 2; // 竖版海报在 16:10 卡片里会被裁掉大半
            }
            if w >= 1000.0 {
                score += 1;
            }
        }

        // 标题里带这些词，多半是官宣图 / 封面
        for kw in ["游戏", "封面", "海报", "壁纸", "预告", "公布", "官宣", "宣传"] {
            if title.contains(kw) {
                score += 2;
                break;
            }
        }
        // 这些是新闻配图、攻略截图、人物访谈，不适合当封面
        for kw in ["攻略", "解说", "售价", "预约", "脸模", "试玩", "直播", "评论", "销量", "榜单"] {
            if title.contains(kw) {
                score -= 5;
                break;
            }
        }

        cands.push((score, img.to_string()));
    }

    if cands.is_empty() {
        return Err("国内图片站没有匹配到这个游戏的封面".into());
    }
    cands.sort_by(|a, b| b.0.cmp(&a.0));

    let mut last = String::from("国内图片站没有合适的图");
    for (_, url) in cands.iter().take(5) {
        let Ok(cli) = client(10) else { continue };
        let Ok(r) = cli.get(url).header("Referer", "https://image.so.com/").send() else {
            continue;
        };
        if !r.status().is_success() {
            continue;
        }
        let Ok(bytes) = r.bytes() else { continue };
        if bytes.len() > 8000 && looks_like_image(&bytes) {
            return Ok(bytes.to_vec());
        }
        last = "图片站返回的不是有效图片".to_string();
    }
    Err(last)
}

/// 粗略判断字节流是不是图片（不依赖扩展名）
fn looks_like_image(b: &[u8]) -> bool {
    b.starts_with(&[0xFF, 0xD8])
        || b.starts_with(&[0x89, b'P', b'N', b'G'])
        || b.starts_with(b"GIF8")
        || (b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP")
}

pub fn cover_path(cache: &Path, id: &str) -> PathBuf {
    cache.join(format!("{id}.jpg"))
}

pub fn remove(cache: &Path, id: &str) {
    let _ = fs::remove_file(cover_path(cache, id));
}

/// 有缓存就直接用；否则按「来源」设置抓一张低清图存下来。
///
/// - `domestic`：只用国内图片站
/// - `global`：只用 Steam / GOG
/// - 其他（auto）：优先国外官方封面，探测不到国外服务时直接走国内
pub fn ensure_cover(cache: &Path, id: &str, name: &str, source: &str) -> Result<PathBuf, String> {
    let path = cover_path(cache, id);
    if let Ok(m) = fs::metadata(&path) {
        if m.len() > 1000 {
            return Ok(path);
        }
    }
    let _ = fs::create_dir_all(cache);

    let attempts: Vec<&str> = match source {
        "domestic" => vec!["domestic"],
        "global" => {
            if provider() == "gog" {
                vec!["gog", "steam"]
            } else {
                vec!["steam", "gog"]
            }
        }
        _ => {
            if provider() == "none" {
                // 国外服务探测不到，别让用户干等，直接走国内
                vec!["domestic", "steam", "gog"]
            } else {
                vec!["steam", "gog", "domestic"]
            }
        }
    };

    let mut last = String::from("没有可用的图片来源");
    // 网络不通比"GOG 不可用"更值得告诉用户，所以单独记住第一个网络类错误
    let mut net_err: Option<String> = None;
    for src in attempts {
        let got = match src {
            "steam" => from_steam(name),
            "gog" => from_gog(name),
            "domestic" => from_domestic(name),
            _ => continue,
        };
        match got {
            Ok(bytes) => {
                fs::write(&path, &bytes).map_err(|e| format!("保存封面失败：{e}"))?;
                return Ok(path);
            }
            Err(e) => {
                if net_err.is_none()
                    && (e.contains("连不上")
                        || e.contains("请求")
                        || e.contains("timed out")
                        || e.contains("图片站"))
                {
                    net_err = Some(e.clone());
                }
                last = e;
            }
        }
    }
    Err(net_err.unwrap_or(last))
}

/// 按文件头猜 MIME：用户可能放进来 PNG / WebP，标成 jpeg 的话浏览器会拒绝渲染
pub fn mime_of(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.len() > 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else {
        "image/jpeg"
    }
}

pub fn data_uri(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(format!(
        "data:{};base64,{}",
        mime_of(&bytes),
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cjk_detection() {
        assert!(has_cjk("影之刃零"));
        assert!(has_cjk("サイバーパンク2077"));
        assert!(!has_cjk("Cyberpunk 2077"));
        assert!(!has_cjk("ELDEN RING"));
    }

    #[test]
    fn picks_exact_chinese_title() {
        let items = vec![
            json!({"type":"app","id":111,"name":"影之刃零数字豪华版升级包"}),
            json!({"type":"app","id":222,"name":"影之刃零"}),
            json!({"type":"app","id":333,"name":"明末影之刃零"}),
        ];
        let best = pick_best(&items, "影之刃零").expect("should match");
        assert_eq!(best["id"], 222);
    }

    #[test]
    fn chinese_name_does_not_match_english_titles() {
        // 用 l=english 查中文名时会拿到英文标题，此时不应误判为匹配
        let items = vec![json!({"type":"app","id":1,"name":"Phantom Blade Zero"})];
        assert!(pick_best(&items, "影之刃零").is_none());
        // 但兜底能拿到第一条
        assert_eq!(first_usable(&items).unwrap()["id"], 1);
    }

    #[test]
    fn skips_noise_entries() {
        let items = vec![
            json!({"type":"app","id":1,"name":"Elden Ring Soundtrack"}),
            json!({"type":"app","id":2,"name":"ELDEN RING"}),
        ];
        let best = pick_best(&items, "Elden Ring").expect("should match");
        assert_eq!(best["id"], 2);
    }

    #[test]
    fn norm_ignores_punctuation_and_case() {
        assert_eq!(norm("Cyberpunk 2077"), norm("cyberpunk-2077"));
        assert_eq!(norm("赛博朋克 2077"), norm("赛博朋克2077"));
    }
}
