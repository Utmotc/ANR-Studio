import { useState } from 'react';
import { AlertTriangle, Check, ChevronDown, Eraser, RotateCcw, X } from 'lucide-react';
import { clsx } from '../lib';
import { useApp } from '../store';
import type { Task } from '../types';
import { Progress } from './ui';

const STATUS: Record<Task['status'], { label: string; color: string }> = {
  running: { label: '进行中', color: 'var(--s-accent)' },
  done: { label: '已完成', color: 'var(--s-ok)' },
  error: { label: '失败', color: 'var(--s-danger)' },
  canceled: { label: '已取消', color: 'var(--s-warn)' },
};

function relMs(ms: number): string {
  const diff = Date.now() - ms;
  const min = Math.round(diff / 60000);
  if (min < 1) return '刚刚';
  if (min < 60) return `${min} 分钟前`;
  const h = Math.round(min / 60);
  if (h < 24) return `${h} 小时前`;
  return `${Math.round(h / 24)} 天前`;
}

/**
 * 任务中心：所有安装 / 卸载 / 切换 / 下载的历史。
 * 后端每个任务都有独立的 taskId，事件通过 `task://`（Tauri 事件名 `task`）推进度。
 */
export function TaskCenter() {
  const { tasks, tasksOpen, setTasksOpen, cancelTask, dismissTask, retryTask, clearFinishedTasks } =
    useApp();
  const [openId, setOpenId] = useState<string | null>(null);

  if (!tasksOpen) return null;
  const running = tasks.filter((t) => t.status === 'running').length;
  const finished = tasks.length - running;
  const list = [...tasks].reverse();

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-[85]">
      <div
        className="anim-overlay absolute inset-0 bg-black/40"
        onClick={() => setTasksOpen(false)}
        aria-hidden
      />
      <aside
        className="anim-drawer absolute right-0 top-0 flex h-full w-[460px] flex-col border-l border-line bg-surface shadow-2xl"
        aria-label="任务中心"
      >
        <div className="flex items-start gap-3 border-b border-line px-5 py-4">
          <div className="min-w-0 flex-1">
            <h2 className="text-[14px] font-medium leading-tight">任务</h2>
            <p className="mt-1 text-[11px] text-ink-3">
              {running > 0 ? `${running} 个进行中` : '没有正在跑的任务'}
              {finished > 0 && ` · ${finished} 条历史`}
            </p>
          </div>
          {finished > 0 && (
            <button
              className="btn btn-ghost h-7 px-2 text-[11px]"
              onClick={clearFinishedTasks}
              title="清除所有已结束的任务记录"
            >
              <Eraser size={12} />
              清除记录
            </button>
          )}
          <button
            onClick={() => setTasksOpen(false)}
            className="btn btn-icon -mr-1 -mt-1"
            aria-label="关闭任务中心"
          >
            <X size={15} />
          </button>
        </div>

        <div className="scroll-thin min-h-0 flex-1 overflow-y-auto px-4 py-3">
          {list.length === 0 && (
            <div className="mt-10 rounded-[12px] border border-dashed border-line px-4 py-8 text-center">
              <p className="text-[12.5px] font-medium">还没有任务</p>
              <p className="mt-1 text-[11px] leading-relaxed text-ink-3">
                安装、卸载、切换版本、预下载都会记录在这里，失败原因也能展开看。
              </p>
            </div>
          )}

          <div className="space-y-2">
            {list.map((t) => {
              const meta = STATUS[t.status];
              const expanded = openId === t.id;
              const hasDetail = !!t.detail && t.detail !== t.stage;
              return (
                <div
                  key={t.id}
                  className={clsx(
                    'rounded-[10px] border px-3 py-2.5',
                    t.status === 'error'
                      ? 'border-danger/35 bg-danger-soft/30'
                      : 'border-line bg-surface-2/40',
                  )}
                >
                  <div className="flex items-center gap-2">
                    <span
                      className="h-[6px] w-[6px] shrink-0 rounded-full"
                      style={{ background: meta.color }}
                    />
                    <span className="min-w-0 flex-1 truncate text-[12.5px] font-medium">
                      {t.title}
                    </span>
                    <span className="shrink-0 text-[10.5px] text-ink-3">
                      {relMs(t.createdAt)}
                    </span>
                  </div>

                  <p className="mt-1 flex items-center gap-1.5 text-[10.5px] text-ink-3">
                    <span className="truncate">
                      {t.subtitle}
                      {t.stage && t.stage !== meta.label ? ` · ${t.stage}` : ''}
                    </span>
                  </p>

                  {t.status === 'running' && (
                    <div className="mt-2 flex items-center gap-2">
                      <div className="flex-1">
                        <Progress value={t.progress} />
                      </div>
                      <span className="num w-[28px] shrink-0 text-right text-[10px] text-ink-3">
                        {Math.round(t.progress)}%
                      </span>
                    </div>
                  )}

                  <div className="mt-2 flex items-center gap-1.5">
                    <span
                      className="rounded px-1.5 py-[1px] text-[9.5px]"
                      style={{ background: `${meta.color}22`, color: meta.color }}
                    >
                      {meta.label}
                    </span>
                    <div className="flex-1" />
                    {hasDetail && (
                      <button
                        className="btn btn-ghost h-6 px-1.5 text-[10.5px]"
                        onClick={() => setOpenId(expanded ? null : t.id)}
                      >
                        <ChevronDown
                          size={11}
                          className={clsx('transition-transform', expanded && 'rotate-180')}
                        />
                        详情
                      </button>
                    )}
                    {t.status === 'running' && (
                      <button
                        className="btn btn-ghost h-6 px-1.5 text-[10.5px]"
                        onClick={() => cancelTask(t.id)}
                      >
                        取消
                      </button>
                    )}
                    {t.status === 'error' && t.retry && (
                      <button
                        className="btn btn-ghost h-6 px-1.5 text-[10.5px]"
                        onClick={() => void retryTask(t.id)}
                      >
                        <RotateCcw size={11} />
                        重试
                      </button>
                    )}
                    {t.status !== 'running' && (
                      <button
                        className="btn btn-icon h-6 w-6"
                        onClick={() => dismissTask(t.id)}
                        aria-label="移除这条记录"
                      >
                        <X size={11} />
                      </button>
                    )}
                  </div>

                  {expanded && hasDetail && (
                    <pre className="scroll-thin mono mt-2 max-h-[180px] overflow-auto whitespace-pre-wrap break-all rounded-[8px] border border-line-soft bg-surface px-2.5 py-2 text-[10px] leading-relaxed text-ink-2">
                      {t.detail}
                    </pre>
                  )}
                </div>
              );
            })}
          </div>
        </div>

        <div className="flex items-center gap-2 border-t border-line px-5 py-3 text-[10.5px] text-ink-3">
          <AlertTriangle size={12} />
          任务在后台继续跑，关掉面板不影响进度
        </div>
      </aside>
    </div>
  );
}

/** 右下角浮层：只显示进行中的任务 + 最近一条结果 */
export function TaskDock() {
  const { tasks, toasts, cancelTask, dismissTask, dismissToast, setTasksOpen, tasksOpen } = useApp();

  const running = tasks.filter((t) => t.status === 'running');
  const recent = [...tasks].reverse().find((t) => t.status !== 'running');
  const visible = [...running.slice(0, 3), ...(recent && running.length < 3 ? [recent] : [])];
  const hidden = tasks.length - visible.length;

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-30 flex w-[330px] flex-col gap-2">
      {toasts.map((t) => {
        const color =
          t.kind === 'ok'
            ? 'var(--s-ok)'
            : t.kind === 'warn'
              ? 'var(--s-warn)'
              : t.kind === 'danger'
                ? 'var(--s-danger)'
                : 'var(--s-accent)';
        return (
          <div
            key={t.id}
            className="anim-toast pointer-events-auto flex items-start gap-2.5 rounded-[10px] border border-line bg-surface px-3 py-2.5 shadow-[var(--s-shadow)]"
          >
            <span
              className="mt-[5px] h-[6px] w-[6px] shrink-0 rounded-full"
              style={{ background: color }}
            />
            <div className="min-w-0 flex-1">
              <p className="truncate text-[12px] font-medium">{t.title}</p>
              {t.desc && (
                <p className="mt-0.5 max-h-[48px] overflow-hidden text-[10.5px] leading-relaxed text-ink-3">
                  {t.desc}
                </p>
              )}
            </div>
            <button
              onClick={() => dismissToast(t.id)}
              className="-mr-1 -mt-0.5 shrink-0 text-ink-3 transition-colors hover:text-ink"
              aria-label="关闭提示"
            >
              <X size={12} />
            </button>
          </div>
        );
      })}

      {!tasksOpen &&
        visible.map((t) => {
          const done = t.status === 'done';
          const runningNow = t.status === 'running';
          return (
            <div
              key={t.id}
              className="anim-toast pointer-events-auto rounded-[12px] border border-line bg-surface p-3 shadow-[var(--s-shadow)]"
            >
              <div className="flex items-center gap-2">
                <span className="flex-1 truncate text-[12.5px] font-medium">{t.title}</span>
                {runningNow ? (
                  <button
                    onClick={() => cancelTask(t.id)}
                    className="-mr-1 text-ink-3 transition-colors hover:text-ink"
                    aria-label="取消任务"
                  >
                    <X size={13} />
                  </button>
                ) : (
                  <>
                    {done ? (
                      <Check size={13} className="text-ok" />
                    ) : (
                      <AlertTriangle size={13} className="text-warn" />
                    )}
                    <button
                      onClick={() => dismissTask(t.id)}
                      className="-mr-1 text-ink-3 transition-colors hover:text-ink"
                      aria-label="移除任务"
                    >
                      <X size={12} />
                    </button>
                  </>
                )}
              </div>
              <p className="mt-0.5 line-clamp-2 text-[10.5px] leading-relaxed text-ink-3">
                {t.subtitle} · {t.stage}
              </p>
              {runningNow && (
                <div className="mt-2 flex items-center gap-2">
                  <div className="flex-1">
                    <Progress value={t.progress} />
                  </div>
                  <span className="num w-[26px] text-right text-[10px] text-ink-3">
                    {Math.round(t.progress)}%
                  </span>
                </div>
              )}
            </div>
          );
        })}

      {(hidden > 0 || (!tasksOpen && tasks.length > 0)) && (
        <button
          onClick={() => setTasksOpen(true)}
          className="pointer-events-auto rounded-[10px] border border-line bg-surface px-3 py-2 text-left text-[11px] text-ink-2 shadow-[var(--s-shadow)] transition-colors hover:text-ink"
        >
          {hidden > 0 ? `还有 ${hidden} 条任务` : '打开任务中心'}
          <span className="text-ink-3"> · 历史与详情</span>
        </button>
      )}
    </div>
  );
}