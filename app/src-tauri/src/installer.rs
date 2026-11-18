use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ini::IniDoc;
use crate::inspect;
use crate::paths as P;
use crate::releases;
use crate::scan::hidden;

/// 下载 zip / 校验表用的客户端。
///
/// 和版本列表一样听「下载源」：直连和镜像都不吃系统代理，只有自动模式才跟随代理。
/// 否则会出现「列表走直连、zip 却还在过代理」的两套线路，用户按设置根本对不上。
fn dl_client(mirror: &str) -> Result<reqwest::blocking::Client, String> {
    let use_proxy = mirror != "github" && mirror != "mirror";
    crate::net::client_with(600, "ANR-Studio/0.1", use_proxy)
}

fn part_path(dest: &Path) -> PathBuf {
    let mut s = dest.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

/// 下载到 `<dest>.part`，支持 HTTP Range 续传；失败/取消时保留 .part，
/// 下一次重试从断点继续。完整下载后才原子改名为最终 zip。
fn download_to(
    url: &str,
    dest: &Path,
    total_hint: u64,
    mirror: &str,
    mut cb: impl FnMut(f64),
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    let client = dl_client(mirror)?;
    let part = part_path(dest);
    let mut offset = fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    if total_hint > 0 && offset == total_hint {
        // 上次已下完但没来得及改名的残留，直接复用；后面仍会强制做 SHA256 校验
        fs::rename(&part, dest).map_err(|e| format!("保存安装包失败：{e}"))?;
        return Ok(());
    }
    if total_hint > 0 && offset > total_hint {
        let _ = fs::remove_file(&part);
        offset = 0;
    }

    let mut req = client.get(url);
    if offset > 0 {
        req = req.header(reqwest::header::RANGE, format!("bytes={offset}-"));
    }
    let mut resp = req.send().map_err(|e| format!("下载失败：{e}"))?;
    let code = resp.status().as_u16();
    let append = code == 206 && offset > 0;
    if code == 200 {
        // 服务器不支持 Range，忽略已有进度从头写
        offset = 0;
    } else if !append {
        if code == 416 {
            let _ = fs::remove_file(&part);
        }
        return Err(format!("下载失败：HTTP {code}"));
    }

    let remaining = resp.content_length().unwrap_or(0);
    let total = if append {
        offset + remaining
    } else if total_hint > 0 {
        total_hint
    } else {
        remaining
    };

    let file = if append {
        fs::OpenOptions::new().append(true).open(&part)
    } else {
        File::create(&part)
    }
    .map_err(|e| format!("创建下载文件失败 {}：{e}", part.display()))?;
    let mut out = BufWriter::new(file);
    let mut buf = [0u8; 128 * 1024];
    let mut got = offset;
    loop {
        if cancel() {
            drop(out);
            return Err("已取消（下载进度已保留，重试会从断点继续）".into());
        }
        let n = resp.read(&mut buf).map_err(|e| format!("读取下载流失败：{e}"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| format!("写入失败：{e}"))?;
        got += n as u64;
        if total > 0 {
            cb((got as f64 / total as f64).min(1.0));
        }
    }
    out.flush().map_err(|e| format!("写入失败：{e}"))?;
    drop(out);
    if total > 0 && got < total {
        return Err(format!(
            "下载不完整：{got}/{total} 字节。已保留 {:.1} MB 进度，重试会自动续传",
            got as f64 / 1_048_576.0
        ));
    }
    fs::rename(&part, dest).map_err(|e| format!("保存安装包失败：{e}"))?;
    Ok(())
}

fn download_text(url: &str, mirror: &str) -> Result<String, String> {
    let resp = dl_client(mirror)?
        .get(url)
        .send()
        .map_err(|e| format!("下载失败：{e}"))?;
    resp.text().map_err(|e| format!("读取文本失败：{e}"))
}

fn parse_sum(text: &str, file: &str) -> Option<String> {
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(hash), Some(name)) = (it.next(), it.next()) else {
            continue;
        };
        if name.trim_start_matches('*').eq_ignore_ascii_case(file) {
            return Some(hash.to_string());
        }
    }
    None
}

fn unzip(zip_path: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("打开安装包失败：{e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("安装包损坏：{e}"))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取安装包内文件失败：{e}"))?;
        let Some(rel) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        let out_path = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|e| format!("创建目录失败：{e}"))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
            }
            let mut f = File::create(&out_path).map_err(|e| format!("写入失败：{e}"))?;
            std::io::copy(&mut entry, &mut f).map_err(|e| format!("解压失败：{e}"))?;
        }
    }
    Ok(())
}

/// 同一时间只允许一个下载/解压任务，避免两次安装抢同一个 zip
static PKG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 确保安装包已下载并解压到缓存，返回含 install.ps1 的目录。
pub fn ensure_package(
    cache: &Path,
    tag: &str,
    kind: &str,
    mirror: &str,
    mut report: impl FnMut(&str, f64),
    cancel: &dyn Fn() -> bool,
) -> Result<PathBuf, String> {
    let _guard = PKG_LOCK.lock().unwrap();
    let dest = cache.join(tag).join(kind);
    if dest.join(".ok").exists() {
        return Ok(dest);
    }

    if cancel() {
        return Err("已取消".into());
    }
    report("获取版本信息", 0.02);
    let rels = releases::list_releases(mirror)?;
    let rel = rels
        .iter()
        .find(|r| r.tag.eq_ignore_ascii_case(tag))
        .ok_or_else(|| format!("找不到版本 {tag}"))?;
    let asset = releases::find_asset(rel, kind)
        .ok_or_else(|| format!("版本 {tag} 没有 {kind} 包"))?;

    // R6：必须拿到官方 SHA256 才能装，不允许「校验和取不到就放行」
    let sums = releases::sums_asset(rel)
        .ok_or_else(|| format!("版本 {tag} 没有提供 SHA256SUMS.txt，按安全策略拒绝安装"))?;
    let sums_text = download_text(&releases::mirror_url(&sums.url, mirror), mirror).map_err(|e| {
        format!("下载 SHA256SUMS.txt 失败：{e}；按安全策略拒绝安装，可切换下载源或手动导入本地包")
    })?;
    let expected = parse_sum(&sums_text, &asset.name).ok_or_else(|| {
        format!(
            "SHA256SUMS.txt 里没有 {} 的记录，按安全策略拒绝安装",
            asset.name
        )
    })?;

    P::ensure_dir(&cache.join(tag))?;
    let zip_path = cache.join(tag).join(format!("{kind}.zip"));
    // 已有 zip（或上次下载的残留）先走校验，通过就不必重下
    if !zip_path.exists() {
        download_to(
            &releases::mirror_url(&asset.url, mirror),
            &zip_path,
            asset.size,
            mirror,
            |p| report("下载安装包", 0.05 + p * 0.82),
            cancel,
        )
        .map_err(|e| format!("{e}（可以在设置里切换下载源再试）"))?;
    }

    report("校验 SHA256", 0.9);
    let got = inspect::sha256_file(&zip_path).ok_or("读取安装包失败")?;
    if !got.eq_ignore_ascii_case(&expected) {
        let _ = fs::remove_file(&zip_path);
        return Err(format!("安装包校验失败，已删除。期望 {expected}，实际 {got}"));
    }

    report("解压安装包", 0.95);
    if dest.exists() {
        let _ = fs::remove_dir_all(&dest);
    }
    P::ensure_dir(&dest)?;
    unzip(&zip_path, &dest)?;
    if !dest.join("install.ps1").exists() {
        return Err("安装包里没有 install.ps1，可能官方包结构已变化".into());
    }
    let _ = fs::write(dest.join(".ok"), b"ok");
    Ok(dest)
}

/// R5：网络实在拉不动时手动导入本地 zip。
/// R6 同样适用：调用方必须给出官方 SHA256，校验不过直接拒绝，不提供「跳过」。
pub fn import_local(
    zip_path: &Path,
    cache: &Path,
    tag: &str,
    kind: &str,
    expected: &str,
) -> Result<PathBuf, String> {
    let _guard = PKG_LOCK.lock().unwrap();
    if !zip_path.is_file() {
        return Err("选择的安装包不存在".into());
    }
    let expect = expected.trim();
    if expect.len() != 64 || !expect.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("请填写官方 SHA256SUMS.txt 里 64 位的 SHA256 值".into());
    }
    let got = inspect::sha256_file(zip_path).ok_or("读取安装包失败")?;
    if !got.eq_ignore_ascii_case(expect) {
        return Err(format!("校验失败，已拒绝导入。期望 {expect}，实际 {got}"));
    }

    let dest = cache.join(tag).join(kind);
    P::ensure_dir(&cache.join(tag))?;
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| format!("清理旧缓存失败：{e}"))?;
    }
    P::ensure_dir(&dest)?;
    unzip(zip_path, &dest)?;
    if !dest.join("install.ps1").exists() {
        let _ = fs::remove_dir_all(&dest);
        return Err("安装包里没有 install.ps1，可能官方包结构已变更，请更新本工具".into());
    }
    fs::write(dest.join(".ok"), b"ok").map_err(|e| format!("写入缓存标记失败：{e}"))?;
    Ok(dest)
}

/// R8：启动时检查已缓存的包结构，返回失效的包（tag/kind）列表。
pub fn check_cached_packages(cache: &Path) -> Vec<String> {
    let mut bad = Vec::new();
    let Ok(tags) = fs::read_dir(cache) else {
        return bad;
    };
    for tag in tags.flatten() {
        let tag_dir = tag.path();
        if !tag_dir.is_dir() {
            continue;
        }
        let Ok(kinds) = fs::read_dir(&tag_dir) else {
            continue;
        };
        for k in kinds.flatten() {
            let d = k.path();
            if d.is_dir() && d.join(".ok").exists() && !d.join("install.ps1").exists() {
                bad.push(format!(
                    "{}/{}",
                    tag.file_name().to_string_lossy(),
                    k.file_name().to_string_lossy()
                ));
            }
        }
    }
    bad
}

/// 从官方 release 的 SHA256SUMS.txt 里查某个包的校验值（供手动导入自动填充）。
/// 网络不通时返回 Err，由调用方要求用户手工填入，绝不静默跳过校验。
pub fn official_sum(tag: &str, filename: &str, mirror: &str) -> Result<String, String> {
    let rels = releases::list_releases(mirror)?;
    let rel = rels
        .iter()
        .find(|r| r.tag.eq_ignore_ascii_case(tag))
        .ok_or_else(|| format!("GitHub 上没有版本 {tag}"))?;
    let sums = releases::sums_asset(rel)
        .ok_or_else(|| format!("版本 {tag} 没有提供 SHA256SUMS.txt"))?;
    let text = download_text(&releases::mirror_url(&sums.url, mirror), mirror)?;
    parse_sum(&text, filename)
        .ok_or_else(|| format!("SHA256SUMS.txt 里没有 {filename} 的记录"))
}

fn extract_error(stderr: &str, stdout: &str) -> String {
    for line in stderr.lines() {
        if let Some(idx) = line.find("FullyQualifiedErrorId") {
            let rest = &line[idx..];
            if let Some((_, msg)) = rest.split_once(':') {
                let msg = msg.trim();
                if !msg.is_empty() {
                    return msg.to_string();
                }
            }
        }
    }
    let fallback = stderr
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("At ") && !l.starts_with('+'))
        .next_back()
        .unwrap_or("");
    if fallback.is_empty() {
        let s = stdout.trim();
        if s.is_empty() {
            "安装器没有返回信息".into()
        } else {
            s.lines().last().unwrap_or(s).to_string()
        }
    } else {
        fallback.to_string()
    }
}

#[derive(Debug)]
pub struct ScriptError {
    pub message: String,
    pub output: String,
}

pub fn run_install(pkg: &Path, game: &Path, proxy: Option<&str>) -> Result<String, ScriptError> {
    let script = pkg.join("install.ps1");
    if !script.exists() {
        return Err(ScriptError {
            message: "安装包里没有 install.ps1".into(),
            output: String::new(),
        });
    }
    let mut cmd = Command::new("powershell");
    cmd.arg("-NoProfile")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&script)
        .arg(game);
    if let Some(p) = proxy {
        cmd.arg(p);
    }
    hidden(&mut cmd);
    let out = cmd
        .output()
        .map_err(|e| ScriptError {
            message: format!("启动安装器失败：{e}"),
            output: String::new(),
        })?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        let message = extract_error(&stderr, &stdout);
        let raw = if stderr.trim().is_empty() {
            stdout
        } else {
            format!("{stdout}\n{stderr}")
        };
        return Err(ScriptError {
            message,
            output: raw.trim().to_string(),
        });
    }
    Ok(stdout)
}

/// 卸载：全量安装按官方逻辑还原并删除 OptiScaler 目录；
/// 插件安装只移除 provider 并把 OptiScaler.ini 恢复成安装前的值（不碰用户的 OptiScaler）。
pub fn uninstall(game: &Path) -> Result<(), String> {
    let rec_text = fs::read_to_string(inspect::record_path(game))
        .map_err(|_| "这个目录没有本工具的安装记录，未做任何改动".to_string())?;
    let rec: serde_json::Value =
        serde_json::from_str(&rec_text).map_err(|e| format!("安装记录损坏：{e}"))?;

    let is_addon = rec.get("proxy").is_none();

    if is_addon {
        let ini_p = inspect::ini_path(game);
        if ini_p.exists() {
            let mut doc = IniDoc::load(&ini_p);
            let before = rec.get("before").cloned().unwrap_or_default();
            let fg_in = before.get("FGInput").and_then(|x| x.as_str()).unwrap_or("auto");
            let rep = before
                .get("FGNvngxReplacement")
                .and_then(|x| x.as_str())
                .unwrap_or("auto");
            IniDoc::backup_once(&ini_p);
            doc.set("FrameGen", "FGInput", fg_in);
            doc.set("FrameGen", "FGNvngxReplacement", rep);
            doc.save(&ini_p).map_err(|e| format!("写入 OptiScaler.ini 失败：{e}"))?;
        }
        let provider = inspect::provider_dir(game);
        if provider.exists() {
            fs::remove_dir_all(&provider).map_err(|e| format!("删除 provider 失败：{e}"))?;
        }
        return Ok(());
    }

    let proxy = rec
        .get("proxy")
        .and_then(|x| x.as_str())
        .unwrap_or("dxgi.dll")
        .to_string();
    let originals = rec
        .get("originals")
        .and_then(|x| x.as_object())
        .cloned()
        .unwrap_or_default();
    let backup_dir = inspect::opti_dir(game).join("nr_fg_backup");

    for f in [proxy.as_str(), "OptiScaler.ini"] {
        let current = game.join(f);
        let backup = backup_dir.join(f);
        match originals.get(f).and_then(|x| x.as_str()) {
            Some(orig) => {
                let got = inspect::sha256_file(&backup)
                    .ok_or_else(|| format!("备份文件缺失：{}", backup.display()))?;
                if !got.eq_ignore_ascii_case(orig) {
                    return Err(format!(
                        "备份文件校验不符，已停止（未做任何改动）：{}",
                        backup.display()
                    ));
                }
                fs::copy(&backup, &current).map_err(|e| format!("还原 {f} 失败：{e}"))?;
            }
            None => {
                if current.exists() {
                    let _ = fs::remove_file(&current);
                }
            }
        }
    }

    let _ = fs::remove_file(game.join("OptiScaler.log"));
    // 我们自己产生的 ini 备份：全量卸载后 ini 也已删除，备份没有留存价值
    let _ = fs::remove_file(inspect::ini_path(game).with_extension("ini.anr-bak"));
    fs::remove_dir_all(inspect::opti_dir(game))
        .map_err(|e| format!("删除 OptiScaler 目录失败：{e}"))?;
    Ok(())
}

fn add_dir_to_zip(
    zw: &mut zip::ZipWriter<File>,
    src: &Path,
    prefix: &Path,
    opts: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    let rd = fs::read_dir(src).map_err(|e| format!("读取目录失败：{e}"))?;
    for entry in rd.flatten() {
        let p = entry.path();
        let rel = prefix.join(entry.file_name());
        let name = rel.to_string_lossy().replace('\\', "/");
        if p.is_dir() {
            add_dir_to_zip(zw, &p, &rel, opts)?;
        } else {
            zw.start_file(name, opts).map_err(|e| format!("写入快照失败：{e}"))?;
            let mut f = File::open(&p).map_err(|e| format!("读取文件失败：{e}"))?;
            std::io::copy(&mut f, zw).map_err(|e| format!("写入快照失败：{e}"))?;
        }
    }
    Ok(())
}


fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

fn read_snapshot_meta(zip_path: &Path) -> Option<serde_json::Value> {
    let file = File::open(zip_path).ok()?;
    let mut ar = zip::ZipArchive::new(file).ok()?;
    let mut entry = ar.by_name("snapshot.json").ok()?;
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    serde_json::from_str(&text).ok()
}

fn record_proxy(game: &Path) -> Option<String> {
    let text = fs::read_to_string(inspect::record_path(game)).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("proxy").and_then(|x| x.as_str()).map(|s| s.to_string())
}

/// 把快照 zip 解回游戏目录。
///
/// 新快照（schema 2）除了 `OptiScaler/`，还包含代理 DLL 和 `OptiScaler.ini`；
/// 旧快照只有 `OptiScaler/`，仍然可以恢复。只允许写这三个白名单根文件，
/// 防止被篡改的 zip 往游戏目录里塞任意文件。
pub fn restore_snapshot(zip_path: &Path, game: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("打开快照失败：{e}"))?;
    let mut ar = zip::ZipArchive::new(file).map_err(|e| format!("快照损坏：{e}"))?;

    // 先清掉现在的 OptiScaler，避免回滚后残留半成品文件
    let opti = inspect::opti_dir(game);
    if opti.exists() {
        let _ = fs::remove_dir_all(&opti);
    }

    for i in 0..ar.len() {
        let mut entry = ar.by_index(i).map_err(|e| format!("读取快照失败：{e}"))?;
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str == "snapshot.json" {
            continue;
        }
        let root_allowed = rel_str == "OptiScaler.ini"
            || matches!(rel_str.as_str(), "dxgi.dll" | "winmm.dll" | "version.dll");
        if !root_allowed && !rel_str.starts_with("OptiScaler/") {
            continue;
        }
        let dest = game.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&dest).map_err(|e| format!("创建目录失败：{e}"))?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建目录失败：{e}"))?;
        }
        let mut out = File::create(&dest).map_err(|e| format!("写入失败：{e}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| format!("写入失败：{e}"))?;
    }
    Ok(())
}

/// schema 2 的完整快照：`OptiScaler/` + 代理 DLL + `OptiScaler.ini`。
/// 这样卸载后恢复时，不会只剩 provider 而缺少代理 DLL / ini。
pub fn snapshot(data: &Path, game: &Path, name: &str) -> Result<PathBuf, String> {
    let dir = P::snapshots_dir(data);
    P::ensure_dir(&dir)?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let safe = safe_name(name);
    let mut out = dir.join(format!("{safe}-{ts}.zip"));
    let mut n = 2;
    while out.exists() {
        out = dir.join(format!("{safe}-{ts}-{n}.zip"));
        n += 1;
    }

    let file = File::create(&out).map_err(|e| format!("创建快照失败：{e}"))?;
    let mut zw = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let proxy = record_proxy(game);
    let meta = serde_json::json!({
        "schema": 2,
        "gameId": inspect::fnv_id(&game.to_string_lossy()),
        "gameDir": game.to_string_lossy(),
        "gameName": name,
        "proxy": proxy,
        "createdAt": P::now_iso(),
    });
    zw.start_file("snapshot.json", opts)
        .map_err(|e| format!("写入快照失败：{e}"))?;
    zw.write_all(serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?.as_bytes())
        .map_err(|e| format!("写入快照失败：{e}"))?;

    let ini = inspect::ini_path(game);
    if ini.is_file() {
        add_snapshot_file(&mut zw, "OptiScaler.ini", &ini, opts)?;
    }
    if let Some(p) = &proxy {
        let dll = game.join(p);
        if dll.is_file() {
            add_snapshot_file(&mut zw, p, &dll, opts)?;
        }
    }
    let opti = inspect::opti_dir(game);
    if opti.is_dir() {
        add_dir_to_zip(&mut zw, &opti, Path::new("OptiScaler"), opts)?;
    }
    zw.finish().map_err(|e| format!("写入快照失败：{e}"))?;
    Ok(out)
}

fn add_snapshot_file(
    zw: &mut zip::ZipWriter<File>,
    name: &str,
    path: &Path,
    opts: zip::write::SimpleFileOptions,
) -> Result<(), String> {
    zw.start_file(name.to_string(), opts)
        .map_err(|e| format!("写入快照失败：{e}"))?;
    let mut f = File::open(path).map_err(|e| format!("读取文件失败：{e}"))?;
    std::io::copy(&mut f, zw).map_err(|e| format!("写入快照失败：{e}"))?;
    Ok(())
}

/// 校验快照确实属于这个游戏：新快照看 `gameId`，旧快照只能按文件名前缀兜底。
pub fn validate_snapshot_for_game(
    zip_path: &Path,
    game_id: &str,
    game_name: &str,
) -> Result<(), String> {
    if !zip_path.is_file() {
        return Err("快照文件不存在".into());
    }
    if !zip_path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
    {
        return Err("只支持恢复 .zip 快照".into());
    }
    match read_snapshot_meta(zip_path) {
        Some(meta) => {
            let id = meta.get("gameId").and_then(|x| x.as_str()).unwrap_or("");
            if !id.is_empty() && id != game_id {
                return Err("这个快照属于另一个游戏，已拒绝恢复".into());
            }
        }
        None => {
            let file_name = zip_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !file_name.starts_with(&format!("{}-", safe_name(game_name))) {
                return Err("这个快照不属于这个游戏（旧版快照只能按游戏名匹配）".into());
            }
        }
    }
    Ok(())
}

pub fn list_snapshots(
    data: &Path,
    game_id: &str,
    game_name: &str,
) -> Result<Vec<crate::model::SnapshotInfo>, String> {
    let dir = P::snapshots_dir(data);
    let Ok(rd) = fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let prefix = format!("{}-", safe_name(game_name));
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_file()
            || !p
                .extension()
                .map(|x| x.eq_ignore_ascii_case("zip"))
                .unwrap_or(false)
        {
            continue;
        }
        let file_name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let meta = read_snapshot_meta(&p);
        let meta_id = meta
            .as_ref()
            .and_then(|m| m.get("gameId"))
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let belongs = if !meta_id.is_empty() {
            meta_id == game_id
        } else {
            file_name.starts_with(&prefix)
        };
        if !belongs {
            continue;
        }
        let schema = meta
            .as_ref()
            .and_then(|m| m.get("schema"))
            .and_then(|x| x.as_u64())
            .unwrap_or(0) as u32;
        let created_at = meta
            .as_ref()
            .and_then(|m| m.get("createdAt"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| P::file_mtime_iso(&p));
        let game_name = meta
            .as_ref()
            .and_then(|m| m.get("gameName"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        out.push(crate::model::SnapshotInfo {
            path: p.to_string_lossy().to_string(),
            file_name,
            created_at,
            size_bytes: fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
            game_id: meta_id.to_string(),
            game_name,
            schema,
        });
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_part_is_promoted_without_network() {
        let dir = std::env::temp_dir().join(format!("anr-dl-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("full.zip");
        let part = part_path(&dest);
        fs::write(&part, b"0123456789").unwrap();
        // part 已经等于预期长度时不应再发请求，直接复用并由外层做 SHA256 校验
        let r = download_to("http://127.0.0.1:1/never-used", &dest, 10, "auto", |_| {}, &|| false);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(fs::read(&dest).unwrap(), b"0123456789");
        assert!(!part.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_sum_matches_asset_name() {
        let txt = "aaa  anr-fg-V.B18-full.zip\nbbb *anr-fg-V.B18-addon.zip\n";
        assert_eq!(parse_sum(txt, "anr-fg-V.B18-full.zip").as_deref(), Some("aaa"));
        assert_eq!(parse_sum(txt, "anr-fg-V.B18-addon.zip").as_deref(), Some("bbb"));
        assert_eq!(parse_sum(txt, "missing.zip"), None);
    }

    #[test]
    fn snapshot_round_trips_proxy_and_ini() {
        let root = std::env::temp_dir().join(format!("anr-snap-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let data = root.join("data");
        let game = root.join("game");
        let provider = inspect::provider_dir(&game).join("dlssg");
        fs::create_dir_all(&provider).unwrap();
        fs::write(inspect::record_path(&game), r#"{"proxy":"dxgi.dll"}"#).unwrap();
        fs::write(game.join("dxgi.dll"), b"anr-proxy").unwrap();
        fs::write(inspect::ini_path(&game), b"[FrameGen]\nEnabled=true\n").unwrap();
        fs::write(provider.join("nvngx_dlssg.dll"), b"provider").unwrap();

        let zip = snapshot(&data, &game, "Test Game").unwrap();
        let game_id = inspect::fnv_id(&game.to_string_lossy());
        assert!(zip.is_file());
        assert_eq!(list_snapshots(&data, &game_id, "Test Game").unwrap().len(), 1);
        assert!(list_snapshots(&data, "not-the-id", "Test Game").unwrap().is_empty());
        validate_snapshot_for_game(&zip, &game_id, "Test Game").unwrap();

        // 模拟卸载完成：根目录的代理/ini 与 OptiScaler 目录都被清掉
        fs::remove_file(game.join("dxgi.dll")).unwrap();
        fs::remove_file(inspect::ini_path(&game)).unwrap();
        fs::remove_dir_all(inspect::opti_dir(&game)).unwrap();
        restore_snapshot(&zip, &game).unwrap();

        assert_eq!(fs::read(game.join("dxgi.dll")).unwrap(), b"anr-proxy");
        assert_eq!(
            fs::read(inspect::ini_path(&game)).unwrap(),
            b"[FrameGen]\nEnabled=true\n"
        );
        assert!(inspect::provider_dll(&game).is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn old_snapshot_without_metadata_is_not_restored_for_other_game() {
        let root = std::env::temp_dir().join(format!("anr-snap-old-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let zip = root.join("Other Game-20260101-000000.zip");
        let file = File::create(&zip).unwrap();
        let mut zw = zip::ZipWriter::new(file);
        zw.start_file("OptiScaler/keep.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zw.write_all(b"x").unwrap();
        zw.finish().unwrap();

        assert!(validate_snapshot_for_game(&zip, "unused", "Test Game").is_err());
        let _ = fs::remove_dir_all(&root);
    }
}
