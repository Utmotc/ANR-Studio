//! 版本更新日志：从 GitHub release 的 body 里抽出「变更」条目，
//! 并给每条配一份中文译文（免费翻译接口，结果按 tag 缓存到本地）。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    /// 英文原文
    pub en: String,
    /// 中文译文；翻译失败时为空字符串
    #[serde(default)]
    pub zh: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Changelog {
    pub tag: String,
    pub title: String,
    pub items: Vec<Item>,
    pub url: String,
    /// "github" 表示取自 release 正文，"none" 表示这个版本没有正文
    pub source: String,
}

/// 把 markdown 行内语法削掉，只留可读文本
fn plain(line: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '[' {
            // [text](url) → text
            if let Some(close) = chars[i..].iter().position(|&x| x == ']') {
                let text: String = chars[i + 1..i + close].iter().collect();
                let after = i + close + 1;
                if after < chars.len() && chars[after] == '(' {
                    if let Some(end) = chars[after..].iter().position(|&x| x == ')') {
                        out.push_str(&text);
                        i = after + end + 1;
                        continue;
                    }
                }
            }
        }
        if c == '`' || c == '*' || c == '_' && i + 1 < chars.len() && chars[i + 1] == '_' {
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.replace("  ", " ").trim().to_string()
}

fn heading_text(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return None;
    }
    Some(t.trim_start_matches('#').trim())
}

/// 判断是不是「变更」小节
fn is_changes_heading(h: &str) -> bool {
    let l = h.to_lowercase();
    l.contains("change") || l.contains("what's new") || l.contains("whats new") || l.contains("changelog")
}

/// 判断是不是该跳过的小节（安装方式、下载包、硬件支持这些每版一样的样板）
fn is_noise_heading(h: &str) -> bool {
    let l = h.to_lowercase();
    l.contains("package")
        || l.contains("install")
        || l.contains("download")
        || l.contains("supported gpu")
        || l.contains("requirement")
        || l.contains("feedback")
        || l.contains("known issue")
}

fn split_items(lines: &[&str]) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    // 续行（缩进的补充说明）并到上一条
    for raw in lines {
        let line = raw.trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let trimmed = line.trim_start();
        if heading_text(line).is_some() {
            continue;
        }
        let bullet = trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("• ");
        let indented = line.len() != trimmed.len();
        let text = plain(trimmed.trim_start_matches(['-', '*', '•', ' ']));
        if text.is_empty() {
            continue;
        }
        if bullet {
            items.push(Item { en: text, zh: String::new() });
        } else if indented {
            if let Some(last) = items.last_mut() {
                last.en.push(' ');
                last.en.push_str(&text);
            } else {
                items.push(Item { en: text, zh: String::new() });
            }
        } else {
            items.push(Item { en: text, zh: String::new() });
        }
    }
    items
}

/// 从 release 正文抽条目：优先取「Changes」小节，没有就取全文里的条目
pub fn parse(body: &str) -> Vec<Item> {
    let norm = body.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = norm.lines().collect();

    let start = lines
        .iter()
        .position(|l| heading_text(l).map(is_changes_heading).unwrap_or(false));

    if let Some(s) = start {
        let mut end = lines.len();
        for (i, l) in lines.iter().enumerate().skip(s + 1) {
            if let Some(h) = heading_text(l) {
                if is_noise_heading(h) || !is_changes_heading(h) {
                    end = i;
                    break;
                }
            }
        }
        let items = split_items(&lines[s + 1..end]);
        if !items.is_empty() {
            return items;
        }
    }

    // 没有 Changes 小节：取 Packages 之前的全部内容
    let stop = lines
        .iter()
        .position(|l| heading_text(l).map(is_noise_heading).unwrap_or(false))
        .unwrap_or(lines.len());
    let items = split_items(&lines[..stop]);
    items.into_iter().filter(|i| i.en.chars().count() > 12).collect()
}

// ---------- 翻译 ----------

fn cache_path(data: &Path, tag: &str) -> PathBuf {
    let safe: String = tag
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    data.join("changelog").join(format!("{safe}.json"))
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent("ANR-Studio/0.1 (+https://github.com/apex-compute/ANR)")
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|e| e.to_string())
}

/// 按句子切块，控制单次请求体积
fn chunk(text: &str, max: usize) -> Vec<String> {
    if text.chars().count() <= max {
        return vec![text.to_string()];
    }
    let mut out = Vec::new();
    let mut cur = String::new();
    for piece in text.split_inclusive(['.', ';', '?', '!', '。', '；']) {
        if cur.chars().count() + piece.chars().count() > max && !cur.is_empty() {
            out.push(cur.trim().to_string());
            cur = String::new();
        }
        cur.push_str(piece);
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn translate_block(cli: &reqwest::blocking::Client, text: &str) -> Result<String, String> {
    let mut out = String::new();
    for part in chunk(text, 420) {
        let url = reqwest::Url::parse_with_params(
            "https://api.mymemory.translated.net/get",
            &[("q", part.as_str()), ("langpair", "en|zh-CN")],
        )
        .map_err(|e| e.to_string())?;
        let resp = cli.get(url).send().map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("翻译接口返回 {}", resp.status()));
        }
        let json: serde_json::Value = resp.json().map_err(|e| e.to_string())?;
        let status = json
            .get("responseStatus")
            .and_then(|x| x.as_i64())
            .unwrap_or(200);
        let t = json
            .pointer("/responseData/translatedText")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        // MyMemory 在超额或参数有误时会把错误信息塞进 translatedText
        if status != 200 || t.is_empty() || t.contains("QUERY LENGTH LIMIT") || t.contains("INVALID") {
            return Err("翻译暂不可用".into());
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&t);
        std::thread::sleep(Duration::from_millis(120));
    }
    Ok(out)
}

/// 逐个条目翻译；已缓存过的版本直接读本地
fn translate_all(data: &Path, tag: &str, items: &[Item]) -> Vec<Item> {
    let cp = cache_path(data, tag);
    if let Ok(text) = fs::read_to_string(&cp) {
        if let Ok(cached) = serde_json::from_str::<Vec<Item>>(&text) {
            if cached.len() == items.len()
                && cached.iter().zip(items).all(|(a, b)| a.en == b.en && !a.zh.is_empty())
            {
                return cached;
            }
        }
    }

    let Ok(cli) = client() else { return items.to_vec() };
    let mut out = Vec::with_capacity(items.len());
    let mut complete = true;
    for it in items {
        match translate_block(&cli, &it.en) {
            Ok(zh) => out.push(Item { en: it.en.clone(), zh }),
            Err(_) => {
                complete = false;
                out.push(Item { en: it.en.clone(), zh: String::new() });
            }
        }
    }

    if complete && !out.is_empty() {
        if let Some(dir) = cp.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string(&out) {
            let _ = fs::write(&cp, text);
        }
    }
    out
}

pub fn build(data: &Path, rel: Option<&crate::releases::Release>, tag: &str) -> Changelog {
    let Some(rel) = rel else {
        return Changelog {
            tag: tag.to_string(),
            title: String::new(),
            items: Vec::new(),
            url: format!("https://github.com/apex-compute/ANR/releases/tag/{tag}"),
            source: "none".into(),
        };
    };

    let body = rel.body.trim();
    if body.is_empty() {
        return Changelog {
            tag: tag.to_string(),
            title: String::new(),
            items: Vec::new(),
            url: format!("https://github.com/apex-compute/ANR/releases/tag/{tag}"),
            source: "none".into(),
        };
    }

    let title = body
        .lines()
        .next()
        .map(|l| plain(l.trim_start_matches('#')))
        .unwrap_or_default();

    let items = parse(body);
    let items = if items.is_empty() { items } else { translate_all(data, tag, &items) };

    Changelog {
        tag: tag.to_string(),
        title,
        items,
        url: format!("https://github.com/apex-compute/ANR/releases/tag/{tag}"),
        source: "github".into(),
    }
}
