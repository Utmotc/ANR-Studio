import { useEffect } from 'react';
import { AlertTriangle, Bug, Copy, FileArchive, FolderOpen, RefreshCw, ScrollText } from 'lucide-react';
import { clsx } from '../lib';
import { call, isTauri } from '../lib/ipc';
import { useApp } from '../store';
import { DEFAULT_LOG, LOGS } from '../data';
import { Cover, Empty } from '../components/ui';

function parseLine(line: string) {
  const m = line.match(/^\[([^\]]+)\]\s?(.*)$/);
  return m ? { ts: m[1], text: m[2] } : { ts: '', text: line };
}

function toneOf(text: string) {
  const t = text.toLowerCase();
  if (t.includes('error') || t.includes('fail')) return 'text-danger';
  if (t.includes('warn')) return 'text-warn';
  if (t.includes('hint')) return 'text-accent';
  return 'text-ink-2';
}

export function Logs() {
  const { logGameId, setLogGame, games, toast, logText, logLoading, loadLog, makeDiag } = useApp();

  const installed = games.filter((g) => g.anr);
  const game = games.find((g) => g.id === logGameId) ?? installed[0];

  useEffect(() => {
    if (game && isTauri) void loadLog(game.id);
  }, [game?.id, loadLog]);

  const lines = isTauri ? (logText ? logText.split('\n') : []) : ((game && LOGS[game.id]) ?? DEFAULT_LOG);

  return (
    <div className="flex h-full">
      <aside className="scroll-thin w-[236px] shrink-0 overflow-y-auto border-r border-line py-3">
        <p className="px-4 pb-2 text-[10.5px] text-ink-3">已安装的游戏 · {installed.length}</p>
        {installed.map((g) => (
          <button
            key={g.id}
            onClick={() => setLogGame(g.id)}
            className={clsx(
              'mx-2 flex w-[calc(100%-16px)] items-center gap-2 rounded-[8px] px-2 py-1.5 text-left transition-colors',
              g.id === game?.id ? 'bg-accent-soft' : 'hover:bg-surface-2',
            )}
          >
            <Cover name={g.name} className="h-6 w-6 shrink-0 rounded-[5px]" glyph={11} />
            <span className="min-w-0 flex-1">
              <span
                className={clsx(
                  'block truncate text-[11.5px]',
                  g.id === game?.id ? 'font-medium text-accent' : 'text-ink-2',
                )}
              >
                {g.name}
              </span>
              <span className="mono block truncate text-[10px] text-ink-3">{g.anr?.version}</span>
            </span>
            {g.anr && g.anr.health !== 'ok' && <AlertTriangle size={11} className="text-warn" />}
          </button>
        ))}
        {installed.length === 0 && (
          <p className="px-4 py-6 text-[11px] text-ink-3">还没有安装任何游戏</p>
        )}
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        {game ? (
          <>
            <div className="flex shrink-0 items-center gap-2.5 border-b border-line px-5 py-3">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <h2 className="truncate text-[13px] font-medium">{game.name}</h2>
                  {!isTauri && <span className="chip h-[17px] text-[10px]">示例日志</span>}
                </div>
                <p className="mono mt-0.5 truncate text-[10px] text-ink-3">
                  {game.anr?.logPath || '暂无日志文件'}
                </p>
              </div>
              <button
                className="btn btn-icon h-7 w-7"
                onClick={() => void loadLog(game.id)}
                aria-label="重新读取"
                title="重新读取"
              >
                <RefreshCw size={12} className={logLoading ? 'animate-spin' : undefined} />
              </button>
              <button
                className="btn btn-ghost h-7 text-[11.5px]"
                disabled={lines.length === 0}
                onClick={() => {
                  navigator.clipboard?.writeText(lines.join('\n'));
                  toast({ kind: 'info', title: '日志已复制到剪贴板' });
                }}
              >
                <Copy size={12} />
                复制
              </button>
              <button
                className="btn btn-icon h-7 w-7"
                onClick={() => toast({ kind: 'info', title: '日志目录', desc: game.dir })}
                aria-label="打开日志目录"
              >
                <FolderOpen size={13} />
              </button>
            </div>

            <div className="scroll-thin flex-1 overflow-auto bg-surface-2/25 px-1 py-3">
              {lines.length === 0 ? (
                <p className="px-4 py-8 text-[11.5px] leading-relaxed text-ink-3">
                  {logLoading
                    ? '读取中…'
                    : '这个游戏还没有 nr_fg 日志。进游戏跑一次帧生成，再回来看。'}
                </p>
              ) : (
                lines.map((line, i) => {
                  const { ts, text } = parseLine(line);
                  return (
                    <div
                      key={i}
                      className="flex gap-3 px-4 py-[3px] font-mono text-[11.5px] leading-relaxed hover:bg-surface-2/60"
                    >
                      <span className="num w-[176px] shrink-0 whitespace-nowrap text-[11px] text-ink-3">
                        {ts}
                      </span>
                      <span className="min-w-0 break-all">
                        <span className={toneOf(text)}>{text}</span>
                      </span>
                    </div>
                  );
                })
              )}
            </div>

            <div className="flex shrink-0 items-center gap-2 border-t border-line px-5 py-3">
              <div className="flex-1">
                <p className="text-[11.5px] text-ink-2">
                  提交问题前建议生成诊断包，包含日志、两个 ini 与系统信息
                </p>
                <p className="mt-0.5 text-[10.5px] text-ink-3">
                  官方要求附带最新的 nr_fg-*.log 与 OptiScaler.log
                </p>
              </div>
              <button
                className="btn btn-ghost h-8 text-[11.5px]"
                onClick={() => {
                  navigator.clipboard?.writeText(lines.join('\n'));
                  if (isTauri) {
                    void call('open_url', {
                      url: 'https://github.com/apex-compute/ANR/issues/new',
                    }).catch(() => undefined);
                  }
                  toast({
                    kind: 'ok',
                    title: '日志已复制',
                    desc: '浏览器已打开 Issue 页面，直接粘贴即可',
                  });
                }}
              >
                <Bug size={13} />
                复制并开 Issue
              </button>
              <button
                className="btn btn-primary h-8 text-[11.5px]"
                onClick={() => void makeDiag(game.id)}
              >
                <FileArchive size={13} />
                生成诊断包
              </button>
            </div>
          </>
        ) : (
          <div className="grid flex-1 place-items-center p-8">
            <Empty
              icon={<ScrollText size={17} />}
              title="没有可查看的日志"
              desc="安装 ANR 之后，这里会显示每个游戏最新的 nr_fg 日志。"
            />
          </div>
        )}
      </div>
    </div>
  );
}
