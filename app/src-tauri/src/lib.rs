mod catalog;
mod covers;
mod changelog;
mod ini;
mod jobs;
mod net;
mod precheck;
mod tuning;
mod inspect;
mod installer;
mod logs;
mod model;
mod paths;
mod releases;
mod scan;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};

use ini::IniDoc;
use model::{Game, NrFgConfig, ProgressEvent, Settings, SnapshotInfo, Version};
use paths as P;

static CANCELED: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();

fn canceled_set() -> &'static Mutex<std::collections::HashSet<String>> {
    CANCELED.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

pub(crate) fn canceled(task: &str) -> bool {
    canceled_set().lock().unwrap().contains(task)
}

#[tauri::command]
fn cancel_task(task_id: String) {
    canceled_set().lock().unwrap().insert(task_id);
}

static TASK_SEQ: AtomicU64 = AtomicU64::new(1);

pub struct AppState {
    pub data_dir: PathBuf,
    pub settings: Mutex<Settings>,
    pub catalog: Mutex<HashMap<String, String>>,
    pub games: Mutex<Vec<Game>>,
    pub releases: Mutex<Option<(Instant, Vec<releases::Release>)>>,
    /// 用户确认「直接退出」后置位，让关闭事件真正放行
    pub allow_close: AtomicBool,
}

/// 6.5.4：所有重活共用一个闸门。等待时前端任务卡显示「排队中」，取消也生效。
fn task_gate(
    app: &AppHandle,
    task: &str,
    title: &str,
    name: &str,
    limit: u32,
) -> Result<jobs::Ticket, String> {
    jobs::acquire(limit, &|| canceled(task), &|| {
        emit(app, task, title, name, "排队中（等前面的任务完成）", 0.01, "running");
    })
}

/// 5.1：异常态游戏的所有写操作都要人工确认，后端也挡一道
fn require_confirmation(game: &Game, force: Option<bool>) -> Result<(), String> {
    if let Some(reason) = &game.conflict {
        if !force.unwrap_or(false) {
            return Err(format!("CONFLICT|{reason}"));
        }
    }
    Ok(())
}

const OUTPUT_HEADER: &str = "--- 安装器输出 ---";
const SNAPSHOT_MARKER: &str = "[[ANR_SNAPSHOT]]";
const ROLLED_BACK_MARKER: &str = "[[ANR_ROLLED_BACK]]";

fn tail_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let start = s
        .char_indices()
        .rev()
        .nth(max)
        .map(|(i, _)| i)
        .unwrap_or(0);
    format!("…{}", &s[start..])
}

/// 把官方脚本的原始输出附在错误里，供前端「原始输出」区域展示。
fn script_error_text(e: installer::ScriptError) -> String {
    let raw = tail_chars(e.output.trim(), 12_000);
    if raw.is_empty() {
        e.message
    } else {
        format!("{}\n\n{}\n{}", e.message, OUTPUT_HEADER, raw)
    }
}

/// 失败时把快照路径/自动回滚结果用机器可读标记附在错误末尾。
/// 前端会剥掉标记，失败弹窗据此决定「回滚」按钮是否可用。
fn failure_message(mut msg: String, snapshot: Option<&Path>, rolled_back: bool) -> String {
    if let Some(z) = snapshot {
        msg.push('\n');
        msg.push_str(SNAPSHOT_MARKER);
        msg.push_str(&z.to_string_lossy());
    }
    if rolled_back {
        msg.push('\n');
        msg.push_str(ROLLED_BACK_MARKER);
    }
    msg
}

fn failure_ui_text(msg: &str) -> String {
    msg.lines()
        .filter(|l| {
            !l.starts_with(SNAPSHOT_MARKER) && !l.starts_with(ROLLED_BACK_MARKER)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn emit(
    app: &AppHandle,
    id: &str,
    title: &str,
    subtitle: &str,
    stage: &str,
    progress: f64,
    status: &str,
) {
    if matches!(status, "done" | "error" | "canceled") {
        canceled_set().lock().unwrap().remove(id);
    }
    let _ = app.emit(
        "task",
        ProgressEvent {
            id: id.to_string(),
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            stage: stage.to_string(),
            progress,
            status: status.to_string(),
        },
    );
}

fn rescan(state: &AppState) -> Result<Vec<Game>, String> {
    let manual = state.settings.lock().unwrap().manual_games.clone();
    let catalog = state.catalog.lock().unwrap().clone();
    let hidden = state.settings.lock().unwrap().hidden_games.clone();
    let procs = scan::running_names();
    let raws: Vec<_> = scan::scan(&manual)
        .into_iter()
        .filter(|r| !hidden.iter().any(|h| PathBuf::from(h) == r.dir))
        .collect();
    let cover_dir = state.data_dir.join("covers");
    let mut games: Vec<Game> = raws
        .into_iter()
        .map(|r| inspect::build_game(r, &catalog, &procs))
        .collect();
    let renames = state.settings.lock().unwrap().renames.clone();
    for g in games.iter_mut() {
        if let Some(name) = renames.get(&g.id) {
            if !name.trim().is_empty() {
                g.name = name.clone();
            }
        }
        g.cover = covers::data_uri(&covers::cover_path(&cover_dir, &g.id));
    }
    *state.games.lock().unwrap() = games.clone();
    Ok(games)
}

fn ensure_game(state: &AppState, id: &str) -> Result<Game, String> {
    if let Some(g) = state
        .games
        .lock()
        .unwrap()
        .iter()
        .find(|g| g.id == id)
        .cloned()
    {
        return Ok(g);
    }
    let list = rescan(state)?;
    list.into_iter()
        .find(|g| g.id == id)
        .ok_or_else(|| "找不到这个游戏，请刷新列表".to_string())
}

fn get_releases(state: &AppState, refresh: bool) -> Result<Vec<releases::Release>, String> {
    if !refresh {
        let cached = state.releases.lock().unwrap().clone();
        if let Some((at, list)) = cached {
            if at.elapsed() < Duration::from_secs(600) {
                return Ok(list);
            }
        }
    }
    // 下载源既决定安装包走哪条线，也决定版本列表先试哪条线
    let mirror = state.settings.lock().unwrap().mirror.clone();
    let list = releases::list_releases(&mirror)?;
    *state.releases.lock().unwrap() = Some((Instant::now(), list.clone()));
    Ok(list)
}

static COVER_INFLIGHT: std::sync::OnceLock<Mutex<std::collections::HashSet<String>>> =
    std::sync::OnceLock::new();

fn inflight() -> &'static Mutex<std::collections::HashSet<String>> {
    COVER_INFLIGHT.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

#[tauri::command]
fn scan_library(app: AppHandle, state: State<'_, AppState>) -> Result<Vec<Game>, String> {
    let games = rescan(state.inner())?;
    refresh_tray(&app, &games);

    let cover_dir = state.data_dir.join("covers");
    let cover_source = state.settings.lock().unwrap().cover_source.clone();
    let missing: Vec<(String, String)> = games
        .iter()
        .filter(|g| g.cover.is_none())
        .map(|g| (g.id.clone(), g.name.clone()))
        .collect();

    if !missing.is_empty() {
        let app2 = app.clone();
        std::thread::spawn(move || {
            for (id, name) in missing {
                {
                    let mut set = inflight().lock().unwrap();
                    if !set.insert(id.clone()) {
                        continue;
                    }
                }
                if let Ok(p) = covers::ensure_cover(&cover_dir, &id, &name, &cover_source) {
                    if let Some(uri) = covers::data_uri(&p) {
                        let _ = app2.emit("cover", serde_json::json!({ "id": id, "cover": uri }));
                    }
                }
                inflight().lock().unwrap().remove(&id);
            }
        });
    }

    Ok(games)
}

/// 升级后刷新 Windows 图标缓存。安装程序覆盖 exe 后，桌面快捷方式时常仍显示旧图标，
/// 这里让 Shell 重建一次图标缓存（等价于安装器常见的 SHChangeNotify）。
#[cfg(windows)]
fn refresh_shell_icons() {
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHChangeNotify(event: i32, flags: u32, item1: *const std::ffi::c_void, item2: *const std::ffi::c_void);
    }
    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    const SHCNF_IDLIST: u32 = 0x0000;
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

#[cfg(not(windows))]
fn refresh_shell_icons() {}

#[tauri::command]
async fn pick_cover_image() -> Result<Option<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms | Out-Null
$d = New-Object System.Windows.Forms.OpenFileDialog
$d.Title = '选择封面图片'
$d.Filter = '图片文件|*.jpg;*.jpeg;*.png;*.webp;*.gif|所有文件|*.*'
$d.CheckFileExists = $true
if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $d.FileName }
"#;
    let out = tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-STA", "-Command", script]);
        scan::hidden(&mut cmd);
        cmd.output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("打开图片选择器失败：{e}"))?;

    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if text.is_empty() { None } else { Some(text) })
}

#[tauri::command]
fn set_cover_from_file(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    path: String,
) -> Result<Option<String>, String> {
    ensure_game(state.inner(), &id)?;
    let bytes = fs::read(&path).map_err(|e| format!("读取图片失败：{e}"))?;
    if bytes.len() < 512 {
        return Err("这张图片读不出来，换一张试试".into());
    }
    let dir = state.data_dir.join("covers");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let dest = covers::cover_path(&dir, &id);
    fs::write(&dest, &bytes).map_err(|e| format!("保存封面失败：{e}"))?;

    let uri = covers::data_uri(&dest);
    if let Some(g) = state.games.lock().unwrap().iter_mut().find(|g| g.id == id) {
        g.cover = uri.clone();
    }
    let _ = app.emit("cover", serde_json::json!({ "id": id, "cover": uri }));
    Ok(uri)
}

#[tauri::command]
fn remove_game(state: State<'_, AppState>, id: String) -> Result<Vec<Game>, String> {
    let game = ensure_game(state.inner(), &id)?;
    let dir = PathBuf::from(&game.dir);

    let mut s = state.settings.lock().unwrap().clone();
    let before = s.manual_games.len();
    s.manual_games.retain(|p| PathBuf::from(p) != dir);
    if s.manual_games.len() == before {
        // 不是手动添加的（从 Steam 等扫描出来的），记到隐藏列表里
        if !s.hidden_games.iter().any(|h| PathBuf::from(h) == dir) {
            s.hidden_games.push(game.dir.clone());
        }
    }
    P::save_settings(&state.data_dir, &s)?;
    *state.settings.lock().unwrap() = s;

    rescan(state.inner())
}

fn dir_has_exe(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok()).any(|e| {
                e.path()
                    .extension()
                    .map(|x| x.eq_ignore_ascii_case("exe"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

#[tauri::command]
async fn change_game_dir(
    state: State<'_, AppState>,
    id: String,
    path: String,
    migrate: bool,
) -> Result<Vec<Game>, String> {
    let game = ensure_game(state.inner(), &id)?;
    let old_dir = PathBuf::from(&game.dir);
    let new_dir = PathBuf::from(path.trim());

    if !new_dir.is_dir() {
        return Err("这个路径不是文件夹".into());
    }
    if new_dir == old_dir {
        return Err("新目录和原来的一样".into());
    }
    if !dir_has_exe(&new_dir) {
        return Err("这个目录里没有 .exe，请选择游戏主程序所在的文件夹".into());
    }

    // 需要迁移时先把旧目录的安装卸掉（会按哈希还原原文件）
    let old_installed = game.anr.is_some();
    if migrate && old_installed {
        let settings = state.settings.lock().unwrap().clone();
        let data = state.data_dir.clone();
        let d = old_dir.clone();
        let name = game.name.clone();
        tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
            if settings.keep_snapshot {
                let _ = installer::snapshot(&data, &d, &name);
            }
            installer::uninstall(&d)
        })
        .await
        .map_err(|e| format!("卸载旧目录失败：{e}"))??;
    }

    let mut s = state.settings.lock().unwrap().clone();
    let path_str = new_dir.to_string_lossy().to_string();
    let mut replaced = false;
    for p in s.manual_games.iter_mut() {
        if PathBuf::from(&*p) == old_dir {
            *p = path_str.clone();
            replaced = true;
        }
    }
    if !replaced {
        // 扫描到的游戏：旧目录加入隐藏，新目录作为手动添加
        if !s.hidden_games.iter().any(|h| PathBuf::from(h) == old_dir) {
            s.hidden_games.push(game.dir.clone());
        }
        s.manual_games.push(path_str);
    }
    P::save_settings(&state.data_dir, &s)?;
    *state.settings.lock().unwrap() = s;

    rescan(state.inner())
}

#[tauri::command]
fn clear_cover(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    ensure_game(state.inner(), &id)?;
    covers::remove(&state.data_dir.join("covers"), &id);
    if let Some(g) = state.games.lock().unwrap().iter_mut().find(|g| g.id == id) {
        g.cover = None;
    }
    let _ = app.emit("cover", serde_json::json!({ "id": id, "cover": null }));
    Ok(())
}

/// 目标目录里已经有 winmm.dll（常见于破解补丁或别的注入工具）时，
/// 不要让安装脚本自己挑代理名，直接用 dxgi.dll，避免把别人的代理覆盖掉。
fn proxy_hint(dir: &Path) -> Option<&'static str> {
    if dir.join("winmm.dll").exists() && !dir.join("dxgi.dll").exists() {
        Some("dxgi.dll")
    } else {
        None
    }
}

#[tauri::command]
fn read_opti_tuning(state: State<'_, AppState>, id: String) -> Result<tuning::Snapshot, String> {
    let game = ensure_game(state.inner(), &id)?;
    let mut snap = tuning::read(Path::new(&game.dir))?;
    snap.running = game.running;
    Ok(snap)
}

#[tauri::command]
fn write_opti_tuning(
    state: State<'_, AppState>,
    id: String,
    section: String,
    key: String,
    value: String,
) -> Result<String, String> {
    let game = ensure_game(state.inner(), &id)?;
    if game.running {
        return Err(
            "游戏正在运行：OptiScaler 退出时会把自己的设置写回 ini，会覆盖这次的修改。先关掉游戏再调。"
                .into(),
        );
    }
    tuning::write(Path::new(&game.dir), &section, &key, &value)
}

#[tauri::command]
fn precheck_install(state: State<'_, AppState>, id: String) -> Result<Vec<precheck::Issue>, String> {
    let game = ensure_game(state.inner(), &id)?;
    let warn_anticheat = state.settings.lock().unwrap().warn_anti_cheat;
    Ok(precheck::run(Path::new(&game.dir), warn_anticheat))
}

#[tauri::command]
fn rename_game(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
) -> Result<Vec<Game>, String> {
    ensure_game(state.inner(), &id)?;

    let trimmed = name.unwrap_or_default().trim().to_string();
    let mut s = state.settings.lock().unwrap().clone();
    if trimmed.is_empty() {
        s.renames.remove(&id);
    } else {
        s.renames.insert(id.clone(), trimmed);
    }
    P::save_settings(&state.data_dir, &s)?;
    *state.settings.lock().unwrap() = s;

    let games = rescan(state.inner())?;

    // 名字变了，旧封面就不再对应，直接重抓
    if let Some(game) = games.iter().find(|g| g.id == id).cloned() {
        let dir = state.data_dir.join("covers");
        let source = state.settings.lock().unwrap().cover_source.clone();
        covers::remove(&dir, &id);
        if let Ok(path) = covers::ensure_cover(&dir, &id, &game.name, &source) {
            let uri = covers::data_uri(&path);
            if let Some(g) = state.games.lock().unwrap().iter_mut().find(|g| g.id == id) {
                g.cover = uri.clone();
            }
            let _ = app.emit("cover", serde_json::json!({ "id": id, "cover": uri }));
        }
    }

    Ok(state.games.lock().unwrap().clone())
}

#[tauri::command]
fn refresh_cover(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<Option<String>, String> {
    let game = ensure_game(state.inner(), &id)?;
    let dir = state.data_dir.join("covers");
    let source = state.settings.lock().unwrap().cover_source.clone();
    covers::remove(&dir, &id);
    let path = covers::ensure_cover(&dir, &id, &game.name, &source)?;
    let uri = covers::data_uri(&path);
    if let Some(g) = state.games.lock().unwrap().iter_mut().find(|g| g.id == id) {
        g.cover = uri.clone();
    }
    let _ = app.emit("cover", serde_json::json!({ "id": id, "cover": uri }));
    Ok(uri)
}

#[tauri::command]
fn cover_provider() -> String {
    covers::provider().to_string()
}

#[tauri::command]
fn delete_version(state: State<'_, AppState>, version: String) -> Result<u64, String> {
    let cache = PathBuf::from(&state.settings.lock().unwrap().cache_dir);
    let dir = cache.join(&version);
    if !dir.exists() {
        return Err("这个版本没有缓存".into());
    }
    let freed = dir_size(&dir);
    fs::remove_dir_all(&dir).map_err(|e| format!("删除缓存失败：{e}"))?;
    Ok(freed)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    P::save_settings(&state.data_dir, &settings)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}

#[tauri::command]
fn add_manual_game(state: State<'_, AppState>, path: String) -> Result<Option<String>, String> {
    let dir = PathBuf::from(&path);
    if !dir.is_dir() {
        return Err("这个路径不是一个文件夹".into());
    }
    if scan::find_exe(&dir, None).is_none() {
        return Err("这个文件夹里没有找到 .exe，请选择游戏主程序所在的文件夹".into());
    }
    let mut s = state.settings.lock().unwrap().clone();
    if !s.manual_games.iter().any(|p| p.eq_ignore_ascii_case(&path)) {
        s.manual_games.push(path);
        P::save_settings(&state.data_dir, &s)?;
        *state.settings.lock().unwrap() = s;
    }
    Ok(precheck::steamapps_warning(&dir))
}

#[tauri::command]
fn remove_manual_game(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let mut s = state.settings.lock().unwrap().clone();
    s.manual_games.retain(|p| !p.eq_ignore_ascii_case(&path));
    P::save_settings(&state.data_dir, &s)?;
    *state.settings.lock().unwrap() = s;
    Ok(())
}

#[tauri::command]
async fn get_changelog(
    state: State<'_, AppState>,
    tag: String,
) -> Result<changelog::Changelog, String> {
    // 先用内存里的缓存（哪怕已过期），只有一次都没拉到过才去请求网络，
    // 否则网络一抖，所有版本的日志都会变成"没有提供"。
    let cached = state.releases.lock().unwrap().clone().map(|(_, list)| list);
    let rels = match cached {
        Some(list) if !list.is_empty() => list,
        _ => get_releases(state.inner(), false)?,
    };
    let data = state.data_dir.clone();
    let rel = rels.into_iter().find(|r| r.tag == tag);
    let tag2 = tag.clone();
    tauri::async_runtime::spawn_blocking(move || changelog::build(&data, rel.as_ref(), &tag2))
        .await
        .map_err(|e| format!("读取更新日志失败：{e}"))
}

#[tauri::command]
fn list_releases(state: State<'_, AppState>, refresh: Option<bool>) -> Result<Vec<Version>, String> {
    let rels = get_releases(state.inner(), refresh.unwrap_or(false))?;
    let cache = PathBuf::from(&state.settings.lock().unwrap().cache_dir);
    let catalog = state.catalog.lock().unwrap().clone();
    // 游戏里正在用的旧版本也要留在列表里，否则清掉缓存后就看不见、也回不去了
    let installed: Vec<String> = state
        .games
        .lock()
        .unwrap()
        .iter()
        .filter_map(|g| g.anr.as_ref().map(|a| a.version.clone()))
        .filter(|v| v != "未知" && !v.is_empty())
        .collect();
    Ok(releases::to_versions(&rels, &cache, &catalog, &installed))
}

fn record_fingerprint(state: &AppState, pkg: &Path, tag: &str) {
    let p = pkg.join("nr_fg_package.json");
    let Ok(text) = fs::read_to_string(p) else { return };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    if let Some(sha) = v.get("provider_sha256").and_then(|x| x.as_str()) {
        let mut cat = state.catalog.lock().unwrap();
        cat.insert(sha.to_string(), tag.to_string());
        let _ = P::save_catalog(&state.data_dir, &cat);
    }
}

fn do_install(
    app: &AppHandle,
    task: &str,
    name: &str,
    dir: &Path,
    cache: &Path,
    kind: &str,
    tag: &str,
    mirror: &str,
    force: bool,
) -> Result<PathBuf, String> {
    emit(app, task, "安装", name, "前置检查", 0.04, "running");

    if !dir.is_dir() {
        return Err("游戏目录不存在".into());
    }
    let exe = scan::find_exe(dir, None).ok_or("这个目录里没有找到 .exe")?;
    let procs = scan::running_names();
    let exe_name = exe
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if procs.contains(&exe_name) {
        return Err(format!("游戏正在运行（{exe_name}），请先关闭游戏"));
    }
    let opti = inspect::opti_dir(dir);
    if opti.exists() && !inspect::record_path(dir).exists() && kind != "addon" && !force {
        return Err("这个目录里已有不是本工具安装的 OptiScaler，请改用插件安装方式".into());
    }

    emit(app, task, "安装", name, "准备安装包", 0.08, "running");
    let pkg = installer::ensure_package(
        cache,
        tag,
        kind,
        mirror,
        |stage, p| emit(app, task, "安装", name, stage, 0.08 + p * 0.78, "running"),
        &|| canceled(task),
    )?;
    if canceled(task) {
        return Err("已取消".into());
    }

    if canceled(task) {
        return Err("已取消".into());
    }
    emit(app, task, "安装", name, "执行安装脚本", 0.9, "running");
    installer::run_install(&pkg, dir, proxy_hint(dir)).map_err(script_error_text)?;

    emit(app, task, "安装", name, "后置校验", 0.97, "running");
    if !inspect::provider_dll(dir).exists() {
        return Err("安装脚本执行完成，但没有找到 provider 文件".into());
    }

    emit(app, task, "安装", name, "已完成", 1.0, "done");
    Ok(pkg)
}

#[tauri::command]
async fn install(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    tag: Option<String>,
    force: Option<bool>,
) -> Result<(), String> {
    let game = ensure_game(state.inner(), &id)?;
    require_confirmation(&game, force)?;
    let settings = state.settings.lock().unwrap().clone();
    let kind = if game.third_party_optiscaler { "addon" } else { "full" }.to_string();

    let target = match tag {
        Some(t) if !t.is_empty() => t,
        // 6.4④「设为默认」：没指定版本时优先用用户选定的默认版本
        _ if !settings.default_version.trim().is_empty() => {
            settings.default_version.trim().to_string()
        }
        _ => match &game.anr {
            Some(a) if a.version != "未知" => a.version.clone(),
            _ => get_releases(state.inner(), false)?
                .first()
                .map(|r| r.tag.clone())
                .ok_or("无法获取最新版本")?,
        },
    };

    let cache = PathBuf::from(&settings.cache_dir);
    let mirror = settings.mirror.clone();
    let dir = PathBuf::from(&game.dir);
    let name = game.name.clone();
    let target_out = target.clone();
    let task = format!("task-{}", TASK_SEQ.fetch_add(1, Ordering::Relaxed));
    let app2 = app.clone();
    let task2 = task.clone();
    let name2 = name.clone();

    let had_install = game.anr.is_some();
    let snap = if had_install && settings.keep_snapshot {
        installer::snapshot(&state.data_dir, &dir, &name).ok()
    } else {
        None
    };
    let dir_keep = dir.clone();
    let limit = settings.concurrency;

    let result = tauri::async_runtime::spawn_blocking(move || {
        let _ticket = task_gate(&app2, &task2, "安装", &name2, limit)?;
        do_install(
            &app2, &task2, &name2, &dir, &cache, &kind, &target, &mirror,
            force.unwrap_or(false),
        )
    })
    .await
    .map_err(|e| format!("任务异常：{e}"))?;

    match result {
        Ok(path) => {
            record_fingerprint(state.inner(), &path, &target_out);
            let _ = rescan(state.inner());
            Ok(())
        }
        Err(e) => {
            let mut msg = e;
            let mut rolled_back = false;
            if let Some(z) = &snap {
                match installer::restore_snapshot(z, &dir_keep) {
                    Ok(()) => {
                        rolled_back = true;
                        msg = format!("{msg}（已自动回滚到安装前的状态）");
                        let _ = rescan(state.inner());
                    }
                    Err(re) => {
                        msg = format!("{msg}\n自动回滚失败：{re}。可以点「回滚到快照」手动恢复。")
                    }
                }
            }
            let full = failure_message(msg, snap.as_deref(), rolled_back);
            emit(&app, &task, "安装", &name, &failure_ui_text(&full), 1.0, "error");
            Err(full)
        }
    }
}

#[tauri::command]
async fn uninstall(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    force: Option<bool>,
) -> Result<(), String> {
    let game = ensure_game(state.inner(), &id)?;
    if game.anr.is_none() {
        return Err("这个游戏没有安装 ANR".into());
    }
    require_confirmation(&game, force)?;
    let settings = state.settings.lock().unwrap().clone();
    let data = state.data_dir.clone();
    let dir = PathBuf::from(&game.dir);
    let name = game.name.clone();
    let task = format!("task-{}", TASK_SEQ.fetch_add(1, Ordering::Relaxed));
    let app2 = app.clone();
    let task2 = task.clone();
    let name2 = name.clone();
    let limit = settings.concurrency;

    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let _ticket = task_gate(&app2, &task2, "卸载", &name2, limit)?;
        if settings.keep_snapshot {
            emit(&app2, &task2, "卸载", &name2, "生成快照", 0.2, "running");
            installer::snapshot(&data, &dir, &name2)?;
        }
        emit(&app2, &task2, "卸载", &name2, "执行卸载", 0.7, "running");
        installer::uninstall(&dir)?;
        emit(&app2, &task2, "卸载", &name2, "校验还原", 1.0, "done");
        Ok(())
    })
    .await
    .map_err(|e| format!("任务异常：{e}"))?;

    match result {
        Ok(()) => {
            let _ = rescan(state.inner());
            Ok(())
        }
        Err(e) => {
            emit(&app, &task, "卸载", &name, &e, 1.0, "error");
            Err(e)
        }
    }
}

#[tauri::command]
async fn switch_version(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    tag: String,
    force: Option<bool>,
) -> Result<(), String> {
    let game = ensure_game(state.inner(), &id)?;
    require_confirmation(&game, force)?;
    let Some(anr) = game.anr.clone() else {
        return Err("这个游戏没有安装 ANR".into());
    };
    let settings = state.settings.lock().unwrap().clone();
    let cache = PathBuf::from(&settings.cache_dir);
    let mirror = settings.mirror.clone();
    let dir = PathBuf::from(&game.dir);
    let name = game.name.clone();
    let kind = anr.kind.clone();
    let tag2 = tag.clone();
    let task = format!("task-{}", TASK_SEQ.fetch_add(1, Ordering::Relaxed));

    // 覆盖安装前先留一份快照，失败时能回到切换前的状态
    let snap = if settings.keep_snapshot {
        installer::snapshot(&state.data_dir, &dir, &name).ok()
    } else {
        None
    };
    let dir2 = dir.clone();
    let task3 = task.clone();
    let name3 = name.clone();
    let app3 = app.clone();
    let limit = settings.concurrency;

    let result = tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf, String> {
        let _ticket = task_gate(&app3, &task3, "切换版本", &name3, limit)?;
        emit(&app3, &task3, "切换版本", &name3, "准备目标版本", 0.1, "running");
        let pkg = installer::ensure_package(
            &cache,
            &tag2,
            &kind,
            &mirror,
            |stage, p| emit(&app3, &task3, "切换版本", &name3, stage, 0.1 + p * 0.7, "running"),
            &|| canceled(&task3),
        )?;
        if canceled(&task3) {
            return Err("已取消".into());
        }
        emit(&app3, &task3, "切换版本", &name3, "覆盖安装", 0.9, "running");
        installer::run_install(&pkg, &dir2, proxy_hint(&dir2)).map_err(script_error_text)?;
        emit(&app3, &task3, "切换版本", &name3, "指纹校验", 1.0, "done");
        Ok(pkg)
    })
    .await
    .map_err(|e| format!("任务异常：{e}"))?;

    match result {
        Ok(path) => {
            record_fingerprint(state.inner(), &path, &tag);
            let _ = rescan(state.inner());
            Ok(())
        }
        Err(e) => {
            let mut msg = e;
            let mut rolled_back = false;
            if let Some(z) = &snap {
                match installer::restore_snapshot(z, &dir) {
                    Ok(()) => {
                        rolled_back = true;
                        msg = format!("{msg}（已自动回滚到切换前的状态）");
                        let _ = rescan(state.inner());
                    }
                    Err(re) => {
                        msg = format!("{msg}\n自动回滚失败：{re}。可以点「回滚到快照」手动恢复。")
                    }
                }
            }
            let full = failure_message(msg, snap.as_deref(), rolled_back);
            emit(&app, &task, "切换版本", &name, &failure_ui_text(&full), 1.0, "error");
            Err(full)
        }
    }
}

#[tauri::command]
fn set_fg_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
    force: Option<bool>,
) -> Result<(), String> {
    let game = ensure_game(state.inner(), &id)?;
    require_confirmation(&game, force)?;
    let dir = PathBuf::from(&game.dir);
    let ini_p = inspect::ini_path(&dir);
    if !ini_p.exists() {
        return Err("这个目录没有 OptiScaler.ini，无法写入开关".into());
    }
    let mut doc = IniDoc::load(&ini_p);
    IniDoc::backup_once(&ini_p);
    doc.set("FrameGen", "Enabled", if enabled { "true" } else { "false" });
    doc.save(&ini_p).map_err(|e| format!("写入 OptiScaler.ini 失败：{e}"))?;
    let _ = rescan(state.inner());
    Ok(())
}

#[tauri::command]
fn write_config(state: State<'_, AppState>, id: String, config: NrFgConfig) -> Result<(), String> {
    let game = ensure_game(state.inner(), &id)?;
    let dir = PathBuf::from(&game.dir);
    let ini_p = inspect::nrf_g_ini(&dir);
    if !ini_p.exists() {
        return Err("找不到 nr_fg.ini".into());
    }
    let mut doc = IniDoc::load(&ini_p);
    IniDoc::backup_once(&ini_p);
    doc.set_flat("path", &config.path);
    doc.set_flat("model_size", &config.model_size);
    doc.set_flat("nits_per_unit", &config.nits.to_string());
    doc.set_flat("max_generated", &config.max_generated.to_string());
    doc.set_flat("scene_cut", if config.scene_cut { "1" } else { "0" });
    doc.save(&ini_p).map_err(|e| format!("写入 OptiScaler.ini 失败：{e}"))?;
    let _ = rescan(state.inner());
    Ok(())
}

#[tauri::command]
fn set_fg_replacement(
    state: State<'_, AppState>,
    id: String,
    mode: String,
    force: Option<bool>,
) -> Result<(), String> {
    let game = ensure_game(state.inner(), &id)?;
    require_confirmation(&game, force)?;
    let dir = PathBuf::from(&game.dir);
    let ini_p = inspect::ini_path(&dir);
    if !ini_p.exists() {
        return Err("找不到 OptiScaler.ini".into());
    }
    let value = match mode.as_str() {
        "anr" => "Apex Neural Rendering FG",
        "native" => "None",
        // Q1 三态：留一个「跟随 OptiScaler 自己判断」的选项
        "auto" => "auto",
        _ => return Err("未知的帧生成实现".into()),
    };
    let mut doc = IniDoc::load(&ini_p);
    IniDoc::backup_once(&ini_p);
    doc.set("FrameGen", "FGNvngxReplacement", value);
    doc.save(&ini_p)
        .map_err(|e| format!("写入 OptiScaler.ini 失败：{e}"))?;
    let _ = rescan(state.inner());
    Ok(())
}

#[tauri::command]
fn repair_install(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let game = ensure_game(state.inner(), &id)?;
    let dir = PathBuf::from(&game.dir);
    let ini_p = inspect::ini_path(&dir);
    if !ini_p.exists() {
        return Err("这个目录没有 OptiScaler.ini，无法修复".into());
    }
    let mut doc = IniDoc::load(&ini_p);
    IniDoc::backup_once(&ini_p);
    doc.set("FrameGen", "FGInput", "nvngxfg");
    doc.set("FrameGen", "FGNvngxReplacement", inspect::PROVIDER);
    doc.save(&ini_p).map_err(|e| format!("写入 OptiScaler.ini 失败：{e}"))?;
    let _ = rescan(state.inner());
    Ok(())
}

#[tauri::command]
fn get_log(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let game = ensure_game(state.inner(), &id)?;
    logs::read_log(Path::new(&game.dir), 400)
}

#[tauri::command]
fn make_diag(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let game = ensure_game(state.inner(), &id)?;
    let json = serde_json::to_string_pretty(&game).unwrap_or_default();
    let out = logs::diag_bundle(&state.data_dir, Path::new(&game.dir), &game.name, &json)?;
    Ok(out.to_string_lossy().to_string())
}

/// P0-1：列出这个游戏的快照（新快照按 gameId 匹配，旧快照按文件名前缀兜底）
#[tauri::command]
fn list_snapshots(state: State<'_, AppState>, id: String) -> Result<Vec<SnapshotInfo>, String> {
    let game = ensure_game(state.inner(), &id)?;
    installer::list_snapshots(&state.data_dir, &game.id, &game.name)
}

/// P0-1：把指定快照恢复回游戏目录，恢复代理 DLL / OptiScaler.ini / OptiScaler 目录
#[tauri::command]
async fn restore_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    path: String,
) -> Result<Vec<Game>, String> {
    let game = ensure_game(state.inner(), &id)?;
    if game.running {
        return Err("游戏正在运行，请先退出游戏再恢复快照".into());
    }

    let snap = PathBuf::from(path.trim());
    let root = P::snapshots_dir(&state.data_dir);
    let root_canon = root.canonicalize().unwrap_or_else(|_| root.clone());
    let snap_canon = snap
        .canonicalize()
        .map_err(|_| "快照文件不存在，可能已经被删除".to_string())?;
    if !snap_canon.starts_with(&root_canon) {
        return Err("只能恢复快照目录里的文件".into());
    }
    installer::validate_snapshot_for_game(&snap_canon, &game.id, &game.name)?;

    let dir = PathBuf::from(&game.dir);
    tauri::async_runtime::spawn_blocking(move || installer::restore_snapshot(&snap_canon, &dir))
        .await
        .map_err(|e| format!("恢复任务异常：{e}"))??;

    let games = rescan(state.inner())?;
    refresh_tray(&app, &games);
    Ok(games)
}

#[tauri::command]
async fn prefetch(
    app: AppHandle,
    state: State<'_, AppState>,
    version: String,
    kind: Option<String>,
) -> Result<(), String> {
    let settings = state.settings.lock().unwrap().clone();
    let cache = PathBuf::from(&settings.cache_dir);
    let mirror = settings.mirror.clone();
    let kind = kind.unwrap_or_else(|| "full".into());
    let task = format!("task-{}", TASK_SEQ.fetch_add(1, Ordering::Relaxed));
    let app2 = app.clone();
    let task2 = task.clone();
    let ver2 = version.clone();
    let limit = settings.concurrency;

    let res = tauri::async_runtime::spawn_blocking(move || {
        let _ticket = task_gate(&app2, &task2, "下载", &ver2, limit)?;
        installer::ensure_package(
            &cache,
            &ver2,
            &kind,
            &mirror,
            |stage, p| emit(&app2, &task2, "下载", &ver2, stage, p, "running"),
            &|| canceled(&task2),
        )
        .map(|_| ())
    })
    .await
    .map_err(|e| format!("任务异常：{e}"))?;

    match res {
        Ok(()) => {
            emit(&app, &task, "下载", &version, "已缓存", 1.0, "done");
            let _ = rescan(state.inner());
            Ok(())
        }
        Err(e) => {
            emit(&app, &task, "下载", &version, &e, 1.0, "error");
            Err(e)
        }
    }
}

// ---------- 版本管理 / 下载健壮性（设计文档 6.4④ / Q2 / R5 / R6 / R7 / R8） ----------

#[tauri::command]
fn set_default_version(
    state: State<'_, AppState>,
    version: Option<String>,
) -> Result<Settings, String> {
    let mut s = state.settings.lock().unwrap().clone();
    s.default_version = version.unwrap_or_default().trim().to_string();
    P::save_settings(&state.data_dir, &s)?;
    *state.settings.lock().unwrap() = s.clone();
    Ok(s)
}

#[tauri::command]
fn mark_version(state: State<'_, AppState>, id: String, tag: String) -> Result<Vec<Game>, String> {
    let game = ensure_game(state.inner(), &id)?;
    let dll = inspect::provider_dll(Path::new(&game.dir));
    let sha = inspect::sha256_file(&dll)
        .ok_or("这个游戏没有找到 provider DLL，无法标记版本")?;
    let clean = tag.trim();
    if clean.is_empty() {
        return Err("请选择要标记的版本".into());
    }
    {
        let mut cat = state.catalog.lock().unwrap();
        cat.insert(sha, clean.to_string());
        P::save_catalog(&state.data_dir, &cat)?;
    }
    rescan(state.inner())
}

#[tauri::command]
async fn refresh_catalog(state: State<'_, AppState>) -> Result<catalog::RefreshResult, String> {
    let data = state.data_dir.clone();
    let cache = PathBuf::from(&state.settings.lock().unwrap().cache_dir);
    let mut cat = state.catalog.lock().unwrap().clone();
    let (result, cat) = tauri::async_runtime::spawn_blocking(move || {
        let r = catalog::refresh(&data, &cache, &mut cat);
        (r, cat)
    })
    .await
    .map_err(|e| format!("任务异常：{e}"))?;
    let result = result?;
    *state.catalog.lock().unwrap() = cat;
    let _ = rescan(state.inner());
    Ok(result)
}

#[tauri::command]
fn check_package_structure(state: State<'_, AppState>) -> Vec<String> {
    let cache = PathBuf::from(&state.settings.lock().unwrap().cache_dir);
    installer::check_cached_packages(&cache)
}

#[tauri::command]
async fn official_sum(
    state: State<'_, AppState>,
    tag: String,
    filename: String,
) -> Result<String, String> {
    let mirror = state.settings.lock().unwrap().mirror.clone();
    tauri::async_runtime::spawn_blocking(move || installer::official_sum(&tag, &filename, &mirror))
        .await
        .map_err(|e| format!("任务异常：{e}"))?
}

#[tauri::command]
async fn pick_zip_file() -> Result<Option<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms | Out-Null
$d = New-Object System.Windows.Forms.OpenFileDialog
$d.Title = '选择 ANR 官方安装包'
$d.Filter = 'ANR 安装包 (*.zip)|*.zip'
$d.Multiselect = $false
if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $d.FileName }
"#;
    let out = tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-STA", "-Command", script]);
        scan::hidden(&mut cmd);
        cmd.output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("打开选择器失败：{e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if text.is_empty() { None } else { Some(text) })
}

#[tauri::command]
async fn import_local_package(
    state: State<'_, AppState>,
    path: String,
    tag: String,
    kind: Option<String>,
    sha256: Option<String>,
) -> Result<(), String> {
    let settings = state.settings.lock().unwrap().clone();
    let cache = PathBuf::from(&settings.cache_dir);
    let kind = kind.unwrap_or_else(|| "full".into());
    let zip = PathBuf::from(&path);
    let filename = zip
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mirror = settings.mirror.clone();
    let tag_for_sum = tag.clone();
    let tag_out = tag.clone();

    let pkg = tauri::async_runtime::spawn_blocking(move || {
        let expected = match sha256 {
            Some(s) if !s.trim().is_empty() => s.trim().to_string(),
            _ => installer::official_sum(&tag_for_sum, &filename, &mirror)
                .map_err(|e| format!("{e}。请在导入窗口里手动填入官方 SHA256"))?,
        };
        installer::import_local(&zip, &cache, &tag, &kind, &expected)
    })
    .await
    .map_err(|e| format!("任务异常：{e}"))??;

    record_fingerprint(state.inner(), &pkg, &tag_out);
    let _ = rescan(state.inner());
    Ok(())
}

fn dir_size(p: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(rd) = fs::read_dir(p) else { return 0 };
    for e in rd.flatten() {
        let path = e.path();
        if path.is_dir() {
            total += dir_size(&path);
        } else if let Ok(m) = e.metadata() {
            total += m.len();
        }
    }
    total
}

#[tauri::command]
fn clear_cache(state: State<'_, AppState>) -> Result<u64, String> {
    let cache = PathBuf::from(&state.settings.lock().unwrap().cache_dir);
    if !cache.exists() {
        return Ok(0);
    }
    let freed = dir_size(&cache);
    fs::remove_dir_all(&cache).map_err(|e| format!("清理缓存失败：{e}"))?;
    let _ = fs::create_dir_all(&cache);
    Ok(freed)
}

#[tauri::command]
async fn pick_folder() -> Result<Option<String>, String> {
    let script = r#"
Add-Type -AssemblyName System.Windows.Forms | Out-Null
$d = New-Object System.Windows.Forms.FolderBrowserDialog
$d.Description = '选择游戏主程序所在的文件夹'
$d.ShowNewFolderButton = $false
if ($d.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) { Write-Output $d.SelectedPath }
"#;
    let out = tauri::async_runtime::spawn_blocking(move || {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-STA", "-Command", script]);
        scan::hidden(&mut cmd);
        cmd.output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("打开选择器失败：{e}"))?;

    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if text.is_empty() { None } else { Some(text) })
}

/// 把主窗口从托盘显示出来
/// 托盘提示里带上"已安装 / 已启用"，不点开也能看到大概状态
fn refresh_tray(app: &AppHandle, games: &[Game]) {
    let installed = games.iter().filter(|g| g.anr.is_some()).count();
    let enabled = games
        .iter()
        .filter(|g| g.anr.as_ref().map(|a| a.enabled).unwrap_or(false))
        .count();
    let tip = if installed > 0 {
        format!("ANR Studio · 已安装 {installed} · 已启用 {enabled}")
    } else {
        "ANR Studio".to_string()
    };
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(&tip));
    }
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
fn show_window(app: AppHandle) -> Result<(), String> {
    show_main(&app);
    Ok(())
}

#[tauri::command]
fn close_window(
    app: AppHandle,
    state: State<'_, AppState>,
    action: String,
    remember: bool,
) -> Result<(), String> {
    if remember && (action == "tray" || action == "exit") {
        let mut s = state.settings.lock().unwrap().clone();
        s.close_action = action.clone();
        P::save_settings(&state.data_dir, &s)?;
        *state.settings.lock().unwrap() = s;
    }
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;

    if action == "tray" {
        win.hide().map_err(|e| format!("隐藏窗口失败：{e}"))?;
    } else {
        state.allow_close.store(true, Ordering::SeqCst);
        win.close().map_err(|e| format!("关闭窗口失败：{e}"))?;
    }
    Ok(())
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("只允许打开 http(s) 链接".into());
    }
    let mut cmd = Command::new("cmd");
    cmd.args(["/C", "start", "", &url]);
    scan::hidden(&mut cmd);
    cmd.spawn().map_err(|e| format!("打开链接失败：{e}"))?;
    Ok(())
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    Command::new("explorer")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("打开目录失败：{e}"))?;
    Ok(())
}

#[tauri::command]
fn app_paths(state: State<'_, AppState>) -> serde_json::Value {
    serde_json::json!({
        "data": state.data_dir.to_string_lossy(),
        "cache": state.settings.lock().unwrap().cache_dir,
        "snapshots": P::snapshots_dir(&state.data_dir).to_string_lossy(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data = app
                .path()
                .app_local_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("anr-studio");
            let _ = fs::create_dir_all(&data);
            let mut settings = P::load_settings(&data);
            let catalog = P::load_catalog(&data);

            // 版本变化时刷新一次系统图标缓存，避免升级后桌面快捷方式仍显示旧图标
            if settings.last_version != env!("CARGO_PKG_VERSION") {
                refresh_shell_icons();
                settings.last_version = env!("CARGO_PKG_VERSION").to_string();
                let _ = P::save_settings(&data, &settings);
            }

            app.manage(AppState {
                data_dir: data,
                settings: Mutex::new(settings),
                catalog: Mutex::new(catalog),
                games: Mutex::new(Vec::new()),
                releases: Mutex::new(None),
                allow_close: AtomicBool::new(false),
            });

            // 系统托盘
            let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出 ANR Studio", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&show_item, &sep, &quit_item])?;

            let mut tray = TrayIconBuilder::with_id("main")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("ANR Studio")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            // 后台探测封面图源，避免第一次抓封面时才等网络
            std::thread::spawn(|| {
                let _ = covers::provider();
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<AppState>();
                if state.allow_close.load(Ordering::SeqCst) {
                    return;
                }
                let action = state.settings.lock().unwrap().close_action.clone();
                match action.as_str() {
                    "tray" => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    "exit" => {}
                    _ => {
                        api.prevent_close();
                        let _ = app.emit("close-requested", ());
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            scan_library,
            get_settings,
            set_settings,
            add_manual_game,
            remove_manual_game,
            list_releases,
            get_changelog,
            install,
            uninstall,
            switch_version,
            set_fg_enabled,
            set_fg_replacement,
            write_config,
            repair_install,
            get_log,
            make_diag,
            list_snapshots,
            restore_snapshot,
            prefetch,
            clear_cache,
            delete_version,
            set_default_version,
            mark_version,
            refresh_catalog,
            check_package_structure,
            official_sum,
            pick_zip_file,
            import_local_package,
            refresh_cover,
            rename_game,
            remove_game,
            precheck_install,
            read_opti_tuning,
            write_opti_tuning,
            cancel_task,
            change_game_dir,
            clear_cover,
            pick_cover_image,
            set_cover_from_file,
            cover_provider,
            pick_folder,
            open_path,
            open_url,
            close_window,
            show_window,
            app_paths
        ])
        .run(tauri::generate_context!())
        .expect("error while running ANR Studio");
}
