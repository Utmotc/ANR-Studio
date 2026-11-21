use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::inspect;
use crate::paths as P;

pub fn read_log(game: &Path, max_lines: usize) -> Result<String, String> {
    let p = inspect::newest_log(game).ok_or("这个游戏还没有 nr_fg 日志")?;
    let bytes = fs::read(&p).map_err(|e| format!("读取日志失败：{e}"))?;
    let text = String::from_utf8_lossy(&bytes).to_string();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    Ok(lines[start..].join("\n"))
}

fn add_file(zw: &mut zip::ZipWriter<File>, name: &str, path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    let opts = zip::write::SimpleFileOptions::default();
    zw.start_file(name.to_string(), opts)
        .map_err(|e| format!("写入诊断包失败：{e}"))?;
    let bytes = fs::read(path).map_err(|e| format!("读取文件失败：{e}"))?;
    zw.write_all(&bytes)
        .map_err(|e| format!("写入诊断包失败：{e}"))?;
    Ok(())
}

pub fn diag_bundle(data: &Path, game: &Path, name: &str, state_json: &str) -> Result<PathBuf, String> {
    let dir = data.join("diag");
    P::ensure_dir(&dir)?;
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let safe: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let out = dir.join(format!("{safe}-diag-{ts}.zip"));

    let file = File::create(&out).map_err(|e| format!("创建诊断包失败：{e}"))?;
    let mut zw = zip::ZipWriter::new(file);

    if let Some(log) = inspect::newest_log(game) {
        add_file(&mut zw, "nr_fg.log", &log)?;
    }
    add_file(&mut zw, "OptiScaler.log", &game.join("OptiScaler.log"))?;
    add_file(&mut zw, "OptiScaler.ini", &inspect::ini_path(game))?;
    add_file(&mut zw, "nr_fg.ini", &inspect::nrf_g_ini(game))?;
    add_file(&mut zw, "nr_fg_install.json", &inspect::record_path(game))?;

    let mut info = String::new();
    info.push_str(&format!("generated: {}\n", P::now_iso()));
    info.push_str(&format!(
        "tool: ANR Studio {} (unofficial)\n",
        env!("CARGO_PKG_VERSION")
    ));
    info.push_str(&format!("os: {} {}\n", std::env::consts::OS, std::env::consts::ARCH));
    info.push_str(&format!("game dir: {}\n", game.display()));
    info.push_str("\n--- detected state ---\n");
    info.push_str(state_json);
    info.push('\n');

    let opts = zip::write::SimpleFileOptions::default();
    zw.start_file("system.txt", opts)
        .map_err(|e| format!("写入诊断包失败：{e}"))?;
    zw.write_all(info.as_bytes())
        .map_err(|e| format!("写入诊断包失败：{e}"))?;
    zw.finish().map_err(|e| format!("写入诊断包失败：{e}"))?;

    Ok(out)
}
