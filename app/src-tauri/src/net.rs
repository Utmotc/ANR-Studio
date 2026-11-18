//! 统一的 HTTP 客户端。
//!
//! 国内用户大多靠系统代理（Clash / v2ray 之类）上网，而 reqwest 不会读 Windows
//! 的 Internet 设置，所以这里显式读注册表里的 ProxyEnable / ProxyServer。
//!
//! 但「开着代理」并不等于「走代理更顺」：api.github.com 过节点时可能被节点直接拒掉，
//! 或者代理出口 IP 已经把 GitHub 匿名 API 的配额用光（403 限流），这时直连反而是通的。
//! 所以这里既允许显式不要代理（`client_with(.., false)`），也认 Windows 的
//! 「请勿对以下列条目使用代理服务器」白名单。

use std::time::Duration;

/// 代理地址：环境变量优先（调试、命令行启动时可能带），其次读 Windows 系统代理。
/// 设置 ANR_NO_PROXY=1 可强制走直连。
pub fn proxy_url() -> Option<String> {
    if env_on("ANR_NO_PROXY") {
        return None;
    }
    for key in [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
    ] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    system_proxy()
}

fn env_on(key: &str) -> bool {
    match std::env::var(key) {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !v.is_empty() && v != "0" && v != "false" && v != "off"
        }
        Err(_) => false,
    }
}

#[cfg(windows)]
fn system_proxy() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
        .ok()?;

    let enabled: u32 = key.get_value("ProxyEnable").ok()?;
    if enabled == 0 {
        return None;
    }
    let server: String = key.get_value("ProxyServer").ok()?;
    let server = server.trim();
    if server.is_empty() {
        return None;
    }

    // 可能是 "127.0.0.1:7890"，也可能是 "http=...;https=...;socks=..."
    let picked = if let Some(pos) = server.find("https=") {
        server[pos + 6..].split(';').next().unwrap_or("").to_string()
    } else if let Some(pos) = server.find("http=") {
        server[pos + 5..].split(';').next().unwrap_or("").to_string()
    } else {
        server.to_string()
    };

    let picked = picked.trim();
    if picked.is_empty() {
        return None;
    }
    Some(if picked.contains("://") {
        picked.to_string()
    } else {
        format!("http://{picked}")
    })
}

#[cfg(not(windows))]
fn system_proxy() -> Option<String> {
    None
}

/// 系统代理的例外列表（注册表 ProxyOverride），"127.*;*.corp.com;<local>" 这种
#[cfg(windows)]
fn override_list() -> Vec<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let Ok(key) = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
    else {
        return Vec::new();
    };
    let raw: String = match key.get_value("ProxyOverride") {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    raw.split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(not(windows))]
fn override_list() -> Vec<String> {
    Vec::new()
}

/// 这个主机是否命中系统代理的例外列表（命中的话浏览器本来就是直连的）
pub fn bypassed(host: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    override_list().iter().any(|p| matches_pattern(p, &host))
}

fn matches_pattern(pat: &str, host: &str) -> bool {
    let p = pat.trim().to_ascii_lowercase();
    if p.is_empty() {
        return false;
    }
    if p == "<local>" {
        return !host.contains('.') || host.ends_with(".local");
    }
    if let Some(rest) = p.strip_prefix("*.") {
        return host == rest || host.ends_with(&format!(".{rest}"));
    }
    if let Some(rest) = p.strip_prefix('.') {
        return host == rest || host.ends_with(&format!(".{rest}"));
    }
    if p.contains('*') {
        return wildcard(&p, host);
    }
    host == p
}

/// 只处理 '*' 的简单通配，够 ProxyOverride（127.* / *.github.com）使用
fn wildcard(pat: &str, text: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match text[pos..].find(part) {
            Some(off) => {
                if i == 0 && off != 0 {
                    return false;
                }
                pos += off + part.len();
            }
            None => return false,
        }
    }
    match parts.last() {
        Some(last) if !last.is_empty() => text.ends_with(last),
        _ => true,
    }
}

/// 从 URL 里取出主机名（小写，不含端口）
pub fn host_of(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    let rest = rest.split('/').next().unwrap_or(rest);
    let rest = rest.rsplit('@').next().unwrap_or(rest);
    let host = rest.split(':').next().unwrap_or(rest);
    host.trim().to_ascii_lowercase()
}

/// 建客户端。`use_proxy = false` 时显式 no_proxy：连环境变量里的代理也不用，
/// 保证「直连」真的是直连。
pub fn client_with(
    timeout_secs: u64,
    ua: &str,
    use_proxy: bool,
) -> Result<reqwest::blocking::Client, String> {
    let mut b = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(timeout_secs.min(15)))
        .user_agent(ua);

    let mut proxy = None;
    if use_proxy {
        proxy = proxy_url();
    }
    match proxy {
        Some(p) => match reqwest::Proxy::all(&p) {
            Ok(px) => b = b.proxy(px),
            Err(_) => b = b.no_proxy(),
        },
        None => b = b.no_proxy(),
    }
    b.build().map_err(|e| format!("创建网络客户端失败：{e}"))
}

pub fn client(timeout_secs: u64, ua: &str) -> Result<reqwest::blocking::Client, String> {
    client_with(timeout_secs, ua, true)
}

static USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ANR-Studio/0.1";
pub static API_UA: &str = "ANR-Studio/0.1 (+https://github.com/apex-compute/ANR)";

/// 抓网页/图片时用的 UA（有些 CDN 会拦非浏览器 UA）
pub fn web_client(timeout_secs: u64) -> Result<reqwest::blocking::Client, String> {
    client(timeout_secs, USER_AGENT)
}

/// 调 GitHub API 时用的 UA
pub fn api_client(timeout_secs: u64) -> Result<reqwest::blocking::Client, String> {
    client(timeout_secs, API_UA)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_extraction() {
        assert_eq!(host_of("https://api.github.com/repos/x/y?z=1"), "api.github.com");
        assert_eq!(host_of("https://gh-proxy.com/https://api.github.com/a"), "gh-proxy.com");
        assert_eq!(host_of("https://user:pw@host.example:8443/x"), "host.example");
        assert_eq!(host_of("api.github.com/x"), "api.github.com");
    }

    #[test]
    fn override_patterns() {
        assert!(matches_pattern("*.github.com", "api.github.com"));
        assert!(matches_pattern("api.github.com", "api.github.com"));
        assert!(matches_pattern("127.*", "127.0.0.1"));
        assert!(matches_pattern(".corp.com", "a.corp.com"));
        assert!(!matches_pattern("*.github.com", "github.com.evil.net"));
        assert!(!matches_pattern("", "api.github.com"));
    }
}
