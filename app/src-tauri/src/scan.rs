use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths::file_mtime_iso;

pub struct RawGame {
    pub name: String,
    pub source: String,
    pub dir: PathBuf,
    pub exe: String,
    pub last_used: String,
}

#[cfg(windows)]
pub fn hidden(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn hidden(_cmd: &mut Command) {}

fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    for c in text.chars() {
        match c {
            '"' => {
                if in_str {
                    out.push(std::mem::take(&mut cur));
                }
                in_str = !in_str;
            }
            _ if in_str => cur.push(c),
            _ => {}
        }
    }
    out
}

fn after(tokens: &[String], key: &str) -> Option<String> {
    tokens
        .iter()
        .position(|t| t.eq_ignore_ascii_case(key))
        .and_then(|i| tokens.get(i + 1).cloned())
}

fn steam_root() -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(k) = hkcu.open_subkey("Software\\Valve\\Steam") {
        if let Ok(p) = k.get_value::<String, _>("SteamPath") {
            return Some(PathBuf::from(p.replace('/', "\\")));
        }
    }
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for key in ["SOFTWARE\\WOW6432Node\\Valve\\Steam", "SOFTWARE\\Valve\\Steam"] {
        if let Ok(k) = hklm.open_subkey(key) {
            if let Ok(p) = k.get_value::<String, _>("InstallPath") {
                return Some(PathBuf::from(p.replace('/', "\\")));
            }
        }
    }
    None
}

fn steam_libraries(root: &Path) -> Vec<PathBuf> {
    let mut out = vec![root.join("steamapps")];
    let vdf = root.join("steamapps").join("libraryfolders.vdf");
    if let Ok(text) = fs::read_to_string(&vdf) {
        let t = tokens(&text);
        for (i, tok) in t.iter().enumerate() {
            if tok.eq_ignore_ascii_case("path") {
                if let Some(p) = t.get(i + 1) {
                    let lib = PathBuf::from(p.replace("\\\\", "\\"));
                    let sa = lib.join("steamapps");
                    if sa.is_dir() && !out.iter().any(|x| x == &sa) {
                        out.push(sa);
                    }
                }
            }
        }
    }
    out
}

fn steam_games() -> Vec<RawGame> {
    let Some(root) = steam_root() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for lib in steam_libraries(&root) {
        let Ok(rd) = fs::read_dir(&lib) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            let is_acf = p
                .extension()
                .map(|e| e.eq_ignore_ascii_case("acf"))
                .unwrap_or(false)
                && p.file_name()
                    .map(|n| n.to_string_lossy().starts_with("appmanifest_"))
                    .unwrap_or(false);
            if !is_acf {
                continue;
            }
            let Ok(text) = fs::read_to_string(&p) else { continue };
            let t = tokens(&text);
            let name = after(&t, "name").unwrap_or_default();
            let installdir = after(&t, "installdir").unwrap_or_default();
            if name.is_empty() || installdir.is_empty() {
                continue;
            }
            let dir = lib.join("common").join(&installdir);
            if !dir.is_dir() {
                continue;
            }
            let Some(exe) = find_exe(&dir, None) else { continue };
            out.push(RawGame {
                name,
                source: "steam".into(),
                dir,
                exe: exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                last_used: file_mtime_iso(&p),
            });
        }
    }
    out
}

fn epic_games() -> Vec<RawGame> {
    let base = PathBuf::from(r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests");
    let Ok(rd) = fs::read_dir(&base) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in rd.flatten() {
        let p = entry.path();
        if !p.extension().map(|e| e.eq_ignore_ascii_case("item")).unwrap_or(false) {
            continue;
        }
        let Ok(text) = fs::read_to_string(&p) else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let name = v.get("DisplayName").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let loc = v.get("InstallLocation").and_then(|x| x.as_str()).unwrap_or("");
        let launch = v.get("LaunchExecutable").and_then(|x| x.as_str()).unwrap_or("");
        if name.is_empty() || loc.is_empty() {
            continue;
        }
        let dir = PathBuf::from(loc);
        if !dir.is_dir() {
            continue;
        }
        let hint = if launch.is_empty() { None } else { Some(dir.join(launch)) };
        let Some(exe) = find_exe(&dir, hint.as_deref()) else { continue };
        out.push(RawGame {
            name,
            source: "epic".into(),
            dir,
            exe: exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            last_used: file_mtime_iso(&p),
        });
    }
    out
}

fn gog_games() -> Vec<RawGame> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut out = Vec::new();
    for root in ["SOFTWARE\\WOW6432Node\\GOG.com\\Games", "SOFTWARE\\GOG.com\\Games"] {
        let Ok(games) = hklm.open_subkey(root) else { continue };
        for id in games.enum_keys().flatten() {
            let Ok(k) = games.open_subkey(&id) else { continue };
            let name = k.get_value::<String, _>("gameName").unwrap_or_default();
            let path = k.get_value::<String, _>("path").unwrap_or_default();
            let exe = k.get_value::<String, _>("exe").unwrap_or_default();
            if name.is_empty() || path.is_empty() {
                continue;
            }
            let dir = PathBuf::from(&path);
            if !dir.is_dir() {
                continue;
            }
            let hint = if exe.is_empty() { None } else { Some(dir.join(&exe)) };
            let Some(found) = find_exe(&dir, hint.as_deref()) else { continue };
            out.push(RawGame {
                name,
                source: "gog".into(),
                last_used: file_mtime_iso(&dir),
                dir,
                exe: found.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            });
        }
    }
    out
}

const EXE_NOISE: &[&str] = &[
    "unins",
    "vcredist",
    "vc_redist",
    "dxsetup",
    "dxwebsetup",
    "dotnet",
    "crashhandler",
    "crashreport",
    "easyanticheat",
    "eac_",
    "beservice",
    "battleye",
];

fn is_noise(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    EXE_NOISE.iter().any(|n| lower.contains(n))
}

fn walk_exes(dir: &Path, depth: usize, out: &mut Vec<(PathBuf, u64)>) {
    if depth == 0 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            let name = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            if name == "redist" || name == "_commonredist" || name == "installers" {
                continue;
            }
            walk_exes(&p, depth - 1, out);
        } else if p.extension().map(|e| e.eq_ignore_ascii_case("exe")).unwrap_or(false) {
            let fname = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            if is_noise(&fname) {
                continue;
            }
            if let Ok(m) = entry.metadata() {
                out.push((p, m.len()));
            }
        }
    }
}

/// 找出游戏目录里的主程序：优先用已知路径，否则取体积最大的非安装器 exe（与官方安装器的判定一致）。
pub fn find_exe(dir: &Path, hint: Option<&Path>) -> Option<PathBuf> {
    if let Some(h) = hint {
        if h.is_file() {
            return Some(h.to_path_buf());
        }
    }
    let mut found = Vec::new();
    walk_exes(dir, 3, &mut found);
    found.sort_by(|a, b| b.1.cmp(&a.1));
    found.into_iter().next().map(|(p, _)| p)
}

pub fn running_names() -> HashSet<String> {
    let mut set = HashSet::new();
    let mut cmd = Command::new("tasklist");
    cmd.args(["/FO", "CSV", "/NH"]);
    hidden(&mut cmd);
    if let Ok(out) = cmd.output() {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix('"') {
                if let Some(end) = rest.find('"') {
                    set.insert(rest[..end].to_ascii_lowercase());
                }
            }
        }
    }
    set
}

pub fn scan(manual: &[String]) -> Vec<RawGame> {
    let mut all: Vec<RawGame> = Vec::new();
    all.extend(steam_games());
    all.extend(epic_games());
    all.extend(gog_games());

    for m in manual {
        let dir = PathBuf::from(m);
        if !dir.is_dir() {
            continue;
        }
        let Some(exe) = find_exe(&dir, None) else { continue };
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| m.clone());
        all.push(RawGame {
            name,
            source: "manual".into(),
            last_used: file_mtime_iso(&dir),
            dir,
            exe: exe.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        });
    }

    let mut seen = HashSet::new();
    all.retain(|g| seen.insert(g.dir.to_string_lossy().to_ascii_lowercase()));
    all
}
