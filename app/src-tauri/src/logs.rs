use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::inspect;
use crate::paths as P;
use crate::sysinfo;

/// 单次增量读取的上限。日志可能几十 MB，界面只需要「新出现的部分」。
pub const MAX_CHUNK: usize = 256 * 1024;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LogChunk {
    pub text: String,
    /// 下一次应该带的 offset（字节）
    pub next: u64,
    pub size: u64,
    /// 文件被截断/轮转，或目标日志换了：前端应清空重来
    pub reset: bool,
    pub path: Option<String>,
}

/// 兼容 0.3.0 的整段读取（诊断/复制用），只取尾部若干行。
pub fn read_log(game: &Path, max_lines: usize) -> Result<String, String> {
    let p = inspect::newest_log(game).ok_or("这个游戏还没有 nr_fg 日志")?;
    let bytes = fs::read(&p).map_err(|e| format!("读取日志失败：{e}"))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

/// P1-5：从 offset 开始只读增量，单次最多 [`MAX_CHUNK`] 字节。
///
/// `offset = None` 表示首次进入页面：直接取文件尾部 256 KB（`reset = true`，
/// 前端整段替换），避免几十 MB 的日志从头读。
/// `expected_path` 是前端上一轮拿到的路径：游戏切换后 `nr_fg-*.log` 的文件名会变，
/// 这时即使 offset 还合法也必须 reset，否则会把新日志从中间截一段出来。
pub fn read_since(
    game: &Path,
    offset: Option<u64>,
    expected_path: Option<&str>,
) -> Result<LogChunk, String> {
    let path = inspect::newest_log(game).ok_or("这个游戏还没有 nr_fg 日志")?;
    let path_str = path.to_string_lossy().to_string();
    let size = fs::metadata(&path)
        .map_err(|e| format!("读取日志信息失败：{e}"))?
        .len();

    let path_changed = expected_path
        .filter(|p| !p.is_empty())
        .map(|p| p != path_str)
        .unwrap_or(false);
    let mut reset = offset.is_none() || path_changed;
    let mut start = match offset {
        None => size.saturating_sub(MAX_CHUNK as u64),
        Some(o) if o > size => {
            reset = true;
            0
        }
        Some(o) => o,
    };
    if start > size {
        start = size;
    }

    if start >= size {
        return Ok(LogChunk {
            text: String::new(),
            next: start,
            size,
            reset,
            path: Some(path_str),
        });
    }

    let mut f = fs::File::open(&path).map_err(|e| format!("读取日志失败：{e}"))?;
    f.seek(SeekFrom::Start(start))
        .map_err(|e| format!("读取日志失败：{e}"))?;
    let want = ((size - start) as usize).min(MAX_CHUNK);
    let mut buf = vec![0u8; want];
    let mut got = 0usize;
    while got < want {
        match f.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(format!("读取日志失败：{e}")),
        }
    }
    buf.truncate(got);

    // 不要把多字节字符劈成两半：退到最后一个完整字符，下一轮从那里继续
    let valid = match std::str::from_utf8(&buf) {
        Ok(_) => got,
        Err(e) => {
            if e.valid_up_to() == 0 && got > 0 {
                // 极端情况：一整块都落在字符中间。宁可多读一次也不能死循环
                got
            } else {
                e.valid_up_to()
            }
        }
    };
    let text = String::from_utf8_lossy(&buf[..valid]).to_string();
    start += valid as u64;

    Ok(LogChunk {
        text,
        next: start,
        size,
        reset,
        path: Some(path_str),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

fn add_bytes(
    zw: &mut zip::ZipWriter<File>,
    name: &str,
    bytes: &[u8],
    sanitized: bool,
    files: &mut Vec<serde_json::Value>,
) -> Result<(), String> {
    let opts = zip::write::SimpleFileOptions::default();
    zw.start_file(name.to_string(), opts)
        .map_err(|e| format!("写入诊断包失败：{e}"))?;
    zw.write_all(bytes)
        .map_err(|e| format!("写入诊断包失败：{e}"))?;
    files.push(serde_json::json!({
        "name": name,
        "bytes": bytes.len(),
        "sha256": sha256_hex(bytes),
        "sanitized": sanitized,
    }));
    Ok(())
}

fn add_file(
    zw: &mut zip::ZipWriter<File>,
    name: &str,
    path: &Path,
    files: &mut Vec<serde_json::Value>,
) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let raw = fs::read(path).map_err(|e| format!("读取文件失败：{e}"))?;
    // 文本类做脱敏（用户名路径 / SteamID64）；二进制原样，不乱改
    let text = String::from_utf8_lossy(&raw);
    let looks_text = path
        .extension()
        .map(|e| {
            let e = e.to_string_lossy().to_ascii_lowercase();
            matches!(e.as_str(), "ini" | "json" | "log" | "txt")
        })
        .unwrap_or(false);
    if looks_text {
        let clean = sysinfo::sanitize(&text);
        add_bytes(zw, name, clean.as_bytes(), true, files)
    } else {
        add_bytes(zw, name, &raw, false, files)
    }
}

fn manifest(
    tool: &str,
    kind: &str,
    game: Option<&str>,
    generated: &str,
    files: &[serde_json::Value],
) -> serde_json::Value {
    serde_json::json!({
        "tool": tool,
        "kind": kind,
        "generated": generated,
        "sanitizeRules": 1,
        "privacy": "用户名路径替换为 %USERPROFILE%，17 位 SteamID64 中间打码",
        "game": game,
        "files": files,
    })
}

fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

pub fn diag_bundle(
    data: &Path,
    game: &Path,
    name: &str,
    state_json: &str,
) -> Result<PathBuf, String> {
    let dir = data.join("diag");
    P::ensure_dir(&dir)?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let out = dir.join(format!("{}-diag-{ts}.zip", safe_name(name)));

    let exe = crate::scan::find_exe(game, None);
    let info = sysinfo::collect(exe.as_deref());

    let file = File::create(&out).map_err(|e| format!("创建诊断包失败：{e}"))?;
    let mut zw = zip::ZipWriter::new(file);
    let mut files: Vec<serde_json::Value> = Vec::new();

    if let Some(log) = inspect::newest_log(game) {
        add_file(&mut zw, "nr_fg.log", &log, &mut files)?;
    }
    add_file(&mut zw, "OptiScaler.log", &game.join("OptiScaler.log"), &mut files)?;
    add_file(&mut zw, "OptiScaler.ini", &inspect::ini_path(game), &mut files)?;
    add_file(&mut zw, "nr_fg.ini", &inspect::nrf_g_ini(game), &mut files)?;
    add_file(
        &mut zw,
        "nr_fg_install.json",
        &inspect::record_path(game),
        &mut files,
    )?;

    let mut text = String::new();
    text.push_str(&format!("generated: {}\n", P::now_iso()));
    text.push_str(&format!(
        "tool: ANR Studio {} (unofficial)\n",
        env!("CARGO_PKG_VERSION")
    ));
    text.push_str(&format!(
        "os: {} {}\n",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    text.push_str(&format!("game dir: {}\n", game.display()));
    text.push('\n');
    text.push_str(&sysinfo::render_text(&info));
    text.push_str("\n--- detected state ---\n");
    text.push_str(&format!("{}\n", state_json.trim_end()));

    let clean = sysinfo::sanitize(&text);
    add_bytes(&mut zw, "system.txt", clean.as_bytes(), true, &mut files)?;

    let m = manifest(
        &format!("ANR Studio {}", env!("CARGO_PKG_VERSION")),
        "game",
        Some(name),
        &P::now_iso(),
        &files,
    );
    let mtext = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?;
    let opts = zip::write::SimpleFileOptions::default();
    zw.start_file("manifest.json", opts)
        .map_err(|e| format!("写入诊断包失败：{e}"))?;
    zw.write_all(mtext.as_bytes())
        .map_err(|e| format!("写入诊断包失败：{e}"))?;
    zw.finish().map_err(|e| format!("写入诊断包失败：{e}"))?;

    Ok(out)
}

/// P0-4：崩溃诊断包 = 所有未处理的崩溃报告 + 系统信息 + manifest。
pub fn crash_bundle(data: &Path) -> Result<PathBuf, String> {
    let dir = data.join("diag");
    P::ensure_dir(&dir)?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let out = dir.join(format!("anr-studio-crash-{ts}.zip"));
    let info = sysinfo::collect(None);

    let file = File::create(&out).map_err(|e| format!("创建诊断包失败：{e}"))?;
    let mut zw = zip::ZipWriter::new(file);
    let mut files: Vec<serde_json::Value> = Vec::new();

    let crash_dir = crate::crash::crash_dir(data);
    if let Ok(rd) = fs::read_dir(&crash_dir) {
        for e in rd.flatten() {
            let p = e.path();
            let n = e.file_name().to_string_lossy().to_string();
            if n.starts_with("crash-") && n.ends_with(".txt") {
                add_file(&mut zw, &format!("crash/{n}"), &p, &mut files)?;
            }
        }
    }

    let mut text = String::new();
    text.push_str(&format!("generated: {}\n", P::now_iso()));
    text.push_str(&format!(
        "tool: ANR Studio {} (unofficial)\n",
        env!("CARGO_PKG_VERSION")
    ));
    text.push('\n');
    text.push_str(&sysinfo::render_text(&info));
    let clean = sysinfo::sanitize(&text);
    add_bytes(&mut zw, "system.txt", clean.as_bytes(), true, &mut files)?;

    let m = manifest(
        &format!("ANR Studio {}", env!("CARGO_PKG_VERSION")),
        "crash",
        None,
        &P::now_iso(),
        &files,
    );
    let mtext = serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?;
    let opts = zip::write::SimpleFileOptions::default();
    zw.start_file("manifest.json", opts)
        .map_err(|e| format!("写入诊断包失败：{e}"))?;
    zw.write_all(mtext.as_bytes())
        .map_err(|e| format!("写入诊断包失败：{e}"))?;
    zw.finish().map_err(|e| format!("写入诊断包失败：{e}"))?;

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game_with_log(dir: &Path) -> PathBuf {
        let log = dir.join("OptiScaler").join("custom").join("Apex Neural Rendering FG").join("dlssg");
        fs::create_dir_all(&log).unwrap();
        let p = log.join("nr_fg-1.log");
        fs::write(&p, b"[ok] line one\n").unwrap();
        p
    }

    #[test]
    fn reads_incremental_chunks() {
        let dir = P::test_dir("logs-inc");
        let log = game_with_log(&dir);
        fs::write(&log, b"aaaa\nbbbb\n").unwrap();

        let first = read_since(&dir, Some(0), None).unwrap();
        assert!(!first.reset);
        assert_eq!(first.text, "aaaa\nbbbb\n");
        assert_eq!(first.next, 10);
        assert_eq!(first.size, 10);

        // 没变化时不重复返回内容
        let same = read_since(&dir, Some(first.next), first.path.as_deref()).unwrap();
        assert!(same.text.is_empty());

        fs::OpenOptions::new()
            .append(true)
            .open(&log)
            .unwrap()
            .write_all(b"cccc\n")
            .unwrap();
        let next = read_since(&dir, Some(first.next), first.path.as_deref()).unwrap();
        assert_eq!(next.text, "cccc\n");
        assert_eq!(next.next, 15);
    }

    #[test]
    fn detects_truncation_as_reset() {
        let dir = P::test_dir("logs-reset");
        let log = game_with_log(&dir);
        fs::write(&log, b"0123456789").unwrap();
        let chunk = read_since(&dir, Some(10), None).unwrap();
        assert!(!chunk.reset);

        fs::write(&log, b"new").unwrap();
        let after = read_since(&dir, Some(10), chunk.path.as_deref()).unwrap();
        assert!(after.reset);
        assert_eq!(after.text, "new");
        assert_eq!(after.next, 3);
    }

    #[test]
    fn detects_log_path_change() {
        let dir = P::test_dir("logs-path");
        game_with_log(&dir);
        // 文件系统 mtime 有粒度，先让它稳定一下再写第二个日志，保证 newest_log 选到 nr_fg-2
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(
            dir.join("OptiScaler").join("custom").join("Apex Neural Rendering FG").join("dlssg").join("nr_fg-2.log"),
            b"two",
        )
        .unwrap();
        // nr_fg-2 后写，newest_log 会选它
        let chunk = read_since(&dir, Some(0), Some("C:\\old\\nr_fg-1.log")).unwrap();
        assert!(chunk.reset);
        assert_eq!(chunk.text, "two");
    }

    #[test]
    fn empty_or_missing_log_reports_clearly() {
        let dir = P::test_dir("logs-empty");
        assert!(read_since(&dir, Some(0), None).is_err());
    }

    #[test]
    fn does_not_split_multibyte_char() {
        let dir = P::test_dir("logs-utf8");
        let log = game_with_log(&dir);
        fs::write(&log, "一二三四".as_bytes()).unwrap();
        // 「一二」= 6 字节，第三次一定落在字符边界上
        let a = read_since(&dir, Some(6), None).unwrap();
        assert_eq!(a.text, "三四");
    }

    #[test]
    fn diag_bundle_sanitizes_text_and_manifest_matches_zip() {
        let data = P::test_dir("diag-data");
        let game = P::test_dir("diag-game");
        let dlssg = game
            .join("OptiScaler")
            .join("custom")
            .join("Apex Neural Rendering FG")
            .join("dlssg");
        fs::create_dir_all(&dlssg).unwrap();
        fs::write(
            dlssg.join("nr_fg-1.log"),
            "userpath C:\\Users\\SomeOne\\AppData\\Local\\x\nsteamid 76561198012345678\n",
        )
        .unwrap();
        fs::write(game.join("OptiScaler.ini"), "[FrameGen]\nEnabled=true\n").unwrap();

        let zip_path = diag_bundle(&data, &game, "Test Game", r#"{"state":"ok"}"#).unwrap();
        let file = File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();

        let mut system = String::new();
        archive
            .by_name("system.txt")
            .unwrap()
            .read_to_string(&mut system)
            .unwrap();
        assert!(!system.contains("SomeOne"), "system.txt 没脱敏：{system}");
        assert!(system.contains("%USERPROFILE%"), "{system}");

        let mut log = String::new();
        archive
            .by_name("nr_fg.log")
            .unwrap()
            .read_to_string(&mut log)
            .unwrap();
        assert!(!log.contains("SomeOne"), "日志没脱敏：{log}");
        assert!(!log.contains("76561198012345678"), "SteamID 没打码：{log}");

        let mut mtext = String::new();
        archive
            .by_name("manifest.json")
            .unwrap()
            .read_to_string(&mut mtext)
            .unwrap();
        let manifest: serde_json::Value = serde_json::from_str(&mtext).unwrap();
        let files = manifest["files"].as_array().unwrap();
        assert!(!files.is_empty());
        for item in files {
            let name = item["name"].as_str().unwrap();
            let mut bytes = Vec::new();
            archive.by_name(name).unwrap().read_to_end(&mut bytes).unwrap();
            assert_eq!(item["sha256"].as_str().unwrap(), sha256_hex(&bytes), "{name}");
            assert_eq!(item["bytes"].as_u64().unwrap(), bytes.len() as u64, "{name}");
        }
    }
}