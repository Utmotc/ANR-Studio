//! 安装 / 下载任务闸门（设计文档 6.5.4）。
//!
//! 多个安装器同时写文件会互相踩，所以所有重活都要先拿到一张「票」。
//! `concurrency` 决定同时能有几张票：1 = 严格串行，>1 时按设置放行。
//! 排队期间可以被取消，票用 RAII 归还，任务 panic / 提前返回也不会漏。

use std::sync::{Condvar, Mutex, OnceLock};
use std::time::Duration;

struct Gate {
    active: Mutex<usize>,
    cv: Condvar,
}

fn gate() -> &'static Gate {
    static GATE: OnceLock<Gate> = OnceLock::new();
    GATE.get_or_init(|| Gate {
        active: Mutex::new(0),
        cv: Condvar::new(),
    })
}

/// 持有期间占用一个名额，Drop 时自动释放并唤醒排队者。
pub struct Ticket;

impl Drop for Ticket {
    fn drop(&mut self) {
        let g = gate();
        let mut n = g.active.lock().unwrap();
        *n = n.saturating_sub(1);
        drop(n);
        g.cv.notify_all();
    }
}

/// `limit` 来自设置；`cancel` 在排队时也能生效；`on_wait` 只在第一次进入排队时调用一次。
pub fn acquire(
    limit: u32,
    cancel: &dyn Fn() -> bool,
    on_wait: &dyn Fn(),
) -> Result<Ticket, String> {
    let limit = limit.clamp(1, 8) as usize;
    let g = gate();
    let mut announced = false;
    loop {
        if cancel() {
            return Err("已取消".into());
        }
        let mut n = g.active.lock().unwrap();
        if *n < limit {
            *n += 1;
            return Ok(Ticket);
        }
        if !announced {
            announced = true;
            on_wait();
        }
        // 超时醒来复查取消标记，避免取消后一直睡在队列里
        let (guard, _) = g
            .cv
            .wait_timeout(n, Duration::from_millis(250))
            .unwrap();
        drop(guard);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn limit_one_serializes() {
        let peak = Arc::new(AtomicUsize::new(0));
        let live = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let peak = peak.clone();
            let live = live.clone();
            handles.push(thread::spawn(move || {
                let _t = acquire(1, &|| false, &|| {}).unwrap();
                let now = live.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(20));
                live.fetch_sub(1, Ordering::SeqCst);
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn queued_task_can_be_canceled() {
        let holder = acquire(1, &|| false, &|| {}).unwrap();
        let flag = Arc::new(AtomicBool::new(false));
        let waited = Arc::new(AtomicUsize::new(0));
        let f = flag.clone();
        let w = waited.clone();
        let h = thread::spawn(move || {
            acquire(1, &|| f.load(Ordering::SeqCst), &|| {
                w.fetch_add(1, Ordering::SeqCst);
            })
        });
        thread::sleep(Duration::from_millis(60));
        assert_eq!(waited.load(Ordering::SeqCst), 1, "第二个任务应进入排队");
        flag.store(true, Ordering::SeqCst);
        drop(holder);
        assert!(h.join().unwrap().is_err(), "排队中的取消应返回 Err");
    }

    #[test]
    fn cancel_before_start_never_takes_a_slot() {
        assert!(acquire(1, &|| true, &|| {}).is_err());
        let t = acquire(1, &|| false, &|| {}).unwrap();
        drop(t);
    }
}
