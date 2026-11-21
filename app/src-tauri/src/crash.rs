//! P0-4 崩溃捕获与下次启动提示。
//!
//! release 用 `panic = "abort"`，闪退后进程直接消失，用户无法说明发生了什么。
//! 这里在进程启动时挂一个 panic hook：hook 在 abort **之前**执行，只做「拼一段文本 +
//! 写文件」，不调用 Tauri、不弹窗、不分配大对象，避免二次崩溃。
//!
//! 文件写在 `<数据目录>/crash/crash-<时间戳>.txt`；前端渲染异常走
//! [`report`]（kind = frontend）落到同一个目录。用户在界面上「忽略」后写同名的
//! `.ack`，下次启动就不再提示。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// 数据目录在 `setup` 里才知道；hook 提前安装时先空着，之后补上。
static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

#[derive(Default)]
struct Context {
    view: String,
    task: String,
}

static CONTEXT: OnceLock<Mutex<Context>> = OnceLock::new();

fn context() -> &'static Mutex<Context> {
    CONTEXT.get_or_init(|| Mutex::new(Context::default()))
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CrashReport {
    pub path: String,
    pub file_name: String,
    pub created_at: String,
    pub kind: String,
    pub message: String,
    pub location: String,
    pub size_bytes: u64,
}

pub fn crash_dir(data: &Path) -> PathBuf {
    data.join("crash")
}

/// 安装 panic hook（幂等）。必须在 `AppState` 建好之前/之后都可以调用。
pub fn install_hook(data_dir: &Path) {
    let _ = DATA_DIR.set(data_dir.to_path_buf());
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    std::panic::set_hook(Box::new(|info| {
        // 二次 panic（比如 hook 自己又炸了）直接放手，别再递归写文件
        static ENTERED: AtomicBool = AtomicBool::new(false);
        if ENTERED.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(dir) = DATA_DIR.get() else { return };
        let payload = info.payload();
        let message = if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "未知 panic".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "未知位置".to_string());
        let thread = std::thread::current()
            .name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unnamed".to_string());
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let detail = format!("location: {location}\nthread: {thread}\nbacktrace:\n{backtrace}");
        let _ = report_in(&crash_dir(dir), "panic", &message, &detail);
    }));
}

/// 由前端上报渲染异常 / `window.onerror` / `unhandledrejection`。
pub fn report(data: &Path, kind: &str, message: &str, detail: &str) -> Option<PathBuf> {
    report_in(&crash_dir(data), kind, message, detail).ok()
}

/// 更新「当前视图 / 最近任务」，崩溃报告里带上这两个能大幅缩小排查范围。
pub fn set_context(view: &str, task: Option<&str>) {
    if let Ok(mut c) = context().lock() {
        c.view = view.to_string();
        if let Some(t) = task {
            if !t.trim().is_empty() {
                c.task = t.to_string();
            }
        }
    }
}

fn stamp() -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now = chrono::Local::now();
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_millis())
        .unwrap_or(0);
    // 同一毫秒内连续上报时靠序号区分，避免后一份把前一份覆盖掉
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("{}-{ms:03}-{seq:04}", now.format("%Y%m%d-%H%M%S"))
}

fn report_in(dir: &Path, kind: &str, message: &str, detail: &str) -> std::io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let stamp = stamp();
    let file = dir.join(format!("crash-{stamp}.txt"));
    let (view, task) = context()
        .lock()
        .map(|c| (c.view.clone(), c.task.clone()))
        .unwrap_or_else(|_| (String::new(), String::new()));

    let mut text = String::with_capacity(1024 + detail.len());
    text.push_str("# ANR Studio 崩溃报告\n");
    text.push_str(&format!("kind: {kind}\n"));
    text.push_str(&format!("time: {}\n", chrono::Local::now().to_rfc3339()));
    text.push_str(&format!("version: {}\n", env!("CARGO_PKG_VERSION")));
    text.push_str(&format!("message: {}\n", one_line(message)));
    text.push_str(&format!("view: {view}\n"));
    text.push_str(&format!("task: {task}\n"));
    text.push_str("\n--- detail ---\n");
    text.push_str(detail);
    if !text.ends_with('\n') {
        text.push('\n');
    }

    // 先写临时文件再改名，避免用户恰好此时再开一个实例读到半截报告
    let tmp = dir.join(format!("crash-{stamp}.tmp"));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.flush()?;
    }
    fs::rename(&tmp, &file)?;
    Ok(file)
}

fn one_line(s: &str) -> String {
    s.lines().take(6).collect::<Vec<_>>().join(" / ")
}

fn read_header(text: &str) -> (String, String, String, String) {
    let mut kind = String::new();
    let mut message = String::new();
    let mut location = String::new();
    let mut time = String::new();
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("kind: ") {
            kind = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("message: ") {
            message = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("time: ") {
            time = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("location: ") {
            // 位置写在 detail 区（hook 才知道），这里整篇扫一次取第一条
            if location.is_empty() {
                location = v.trim().to_string();
            }
        }
    }
    (kind, message, location, time)
}

/// 未 ack 的崩溃报告，新的在前。
pub fn list_pending(data: &Path) -> Vec<CrashReport> {
    let dir = crash_dir(data);
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<CrashReport> = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("crash-") || !name.ends_with(".txt") {
            continue;
        }
        if dir.join(format!("{name}.ack")).exists() {
            continue;
        }
        let text = fs::read_to_string(&p).unwrap_or_default();
        let (kind, message, location, time) = read_header(&text);
        out.push(CrashReport {
            path: p.to_string_lossy().to_string(),
            file_name: name,
            created_at: time,
            kind,
            message,
            location,
            size_bytes: e.metadata().map(|m| m.len()).unwrap_or(0),
        });
    }
    out.sort_by(|a, b| b.file_name.cmp(&a.file_name));
    out
}

/// 标记已处理；只接受 `crash/` 目录内的文件，防止前端传任意路径把别处写成 .ack。
pub fn ack(data: &Path, paths: &[String]) -> Result<(), String> {
    let dir = crash_dir(data);
    let root = dir.canonicalize().unwrap_or_else(|_| dir.clone());
    for raw in paths {
        let p = PathBuf::from(raw.trim());
        let Ok(p) = p.canonicalize() else { continue };
        if !p.starts_with(&root) {
            return Err("只能标记崩溃目录里的报告".into());
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if !name.starts_with("crash-") || !name.ends_with(".txt") {
            return Err("不是崩溃报告文件".into());
        }
        fs::write(dir.join(format!("{name}.ack")), b"ack\n")
            .map_err(|e| format!("标记崩溃报告失败：{e}"))?;
    }
    Ok(())
}

/// 崩溃诊断包的内容：所有崩溃报告 + 系统信息，供用户直接发出去。

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_complete_report_and_parses_header() {
        let dir = crate::paths::test_dir("crash-report");
        set_context("logs", Some("安装 · Test Game"));
        let p = report_in(&dir.join("crash"), "panic", "boom", "location: src/lib.rs:1:1\nbacktrace:\n<x>")
            .unwrap();
        let text = fs::read_to_string(&p).unwrap();
        assert!(text.contains("version: "), "缺少版本号：{text}");
        assert!(text.contains("location: src/lib.rs:1:1"));
        assert!(text.contains("view: logs"));
        assert!(text.contains("task: 安装 · Test Game"));
        assert!(text.contains("boom"));
    }

    #[test]
    fn pending_hides_after_ack() {
        let data = crate::paths::test_dir("crash-ack");
        let crash_dir = data.join("crash");
        let p = report_in(&crash_dir, "frontend", "render failed", "stack").unwrap();
        let list = list_pending(&data);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, "frontend");
        assert_eq!(list[0].message, "render failed");
        ack(&data, &[p.to_string_lossy().to_string()]).unwrap();
        assert!(list_pending(&data).is_empty());
    }

    #[test]
    fn ack_rejects_outside_paths() {
        let data = crate::paths::test_dir("crash-ack-safe");
        let outside = crate::paths::test_dir("crash-outside").join("crash-1.txt");
        fs::write(&outside, "x").unwrap();
        assert!(ack(&data, &[outside.to_string_lossy().to_string()]).is_err());
    }

    #[test]
    fn panic_hook_writes_report_with_location_and_version() {
        let data = crate::paths::test_dir("crash-hook");
        install_hook(&data);
        let h = std::thread::spawn(|| {
            let _ = std::panic::catch_unwind(|| panic!("hook test boom"));
        });
        h.join().unwrap();

        let list = list_pending(&data);
        assert_eq!(list.len(), 1, "hook 应该落下一份报告：{list:?}");
        assert_eq!(list[0].kind, "panic");
        assert!(list[0].message.contains("hook test boom"));
        assert!(list[0].location.contains("crash.rs"), "{}", list[0].location);
        let text = fs::read_to_string(&list[0].path).unwrap();
        assert!(text.contains(&format!("version: {}", env!("CARGO_PKG_VERSION"))));
    }
}