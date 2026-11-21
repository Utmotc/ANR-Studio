mod catalog;
mod covers;
mod changelog;
mod crash;
mod health;
mod ini;
mod jobs;
mod net;
mod precheck;
mod sysinfo;
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
use tauri_plugin_updater::UpdaterExt;

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

/// 把阻塞型命令挪到 blocking 线程池：非 async 的 Tauri 命令跑在主线程上，
/// 全盘扫描 / 哈希 / 网络 / 写盘都会把窗口一起卡住。
async fn run_blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(f).await {
        Ok(r) => r,
        Err(e) => Err(format!("任务异常：{e}")),
    }
}

/// 所有会改动游戏目录的操作都先实时复查一次进程。
/// 界面里的 `running` 来自上次扫描，游戏可能在应用开着的时候被启动。
fn ensure_not_running(game: &Game, action: &str) -> Result<(), String> {
    if scan::game_running(Path::new(&game.dir), &game.exe) {
        return Err(format!(
            "{} 正在运行，请先退出游戏再{action}（安装器会拒绝写入被占用的文件）",
            game.name
        ));
    }
    Ok(())
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
/// 取消的文案统一以「已取消」开头，前端据此把它显示成取消而不是失败。
fn script_error_text(e: installer::ScriptError) -> String {
    let head = if e.canceled && !e.message.starts_with("已取消") {
        format!("已取消：{}", e.message)
    } else {
        e.message
    };
    let raw = tail_chars(e.output.trim(), 12_000);
    if raw.is_empty() {
        head
    } else {
        format!("{head}\n\n{OUTPUT_HEADER}\n{raw}")
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

/// 取消和失败在界面上是两种东西：取消不该弹「失败」红色弹窗。
fn err_status(msg: &str) -> &'static str {
    if msg.contains("已取消") {
        "canceled"
    } else {
        "error"
    }
}

/// 有快照就回滚，并把结果写进错误文案。
///
/// 「游戏正在运行」「用户取消」这两种失败不回滚：前者的文件正被占用，
/// 回滚只会把目录拆成半成品；后者本来就是用户主动停下的，再动一次盘更吓人。
fn rollback_if_possible(snap: Option<&Path>, dir: &Path, msg: String) -> (String, bool) {
    if msg.contains("正在运行") || msg.contains("已取消") {
        return (msg, false);
    }
    match snap {
        Some(z) => match installer::restore_snapshot(z, dir) {
            Ok(()) => (format!("{msg}（已自动回滚到操作前的状态）"), true),
            Err(re) => (
                format!("{msg}\n自动回滚失败：{re}。可以点「回滚到快照」手动恢复。"),
                false,
            ),
        },
        None => (msg, false),
    }
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
async fn scan_library(app: AppHandle) -> Result<Vec<Game>, String> {
    // 扫描要遍历整库目录 + 读每个已装游戏的 provider 指纹，必须离开主线程
    let app2 = app.clone();
    let (games, cover_dir, cover_source, missing) = run_blocking(move || {
        let st = app2.state::<AppState>();
        let games = rescan(st.inner())?;
        let cover_dir = st.data_dir.join("covers");
        let cover_source = st.settings.lock().unwrap().cover_source.clone();
        let missing: Vec<(String, String)> = games
            .iter()
            .filter(|g| g.cover.is_none())
            .map(|g| (g.id.clone(), g.name.clone()))
            .collect();
        Ok((games, cover_dir, cover_source, missing))
    })
    .await?;
    refresh_tray(&app, &games);

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

/// 前端每 5 秒问一次：哪些游戏现在正在跑。
/// 只做一次 tasklist + 每个已装游戏的代理 DLL 占用探测，代价可控。
#[tauri::command]
async fn get_running_ids(app: AppHandle) -> Result<Vec<String>, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let games = st.games.lock().unwrap().clone();
        if games.is_empty() {
            return Ok(Vec::new());
        }
        let names = scan::running_names();
        let mut out = Vec::new();
        for g in games {
            if scan::game_running_with(Path::new(&g.dir), &g.exe, &names) {
                out.push(g.id);
            }
        }
        Ok(out)
    })
    .await
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
async fn set_cover_from_file(app: AppHandle, id: String, path: String) -> Result<Option<String>, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        ensure_game(st.inner(), &id)?;
        let bytes = fs::read(&path).map_err(|e| format!("读取图片失败：{e}"))?;
        if bytes.len() < 512 {
            return Err("这张图片读不出来，换一张试试".into());
        }
        let dir = st.data_dir.join("covers");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let dest = covers::cover_path(&dir, &id);
        fs::write(&dest, &bytes).map_err(|e| format!("保存封面失败：{e}"))?;

        let uri = covers::data_uri(&dest);
        if let Some(g) = st.games.lock().unwrap().iter_mut().find(|g| g.id == id) {
            g.cover = uri.clone();
        }
        let _ = app.emit("cover", serde_json::json!({ "id": id, "cover": uri }));
        Ok(uri)
    })
    .await
}

#[tauri::command]
async fn remove_game(app: AppHandle, id: String) -> Result<Vec<Game>, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        let dir = PathBuf::from(&game.dir);

        let mut s = st.settings.lock().unwrap().clone();
        let before = s.manual_games.len();
        s.manual_games.retain(|p| PathBuf::from(p) != dir);
        if s.manual_games.len() == before {
            // 不是手动添加的（从 Steam 等扫描出来的），记到隐藏列表里
            if !s.hidden_games.iter().any(|h| PathBuf::from(h) == dir) {
                s.hidden_games.push(game.dir.clone());
            }
        }
        P::save_settings(&st.data_dir, &s)?;
        *st.settings.lock().unwrap() = s;

        rescan(st.inner())
    })
    .await
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
    app: AppHandle,
    id: String,
    path: String,
    migrate: bool,
) -> Result<Vec<Game>, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
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
            ensure_not_running(&game, "迁移安装")?;
            let settings = st.settings.lock().unwrap().clone();
            if settings.keep_snapshot {
                let _ = installer::snapshot(&st.data_dir, &old_dir, &game.name);
            }
            installer::uninstall(&old_dir).map_err(|e| format!("卸载旧目录失败：{e}"))?;
        }

        let mut s = st.settings.lock().unwrap().clone();
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
        P::save_settings(&st.data_dir, &s)?;
        *st.settings.lock().unwrap() = s;

        rescan(st.inner())
    })
    .await
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
async fn read_opti_tuning(app: AppHandle, id: String) -> Result<tuning::Snapshot, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        let mut snap = tuning::read(Path::new(&game.dir))?;
        // 实时判定，而不是用上次扫描的结果
        snap.running = scan::game_running(Path::new(&game.dir), &game.exe);
        Ok(snap)
    })
    .await
}

#[tauri::command]
async fn write_opti_tuning(
    app: AppHandle,
    id: String,
    section: String,
    key: String,
    value: String,
) -> Result<String, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        ensure_not_running(&game, "调整参数")?;
        tuning::write(Path::new(&game.dir), &section, &key, &value)
    })
    .await
}

#[tauri::command]
async fn precheck_install(app: AppHandle, id: String) -> Result<Vec<precheck::Issue>, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        let warn_anticheat = st.settings.lock().unwrap().warn_anti_cheat;
        Ok(precheck::run(Path::new(&game.dir), warn_anticheat))
    })
    .await
}

#[tauri::command]
async fn rename_game(app: AppHandle, id: String, name: Option<String>) -> Result<Vec<Game>, String> {
    // 改名会重抓封面（网络），必须在 blocking 线程里做
    run_blocking(move || {
        let st = app.state::<AppState>();
        ensure_game(st.inner(), &id)?;

        let trimmed = name.unwrap_or_default().trim().to_string();
        let mut s = st.settings.lock().unwrap().clone();
        if trimmed.is_empty() {
            s.renames.remove(&id);
        } else {
            s.renames.insert(id.clone(), trimmed);
        }
        P::save_settings(&st.data_dir, &s)?;
        *st.settings.lock().unwrap() = s;

        let games = rescan(st.inner())?;

        // 名字变了，旧封面就不再对应，直接重抓
        if let Some(game) = games.iter().find(|g| g.id == id).cloned() {
            let dir = st.data_dir.join("covers");
            let source = st.settings.lock().unwrap().cover_source.clone();
            covers::remove(&dir, &id);
            if let Ok(path) = covers::ensure_cover(&dir, &id, &game.name, &source) {
                let uri = covers::data_uri(&path);
                if let Some(g) = st.games.lock().unwrap().iter_mut().find(|g| g.id == id) {
                    g.cover = uri.clone();
                }
                let _ = app.emit("cover", serde_json::json!({ "id": id, "cover": uri }));
            }
        }

        let games = st.games.lock().unwrap().clone();
        Ok(games)
    })
    .await
}

#[tauri::command]
async fn refresh_cover(app: AppHandle, id: String) -> Result<Option<String>, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        let dir = st.data_dir.join("covers");
        let source = st.settings.lock().unwrap().cover_source.clone();
        covers::remove(&dir, &id);
        let path = covers::ensure_cover(&dir, &id, &game.name, &source)?;
        let uri = covers::data_uri(&path);
        if let Some(g) = st.games.lock().unwrap().iter_mut().find(|g| g.id == id) {
            g.cover = uri.clone();
        }
        let _ = app.emit("cover", serde_json::json!({ "id": id, "cover": uri }));
        Ok(uri)
    })
    .await
}

#[tauri::command]
fn cover_provider() -> String {
    covers::provider().to_string()
}

#[tauri::command]
async fn delete_version(app: AppHandle, version: String) -> Result<u64, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let cache = PathBuf::from(&st.settings.lock().unwrap().cache_dir);
        let dir = cache.join(&version);
        if !dir.exists() {
            return Err("这个版本没有缓存".into());
        }
        let freed = dir_size(&dir);
        fs::remove_dir_all(&dir).map_err(|e| format!("删除缓存失败：{e}"))?;
        Ok(freed)
    })
    .await
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
async fn add_manual_game(app: AppHandle, path: String) -> Result<Option<String>, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let dir = PathBuf::from(&path);
        if !dir.is_dir() {
            return Err("这个路径不是一个文件夹".into());
        }
        if scan::find_exe(&dir, None).is_none() {
            return Err("这个文件夹里没有找到 .exe，请选择游戏主程序所在的文件夹".into());
        }
        let mut s = st.settings.lock().unwrap().clone();
        if !s.manual_games.iter().any(|p| p.eq_ignore_ascii_case(&path)) {
            s.manual_games.push(path);
            P::save_settings(&st.data_dir, &s)?;
            *st.settings.lock().unwrap() = s;
        }
        Ok(precheck::steamapps_warning(&dir))
    })
    .await
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
async fn list_releases(app: AppHandle, refresh: Option<bool>) -> Result<Vec<Version>, String> {
    // 这里可能真的要联网（refresh 或缓存过期），不能留在主线程
    run_blocking(move || {
        let st = app.state::<AppState>();
        let rels = get_releases(st.inner(), refresh.unwrap_or(false))?;
        let cache = PathBuf::from(&st.settings.lock().unwrap().cache_dir);
        let catalog = st.catalog.lock().unwrap().clone();
        // 游戏里正在用的旧版本也要留在列表里，否则清掉缓存后就看不见、也回不去了
        let installed: Vec<String> = st
            .games
            .lock()
            .unwrap()
            .iter()
            .filter_map(|g| g.anr.as_ref().map(|a| a.version.clone()))
            .filter(|v| v != "未知" && !v.is_empty())
            .collect();
        Ok(releases::to_versions(&rels, &cache, &catalog, &installed))
    })
    .await
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
    proxy: Option<&str>,
) -> Result<PathBuf, String> {
    emit(app, task, "安装", name, "前置检查", 0.04, "running");

    if !dir.is_dir() {
        return Err("游戏目录不存在".into());
    }
    let exe = scan::find_exe(dir, None).ok_or("这个目录里没有找到 .exe")?;
    let exe_name = exe
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    // 实时复查：进程名 + 代理 DLL 占用
    if scan::game_running(dir, &exe_name) {
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

    emit(app, task, "安装", name, "执行安装脚本", 0.9, "running");
    installer::run_install(&pkg, dir, proxy, &|| canceled(task)).map_err(script_error_text)?;

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
    ensure_not_running(&game, "安装")?;
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
    let keep_snapshot = settings.keep_snapshot;
    let limit = settings.concurrency;
    let data_dir = state.data_dir.clone();

    let result = run_blocking(move || -> Result<PathBuf, String> {
        let _ticket = task_gate(&app2, &task2, "安装", &name2, limit)?;
        // 快照之前先确认游戏没在跑（拿到名额后可能又被启动）
        let exe = scan::find_exe(&dir, None)
            .map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default())
            .unwrap_or_default();
        if scan::game_running(&dir, &exe) {
            return Err(format!("游戏正在运行（{exe}），请先关闭游戏"));
        }
        // 快照放在任务闸门之内：队列里等待的任务不会拍到别人写到一半的目录
        let snap: Option<PathBuf> = if had_install && keep_snapshot {
            emit(&app2, &task2, "安装", &name2, "生成快照", 0.1, "running");
            installer::snapshot(&data_dir, &dir, &name2).ok()
        } else {
            None
        };
        match do_install(
            &app2, &task2, &name2, &dir, &cache, &kind, &target, &mirror,
            force.unwrap_or(false), proxy_hint(&dir),
        ) {
            Ok(pkg) => Ok(pkg),
            Err(e) => {
                let (msg, rolled) = rollback_if_possible(snap.as_deref(), &dir, e);
                Err(failure_message(msg, snap.as_deref(), rolled))
            }
        }
    })
    .await;

    match result {
        Ok(path) => {
            record_fingerprint(state.inner(), &path, &target_out);
            let _ = rescan(state.inner());
            Ok(())
        }
        Err(full) => {
            let _ = rescan(state.inner());
            emit(&app, &task, "安装", &name, &failure_ui_text(&full), 1.0, err_status(&full));
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
    // 卸载会删目录里的文件：正在运行的游戏必须硬拦，否则只会留下半套文件
    ensure_not_running(&game, "卸载")?;
    let settings = state.settings.lock().unwrap().clone();
    let data = state.data_dir.clone();
    let dir = PathBuf::from(&game.dir);
    let exe = game.exe.clone();
    let name = game.name.clone();
    let task = format!("task-{}", TASK_SEQ.fetch_add(1, Ordering::Relaxed));
    let app2 = app.clone();
    let task2 = task.clone();
    let name2 = name.clone();
    let limit = settings.concurrency;

    let result = run_blocking(move || -> Result<(), String> {
        let _ticket = task_gate(&app2, &task2, "卸载", &name2, limit)?;
        // 排队期间游戏可能被启动：拿到名额后再查一次
        if scan::game_running(&dir, &exe) {
            return Err(format!("{name2} 正在运行，请先退出游戏再卸载"));
        }
        if settings.keep_snapshot {
            emit(&app2, &task2, "卸载", &name2, "生成快照", 0.2, "running");
            installer::snapshot(&data, &dir, &name2)?;
        }
        emit(&app2, &task2, "卸载", &name2, "执行卸载", 0.7, "running");
        installer::uninstall(&dir)?;
        emit(&app2, &task2, "卸载", &name2, "校验还原", 1.0, "done");
        Ok(())
    })
    .await;

    match result {
        Ok(()) => {
            let _ = rescan(state.inner());
            Ok(())
        }
        Err(e) => {
            let _ = rescan(state.inner());
            emit(&app, &task, "卸载", &name, &e, 1.0, err_status(&e));
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
    let exe = game.exe.clone();
    let name = game.name.clone();
    let kind = anr.kind.clone();
    let tag2 = tag.clone();
    let task = format!("task-{}", TASK_SEQ.fetch_add(1, Ordering::Relaxed));
    let data_dir = state.data_dir.clone();
    let keep_snapshot = settings.keep_snapshot;
    let dir2 = dir.clone();
    let task3 = task.clone();
    let name3 = name.clone();
    let app3 = app.clone();
    let limit = settings.concurrency;

    let result = run_blocking(move || -> Result<PathBuf, String> {
        let _ticket = task_gate(&app3, &task3, "切换版本", &name3, limit)?;
        // 覆盖安装前先确认游戏没在跑：在跑的话连快照都不该打（目录正被占用）
        if scan::game_running(&dir2, &exe) {
            return Err("游戏正在运行，请先退出游戏再切换版本".into());
        }
        // 覆盖安装前先留一份快照，失败时能回到切换前的状态
        let snap: Option<PathBuf> = if keep_snapshot {
            emit(&app3, &task3, "切换版本", &name3, "生成快照", 0.05, "running");
            installer::snapshot(&data_dir, &dir2, &name3).ok()
        } else {
            None
        };
        let fail = |msg: String| -> String {
            let (msg, rolled) = rollback_if_possible(snap.as_deref(), &dir2, msg);
            failure_message(msg, snap.as_deref(), rolled)
        };
        emit(&app3, &task3, "切换版本", &name3, "准备目标版本", 0.1, "running");
        let pkg = installer::ensure_package(
            &cache,
            &tag2,
            &kind,
            &mirror,
            |stage, p| emit(&app3, &task3, "切换版本", &name3, stage, 0.1 + p * 0.7, "running"),
            &|| canceled(&task3),
        )
        .map_err(|e| fail(e))?;
        if canceled(&task3) {
            return Err("已取消".into());
        }
        emit(&app3, &task3, "切换版本", &name3, "覆盖安装", 0.9, "running");
        installer::run_install(&pkg, &dir2, proxy_hint(&dir2), &|| canceled(&task3))
            .map_err(|e| fail(script_error_text(e)))?;
        emit(&app3, &task3, "切换版本", &name3, "指纹校验", 1.0, "done");
        Ok(pkg)
    })
    .await;

    match result {
        Ok(path) => {
            record_fingerprint(state.inner(), &path, &tag);
            let _ = rescan(state.inner());
            Ok(())
        }
        Err(full) => {
            let _ = rescan(state.inner());
            emit(
                &app,
                &task,
                "切换版本",
                &name,
                &failure_ui_text(&full),
                1.0,
                err_status(&full),
            );
            Err(full)
        }
    }
}

#[tauri::command]
async fn set_fg_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
    force: Option<bool>,
) -> Result<(), String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        require_confirmation(&game, force)?;
        // 和调参页一样：OptiScaler 退出时会把内存里的设置写回 ini，
        // 游戏运行时改开关只会被覆盖，不如直接拒绝。
        ensure_not_running(&game, "改帧生成开关")?;
        let dir = PathBuf::from(&game.dir);
        let ini_p = inspect::ini_path(&dir);
        if !ini_p.exists() {
            return Err("这个目录没有 OptiScaler.ini，无法写入开关".into());
        }
        let mut doc = IniDoc::load(&ini_p);
        if doc.is_lossy() {
            return Err("OptiScaler.ini 不是 UTF-8 编码，为免写坏注释已拒绝写入".into());
        }
        IniDoc::backup_once(&ini_p);
        doc.set("FrameGen", "Enabled", if enabled { "true" } else { "false" });
        doc.save(&ini_p)
            .map_err(|e| format!("写入 OptiScaler.ini 失败：{e}"))?;
        let _ = rescan(st.inner());
        Ok(())
    })
    .await
}

#[tauri::command]
async fn write_config(
    app: AppHandle,
    id: String,
    config: NrFgConfig,
) -> Result<(), String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        ensure_not_running(&game, "改参数")?;
        let dir = PathBuf::from(&game.dir);
        let ini_p = inspect::nrf_g_ini(&dir);
        if !ini_p.exists() {
            return Err("找不到 nr_fg.ini".into());
        }
        let mut doc = IniDoc::load(&ini_p);
        if doc.is_lossy() {
            return Err("nr_fg.ini 不是 UTF-8 编码，为免写坏注释已拒绝写入".into());
        }
        IniDoc::backup_once(&ini_p);
        doc.set_flat("path", &config.path);
        doc.set_flat("model_size", &config.model_size);
        doc.set_flat("nits_per_unit", &config.nits.to_string());
        doc.set_flat("max_generated", &config.max_generated.to_string());
        doc.set_flat("scene_cut", if config.scene_cut { "1" } else { "0" });
        doc.save(&ini_p)
            .map_err(|e| format!("写入 nr_fg.ini 失败：{e}"))?;
        let _ = rescan(st.inner());
        Ok(())
    })
    .await
}

#[tauri::command]
async fn set_fg_replacement(
    app: AppHandle,
    id: String,
    mode: String,
    force: Option<bool>,
) -> Result<(), String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        require_confirmation(&game, force)?;
        ensure_not_running(&game, "切换帧生成实现")?;
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
        if doc.is_lossy() {
            return Err("OptiScaler.ini 不是 UTF-8 编码，为免写坏注释已拒绝写入".into());
        }
        IniDoc::backup_once(&ini_p);
        doc.set("FrameGen", "FGNvngxReplacement", value);
        doc.save(&ini_p)
            .map_err(|e| format!("写入 OptiScaler.ini 失败：{e}"))?;
        let _ = rescan(st.inner());
        Ok(())
    })
    .await
}

/// 安装体检：返回这个游戏当前所有的配置/文件问题（供抽屉里的「体检」区块展示）
#[tauri::command]
async fn health_check(app: AppHandle, id: String) -> Result<Vec<health::Issue>, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        Ok(health::check(Path::new(&game.dir)))
    })
    .await
}

/// 一键修复：补写 ini 关键项、从备份还原代理 DLL / OptiScaler.ini。
/// 修不了的（provider 整个丢了）会通过 `needsReinstall` 告诉前端去重装。
#[tauri::command]
async fn repair_install(app: AppHandle, id: String) -> Result<health::RepairReport, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        ensure_not_running(&game, "修复安装")?;
        let dir = PathBuf::from(&game.dir);
        let report = health::repair(&dir);
        let _ = rescan(st.inner());
        Ok(report)
    })
    .await
}

#[tauri::command]
async fn get_log(app: AppHandle, id: String) -> Result<String, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        logs::read_log(Path::new(&game.dir), 400)
    })
    .await
}

#[tauri::command]
async fn make_diag(app: AppHandle, id: String) -> Result<String, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        let json = serde_json::to_string_pretty(&game).unwrap_or_default();
        let out = logs::diag_bundle(&st.data_dir, Path::new(&game.dir), &game.name, &json)?;
        Ok(out.to_string_lossy().to_string())
    })
    .await
}

/// P1-5：增量读取日志。前端每 1 秒带上一轮的 `offset` 与 `path` 来拉新内容。
#[tauri::command]
async fn read_log_since(
    app: AppHandle,
    id: String,
    offset: Option<u64>,
    path: Option<String>,
) -> Result<logs::LogChunk, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        logs::read_since(Path::new(&game.dir), offset, path.as_deref())
    })
    .await
}

/// P0-4：还没处理的崩溃报告（新的在前）。启动时与打开「关于」时都可以问一次。
#[tauri::command]
fn crash_list(state: State<'_, AppState>) -> Vec<crash::CrashReport> {
    crash::list_pending(&state.data_dir)
}

/// 用户「忽略」后写 `.ack`，下次启动不再提示。
#[tauri::command]
fn crash_ack(state: State<'_, AppState>, paths: Vec<String>) -> Result<(), String> {
    crash::ack(&state.data_dir, &paths)
}

/// 前端渲染异常 / window.onerror / unhandledrejection 也落一份同样的报告。
#[tauri::command]
fn report_frontend_error(
    state: State<'_, AppState>,
    message: String,
    detail: String,
) -> Result<String, String> {
    crash::report(&state.data_dir, "frontend", &message, &detail)
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "写入崩溃报告失败".to_string())
}

/// 让崩溃报告带上「当前视图 / 最近任务」，排查时能直接定位到操作。
#[tauri::command]
fn set_crash_context(view: String, task: Option<String>) {
    crash::set_context(&view, task.as_deref());
}

/// P0-4：崩溃诊断包（所有崩溃报告 + 系统信息 + manifest）。
#[tauri::command]
async fn make_crash_diag(app: AppHandle) -> Result<String, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let out = logs::crash_bundle(&st.data_dir)?;
        Ok(out.to_string_lossy().to_string())
    })
    .await
}

/// P1-8：手动指定代理 DLL。改动代理名等于一次完整重装：
/// 确认未运行 → 快照 → 用目标名字重跑官方脚本 → 校验 `nr_fg_install.json`。
#[tauri::command]
async fn set_proxy(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    dll: String,
    force: Option<bool>,
) -> Result<(), String> {
    const ALLOWED: [&str; 3] = ["dxgi.dll", "winmm.dll", "version.dll"];
    let dll = dll.trim().to_ascii_lowercase();
    if !ALLOWED.contains(&dll.as_str()) {
        return Err("代理 DLL 只支持 dxgi.dll / winmm.dll / version.dll".into());
    }
    let game = ensure_game(state.inner(), &id)?;
    require_confirmation(&game, force)?;
    let Some(anr) = game.anr.clone() else {
        return Err("这个游戏没有安装 ANR".into());
    };
    if anr.proxy.eq_ignore_ascii_case(&dll) {
        return Ok(());
    }
    ensure_not_running(&game, "更换代理 DLL")?;

    let settings = state.settings.lock().unwrap().clone();
    let cache = PathBuf::from(&settings.cache_dir);
    let mirror = settings.mirror.clone();
    let kind = anr.kind.clone();
    let tag = if anr.version != "未知" && !anr.version.is_empty() {
        anr.version.clone()
    } else if !settings.default_version.trim().is_empty() {
        settings.default_version.trim().to_string()
    } else {
        get_releases(state.inner(), false)?
            .first()
            .map(|r| r.tag.clone())
            .ok_or("无法获取最新版本")?
    };
    let dir = PathBuf::from(&game.dir);
    let exe = game.exe.clone();
    let name = game.name.clone();
    let data_dir = state.data_dir.clone();
    let keep_snapshot = settings.keep_snapshot;
    let limit = settings.concurrency;
    let task = format!("task-{}", TASK_SEQ.fetch_add(1, Ordering::Relaxed));
    let app2 = app.clone();
    let task2 = task.clone();
    let name2 = name.clone();
    let dll2 = dll.clone();
    let tag2 = tag.clone();
    let kind2 = kind.clone();

    let result = run_blocking(move || -> Result<PathBuf, String> {
        let _ticket = task_gate(&app2, &task2, "更换代理 DLL", &name2, limit)?;
        if scan::game_running(&dir, &exe) {
            return Err("游戏正在运行，请先退出游戏再更换代理 DLL".into());
        }
        let snap: Option<PathBuf> = if keep_snapshot {
            emit(&app2, &task2, "更换代理 DLL", &name2, "生成快照", 0.05, "running");
            installer::snapshot(&data_dir, &dir, &name2).ok()
        } else {
            None
        };
        let fail = |msg: String| -> String {
            let (msg, rolled) = rollback_if_possible(snap.as_deref(), &dir, msg);
            failure_message(msg, snap.as_deref(), rolled)
        };
        emit(&app2, &task2, "更换代理 DLL", &name2, "准备安装包", 0.1, "running");
        let pkg = installer::ensure_package(
            &cache,
            &tag2,
            &kind2,
            &mirror,
            |stage, p| emit(&app2, &task2, "更换代理 DLL", &name2, stage, 0.1 + p * 0.7, "running"),
            &|| canceled(&task2),
        )
        .map_err(|e| fail(e))?;
        if canceled(&task2) {
            return Err("已取消".into());
        }
        emit(&app2, &task2, "更换代理 DLL", &name2, "重新安装", 0.9, "running");
        installer::run_install(&pkg, &dir, Some(&dll2), &|| canceled(&task2))
            .map_err(|e| fail(script_error_text(e)))?;

        // 后置校验：安装记录里的 proxy 必须真的变成用户选的名字
        let rec = fs::read_to_string(inspect::record_path(&dir)).unwrap_or_default();
        let got = serde_json::from_str::<serde_json::Value>(&rec)
            .ok()
            .and_then(|v| v.get("proxy").and_then(|x| x.as_str()).map(|s| s.to_string()));
        if got.as_deref().map(|g| !g.eq_ignore_ascii_case(&dll2)).unwrap_or(true) {
            return Err(fail(format!(
                "安装脚本执行完成，但安装记录里的代理仍是 {}，不是 {dll2}",
                got.unwrap_or_else(|| "空".into())
            )));
        }
        emit(&app2, &task2, "更换代理 DLL", &name2, "已完成", 1.0, "done");
        Ok(pkg)
    })
    .await;

    match result {
        Ok(path) => {
            record_fingerprint(state.inner(), &path, &tag);
            let _ = rescan(state.inner());
            Ok(())
        }
        Err(full) => {
            let _ = rescan(state.inner());
            emit(
                &app,
                &task,
                "更换代理 DLL",
                &name,
                &failure_ui_text(&full),
                1.0,
                err_status(&full),
            );
            Err(full)
        }
    }
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct UpdateCheck {
    current: String,
    available: bool,
    version: String,
    notes: String,
}

fn update_error_text(e: impl std::fmt::Display) -> String {
    let raw = e.to_string();
    let lower = raw.to_ascii_lowercase();
    if raw.contains("No updater endpoints") {
        return "没有配置更新源（tauri.conf.json → plugins.updater.endpoints）".into();
    }
    // 还没发布 latest.json 时 GitHub 会 404，插件把它报成「release JSON 无效」。
    // 这不是故障，也不该吓用户：说明清楚「还没上传」即可。
    if lower.contains("valid release json")
        || lower.contains("could not fetch")
        || lower.contains("status code 404")
        || lower.contains("http 404")
    {
        return "还没有可用的更新信息：GitHub Releases 上尚未发布 latest.json".into();
    }
    if raw.contains("error sending request")
        || raw.contains("dns")
        || raw.contains("connect")
        || raw.contains("timed out")
    {
        return format!("更新检查失败：网络不可达（{raw}）");
    }
    format!("更新检查失败：{raw}")
}

const UPDATE_MANIFEST: &str =
    "https://github.com/Utmotc/ANR-Studio/releases/latest/download/latest.json";

/// 更新源听「下载源」设置：镜像模式走 gh-proxy，直连/自动走 GitHub 本体。
/// 验签用的是内置公钥，跟走哪条线路无关，所以换源不影响安全性。
fn updater_for(app: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let mirror = app.state::<AppState>().settings.lock().unwrap().mirror.clone();
    let mut builder = app
        .updater_builder()
        .timeout(Duration::from_secs(30));
    if mirror == "mirror" {
        let url: tauri::Url = format!("https://gh-proxy.com/{UPDATE_MANIFEST}")
            .parse()
            .map_err(|e| format!("更新源地址无效：{e}"))?;
        builder = builder
            .endpoints(vec![url])
            .map_err(|e| format!("更新源配置失败：{e}"))?;
    }
    builder.build().map_err(update_error_text)
}

/// P1-3：检查更新。离线/未配置时返回 Err，前端静默处理（手动检查才提示）。
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<UpdateCheck, String> {
    let updater = updater_for(&app)?;
    let found = updater.check().await.map_err(update_error_text)?;
    Ok(match found {
        Some(u) => UpdateCheck {
            current: env!("CARGO_PKG_VERSION").to_string(),
            available: true,
            version: u.version.clone(),
            notes: u.body.clone().unwrap_or_default(),
        },
        None => UpdateCheck {
            current: env!("CARGO_PKG_VERSION").to_string(),
            available: false,
            version: String::new(),
            notes: String::new(),
        },
    })
}

/// P1-3：下载 + 验签 + 安装更新；进度走任务中心的事件通道，完成后重启。
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let updater = updater_for(&app)?;
    let update = updater
        .check()
        .await
        .map_err(update_error_text)?
        .ok_or_else(|| "已经是最新版本".to_string())?;
    let version = update.version.clone();
    let task = format!("task-{}", TASK_SEQ.fetch_add(1, Ordering::Relaxed));
    emit(&app, &task, "更新", &version, "正在下载更新包", 0.02, "running");

    let app_dl = app.clone();
    let task_dl = task.clone();
    let version_dl = version.clone();
    let app_inst = app.clone();
    let task_inst = task.clone();
    let version_inst = version.clone();
    let mut last_reported = 0u32;
    update
        .download_and_install(
            move |chunk: usize, total: Option<u64>| {
                if let Some(total) = total {
                    if total > 0 {
                        let done = chunk as f64 / total as f64;
                        // 每 2% 报一次，别把任务事件刷爆
                        let pct = (done * 100.0) as u32;
                        if pct >= last_reported + 2 {
                            last_reported = pct;
                            emit(
                                &app_dl,
                                &task_dl,
                                "更新",
                                &version_dl,
                                &format!("下载中 {pct}%"),
                                0.02 + done * 0.88,
                                "running",
                            );
                        }
                    }
                }
            },
            move || emit(&app_inst, &task_inst, "更新", &version_inst, "正在安装更新", 0.95, "running"),
        )
        .await
        .map_err(|e| {
            let msg = update_error_text(e);
            emit(&app, &task, "更新", &version, &msg, 1.0, "error");
            msg
        })?;

    emit(&app, &task, "更新", &version, "更新已就绪，即将重启", 1.0, "done");
    std::thread::sleep(Duration::from_millis(400));
    app.restart();
}

#[tauri::command]
fn restart_app(app: AppHandle) {
    app.restart();
}

/// P0-1：列出这个游戏的快照（新快照按 gameId 匹配，旧快照按文件名前缀兜底）
#[tauri::command]
async fn list_snapshots(app: AppHandle, id: String) -> Result<Vec<SnapshotInfo>, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        installer::list_snapshots(&st.data_dir, &game.id, &game.name)
    })
    .await
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
    // 实时复查（缓存里的 running 可能已经过期）
    if scan::game_running(Path::new(&game.dir), &game.exe) {
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
    let exe = game.exe.clone();
    run_blocking(move || {
        if scan::game_running(&dir, &exe) {
            return Err("游戏正在运行，请先退出游戏再恢复快照".into());
        }
        installer::restore_snapshot(&snap_canon, &dir)
    })
    .await?;

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
async fn mark_version(app: AppHandle, id: String, tag: String) -> Result<Vec<Game>, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let game = ensure_game(st.inner(), &id)?;
        let dll = inspect::provider_dll(Path::new(&game.dir));
        let sha = inspect::sha256_file_cached(&dll)
            .ok_or("这个游戏没有找到 provider DLL，无法标记版本")?;
        let clean = tag.trim();
        if clean.is_empty() {
            return Err("请选择要标记的版本".into());
        }
        {
            let mut cat = st.catalog.lock().unwrap();
            cat.insert(sha, clean.to_string());
            P::save_catalog(&st.data_dir, &cat)?;
        }
        rescan(st.inner())
    })
    .await
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
async fn check_package_structure(app: AppHandle) -> Vec<String> {
    let handle = app.clone();
    match run_blocking(move || {
        let st = handle.state::<AppState>();
        let cache = PathBuf::from(&st.settings.lock().unwrap().cache_dir);
        Ok(installer::check_cached_packages(&cache))
    })
    .await
    {
        Ok(v) => v,
        Err(_) => Vec::new(),
    }
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
async fn clear_cache(app: AppHandle) -> Result<u64, String> {
    run_blocking(move || {
        let st = app.state::<AppState>();
        let cache = PathBuf::from(&st.settings.lock().unwrap().cache_dir);
        if !cache.exists() {
            return Ok(0);
        }
        let freed = dir_size(&cache);
        fs::remove_dir_all(&cache).map_err(|e| format!("清理缓存失败：{e}"))?;
        let _ = fs::create_dir_all(&cache);
        Ok(freed)
    })
    .await
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

#[cfg(windows)]
mod shell {
    #[link(name = "shell32")]
    unsafe extern "system" {
        pub fn ShellExecuteW(
            hwnd: *mut core::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> isize;
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 只放行 http(s)，且不接受控制字符（防止用换行拼接别的参数）。
fn allowed_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://"))
        && !url.chars().any(|c| c.is_control())
}

/// P1-9：外链不再拼进 `cmd /C start "" <url>`。
///
/// URL 里带 `&` 会被 cmd 当成命令分隔符（`https://a.com/?x=1&y=2` 会多执行一段），
/// 这里直接调 `ShellExecuteW`，不经过 shell 解析；并且只放行 http(s)。
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let url = url.trim();
    if !allowed_url(url) {
        return Err("只允许打开 http(s) 链接，且不能包含控制字符".into());
    }
    #[cfg(windows)]
    {
        let op = wide("open");
        let target = wide(url);
        // 返回值 > 32 才算成功（ShellExecute 的历史约定）
        let code = unsafe {
            shell::ShellExecuteW(
                std::ptr::null_mut(),
                op.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if code <= 32 {
            return Err(format!("打开链接失败（ShellExecute 返回 {code}）"));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("只支持 Windows".into())
    }
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

/// 与 `tauri.conf.json` 的 `identifier` 保持一致；用于在 Tauri 初始化之前推算数据目录。
const APP_ID: &str = "app.anrstudio.desktop";

/// 提前推算数据目录（不依赖 Tauri 的初始化）。
///
/// Tauri 会**先创建窗口/WebView，再调用 setup**（`tauri::app::setup`）。如果 WebView2
/// 初始化失败，setup 根本不会执行——而「双击没反应」正是这种场景。所以 panic hook
/// 必须在进入事件循环之前就挂上。
fn pre_setup_data_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let base = std::env::var_os("LOCALAPPDATA")?;
        Some(PathBuf::from(base).join(APP_ID).join("anr-studio"))
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
        Some(base.join(APP_ID).join("anr-studio"))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 越早越好：窗口创建失败也能落一份崩溃报告
    if let Some(data) = pre_setup_data_dir() {
        let _ = fs::create_dir_all(&data);
        crash::install_hook(&data);
    }

    tauri::Builder::default()
        // 单实例插件必须最先注册：第二个实例在初始化窗口之前就应该退出
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 托盘隐藏状态下再双击一次，应把已有窗口显示出来
            show_main(app);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let data = app
                .path()
                .app_local_data_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("anr-studio");
            let _ = fs::create_dir_all(&data);
            // 越早挂越好：之后任何线程 panic 都能落盘
            crash::install_hook(&data);
            let mut settings = P::load_settings(&data);
            let catalog = P::load_catalog(&data);

            #[cfg(debug_assertions)]
            if std::env::var("ANR_CRASH_TEST").is_ok() {
                std::thread::spawn(|| {
                    std::thread::sleep(Duration::from_secs(2));
                    panic!("ANR_CRASH_TEST：手动触发的崩溃测试");
                });
            }

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
            get_running_ids,
            health_check,
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
            read_log_since,
            make_diag,
            crash_list,
            crash_ack,
            report_frontend_error,
            set_crash_context,
            make_crash_diag,
            set_proxy,
            check_update,
            install_update,
            restart_app,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_http_urls_are_allowed() {
        assert!(allowed_url("https://a.com/?x=1&y=2"));
        assert!(allowed_url("http://a.com"));
        assert!(!allowed_url("file:///C:/Windows/System32/calc.exe"));
        assert!(!allowed_url("javascript:alert(1)"));
        assert!(!allowed_url("https://a.com/\ncalc.exe"));
    }

    #[test]
    fn update_errors_are_explained_in_chinese() {
        // GitHub 还没有 latest.json 时插件会报这句，不该显示成「检查失败」
        let missing = update_error_text("Could not fetch a valid release JSON from the remote");
        assert!(missing.contains("尚未发布 latest.json"), "{missing}");
        assert!(!missing.contains("检查失败"), "{missing}");
        // 断网仍然是失败，但要指出是网络
        let offline = update_error_text("error sending request: dns error");
        assert!(offline.contains("网络不可达"), "{offline}");
    }
}
