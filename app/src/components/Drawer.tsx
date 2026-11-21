import { useEffect, useState } from 'react';
import {
  AlertTriangle,
  Archive,
  ChevronDown,
  Copy,
  FileArchive,
  FolderOpen,
  Info,
  RefreshCw,
  RotateCcw,
  ScrollText,
  Trash2,
  X,
} from 'lucide-react';
import { clsx, fullDate, SOURCE_LABEL, statusOf, versionKnown } from '../lib';
import { call, errorText, isTauri } from '../lib/ipc';
import { useApp } from '../store';
import type { Game, SnapshotInfo } from '../types';
import { Cover, MetaRow, Select, Stepper, Toggle } from './ui';

const PATH_OPTIONS = [
  { value: 'auto', label: 'auto（按 GPU 自动选择）' },
  { value: 'coopmat', label: 'coopmat' },
  { value: 'packed', label: 'packed' },
  { value: 'dp4a', label: 'dp4a（INT8）' },
  { value: 'portable', label: 'portable' },
  { value: 'portable-linalg', label: 'portable-linalg' },
  { value: 'portable-xmx', label: 'portable-xmx' },
];

export function Drawer() {
  const {
    drawerId,
    games,
    versions,
    closeDrawer,
    install,
    uninstall,
    switchVersion,
    toggleEnabled,
    setFgReplacement,
    updateConfig,
    setView,
    setLogGame,
    makeDiag,
    repair,
    toast,
  } = useApp();
  const [advOpen, setAdvOpen] = useState(false);

  const game = games.find((g) => g.id === drawerId);
  if (!game) return null;

  const latest = versions.find((v) => v.latest)?.tag ?? versions[0]?.tag ?? '—';
  const st = statusOf(game, latest);
  const anr = game.anr;
  const replacement = (anr?.fgReplacement ?? '').trim().toLowerCase();
  // Q1 三态：ANR / 原生 DLSS-G / 跟随 OptiScaler 的 auto
  const fgMode: 'anr' | 'native' | 'auto' = replacement.includes('apex')
    ? 'anr'
    : replacement === 'none'
      ? 'native'
      : 'auto';
  const currentIdx = anr ? versions.findIndex((v) => v.tag === anr.version) : -1;
  const previous = currentIdx >= 0 ? versions[currentIdx + 1] : undefined;

  const versionOptions = (() => {
    const top = versions.slice(0, 8);
    if (anr && !top.some((v) => v.tag === anr.version)) {
      const cur = versions.find((v) => v.tag === anr.version);
      if (cur) return [cur, ...top];
    }
    return top;
  })();

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-40">
      <div
        className="anim-overlay absolute inset-0 bg-black/45"
        onClick={closeDrawer}
        aria-hidden
      />
      <aside className="anim-drawer absolute right-0 top-0 flex h-full w-[460px] flex-col border-l border-line bg-surface shadow-2xl">
        <div className="flex items-start gap-3 border-b border-line px-5 py-4">
          <Cover name={game.name} className="h-11 w-11 shrink-0 rounded-[9px]" glyph={16} />
          <div className="min-w-0 flex-1">
            <h2 className="truncate text-[14px] font-medium leading-tight">{game.name}</h2>
            <div className="mt-1 flex items-center gap-1.5">
              <p className="mono truncate text-[10.5px] text-ink-3">{game.dir}</p>
              <button
                className="shrink-0 text-ink-3 transition-colors hover:text-ink"
                onClick={() => {
                  navigator.clipboard?.writeText(game.dir);
                  toast({ kind: 'info', title: '路径已复制' });
                }}
                aria-label="复制路径"
              >
                <Copy size={11} />
              </button>
            </div>
          </div>
          <button onClick={closeDrawer} className="btn btn-icon -mr-1 -mt-1" aria-label="关闭">
            <X size={15} />
          </button>
        </div>

        <div className="scroll-thin flex-1 overflow-y-auto px-5 py-4">
          {game.running && (
            <Banner
              tone="accent"
              icon={<Info size={13} />}
              title="游戏正在运行"
              desc="安装、卸载、切换版本与写配置都已禁用：OptiScaler 退出时会把内存里的设置写回 ini，运行中改的会被覆盖。先退出游戏再操作。"
            />
          )}
          {!anr && game.thirdPartyOptiScaler && (
            <Banner
              tone="warn"
              icon={<Info size={13} />}
              title="检测到已有的 OptiScaler"
              desc="这个游戏不是本工具安装的 OptiScaler。为避免覆盖你的配置，将自动使用「插件安装」方式。"
            />
          )}
          {anr && anr.health !== 'ok' && (
            <Banner
              tone="warn"
              icon={<AlertTriangle size={13} />}
              title="安装不完整"
              desc={anr.healthNote ?? '配置文件存在问题，ANR 可能不会生效。'}
              action={
                <button
                  className="btn btn-ghost h-7 text-[11.5px]"
                  onClick={() => void repair(game.id)}
                >
                  一键修复
                </button>
              }
            />
          )}

          {game.conflict && (
            <div className="mt-3 flex items-start gap-2 rounded-[10px] border border-warn/40 bg-warn-soft/50 px-3 py-2.5">
              <AlertTriangle size={13} className="mt-[2px] shrink-0 text-warn" />
              <div className="min-w-0">
                <p className="text-[11.5px] font-medium text-ink">安装状态需要先确认</p>
                <p className="mt-1 text-[11px] leading-relaxed text-ink-2">{game.conflict}</p>
              </div>
            </div>
          )}

          {anr ? (
            <>
              <div className="rounded-[12px] border border-line bg-surface-2/40 p-3.5">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="flex items-center gap-2">
                      <span
                        className="h-[6px] w-[6px] rounded-full"
                        style={{ background: st.tone === 'ok' ? 'var(--s-ok)' : 'var(--s-warn)' }}
                      />
                      <span className="text-[13px] font-medium">{st.label}</span>
                      <span className="mono text-[11.5px] text-ink-2">{anr.version}</span>
                    </div>
                    <p className="mt-1 text-[11px] text-ink-3">
                      {anr.health !== 'ok'
                        ? '配置有问题，ANR 目前不会生效'
                        : !versionKnown(latest)
                          ? '正在获取最新版本…'
                          : anr.version === latest
                            ? '已是最新版本'
                            : `可更新到 ${latest}，切换会保留你的参数`}
                    </p>
                  </div>
                  {versionKnown(latest) && anr.version !== latest && (
                    <button
                      className="btn btn-primary h-7 text-[11.5px]"
                      disabled={game.running}
                      onClick={() => switchVersion(game.id, latest)}
                    >
                      更新
                    </button>
                  )}
                </div>
                {versionKnown(latest) && anr.version !== latest && (
                  <div className="mt-3 flex items-center gap-2 text-[10.5px]">
                    <span className="mono text-ink-3">{anr.version}</span>
                    <span className="h-px flex-1 bg-line" />
                    <span className="text-ink-3">→</span>
                    <span className="mono font-medium text-accent">{latest}</span>
                  </div>
                )}
              </div>

              <div className="mt-4 flex items-center justify-between rounded-[12px] border border-line px-3.5 py-3">
                <div>
                  <p className="text-[12.5px] font-medium">帧生成</p>
                  <p className="mt-0.5 text-[11px] text-ink-3">
                    写入 OptiScaler.ini 的 [FrameGen] Enabled；关闭只是停用，文件仍留在游戏目录
                  </p>
                </div>
                <Toggle
                  checked={anr.enabled}
                  disabled={game.running}
                  onChange={() => toggleEnabled(game.id)}
                />
              </div>

              <div className="mt-3 rounded-[12px] border border-line px-3.5 py-3">
                <div className="flex items-center justify-between gap-3">
                  <div className="min-w-0">
                    <p className="text-[12.5px] font-medium">帧生成实现</p>
                    <p className="mt-0.5 text-[11px] text-ink-3">
                      写的是 OptiScaler.ini 的 FGNvngxReplacement，重启游戏生效
                    </p>
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
                    {(
                      [
                        { v: 'anr', label: 'ANR' },
                        { v: 'native', label: '原生 DLSS-G' },
                        { v: 'auto', label: '自动' },
                      ] as const
                    ).map((opt) => {
                      const active = fgMode === opt.v;
                      return (
                        <button
                          key={opt.v}
                          disabled={game.running}
                          onClick={() => void setFgReplacement(game.id, opt.v)}
                          className={clsx(
                            'rounded-[7px] border px-2.5 py-1 text-[11px] transition-colors',
                            active
                              ? 'border-accent-soft bg-accent-soft text-accent'
                              : 'border-line text-ink-3 hover:text-ink',
                          )}
                        >
                          {opt.label}
                        </button>
                      );
                    })}
                  </div>
                </div>
                <p className="mt-2 text-[10.5px] leading-relaxed text-ink-3">
                  {fgMode === 'anr'
                    ? '当前由 ANR 生成中间帧，任何显卡都能用。'
                    : fgMode === 'native'
                      ? '当前旁路到 NVIDIA 官方 DLSS-G，只有 40 系显卡生效，其它显卡会没有帧生成。'
                      : '当前为 auto，由 OptiScaler 自己决定用哪个 provider；不确定时选这个。'}
                </p>
              </div>

              <div className="mt-3 flex items-center justify-between gap-3 rounded-[12px] border border-line px-3.5 py-3">
                <div className="min-w-0">
                  <p className="text-[12.5px] font-medium">版本</p>
                  <p className="mt-0.5 text-[11px] text-ink-3">
                    {previous ? `可回滚到 ${previous.tag}` : '没有更早的版本可回滚'}
                  </p>
                </div>
                <div className="flex shrink-0 items-center gap-1.5">
                  <Select
                    value={anr.version}
                    onChange={(v) => switchVersion(game.id, v)}
                    options={versionOptions.map((v) => ({ value: v.tag, label: v.tag }))}
                    className="w-[92px]"
                    disabled={game.running}
                  />
                  <button
                    className="btn btn-ghost h-8 w-8 px-0"
                    disabled={!previous || game.running}
                    title="回滚到上一版"
                    onClick={() => previous && switchVersion(game.id, previous.tag)}
                  >
                    <RefreshCw size={13} />
                  </button>
                </div>
              </div>

              <div className="mt-5">
                <p className="mb-1 text-[11px] font-medium tracking-[0.04em] text-ink-3">安装信息</p>
                <MetaRow label="安装方式">
                  {anr.kind === 'addon' ? '插件（沿用已有 OptiScaler）' : '全量（含 OptiScaler）'}
                </MetaRow>
                <MetaRow label="代理 DLL" mono>
                  {anr.proxy}
                </MetaRow>
                <MetaRow label="来源">{SOURCE_LABEL[game.source]}</MetaRow>
                <MetaRow label="安装时间">{fullDate(anr.installedAt)}</MetaRow>
                <MetaRow label="版本指纹" mono>
                  {anr.fingerprint}…
                </MetaRow>
              </div>

              <div className="mt-4 overflow-hidden rounded-[12px] border border-line">
                <button
                  onClick={() => setAdvOpen(!advOpen)}
                  className="flex w-full items-center justify-between px-3.5 py-2.5 text-left transition-colors hover:bg-surface-2/60"
                >
                  <span className="text-[12.5px] font-medium">高级参数</span>
                  <span className="flex items-center gap-1 text-[11px] text-ink-3">
                    nr_fg.ini
                    <ChevronDown
                      size={14}
                      className={clsx('transition-transform', advOpen && 'rotate-180')}
                    />
                  </span>
                </button>
                {advOpen && (
                  <div
                    className={clsx(
                      'space-y-3 border-t border-line-soft px-3.5 py-3',
                      game.running && 'pointer-events-none opacity-60',
                    )}
                    aria-disabled={game.running}
                  >
                    <ParamRow label="卷积路径" hint="改动后需重启游戏">
                      <Select
                        value={anr.config.path}
                        onChange={(v) =>
                          updateConfig(game.id, { config: { ...anr.config, path: v } })
                        }
                        options={PATH_OPTIONS}
                        className="w-[190px]"
                        disabled={game.running}
                      />
                    </ParamRow>
                    <ParamRow label="最大生成帧" hint="每个真实帧最多生成 5 帧">
                      <div className="flex items-center gap-1">
                        {[2, 3, 4, 5].map((n) => (
                          <button
                            key={n}
                            disabled={game.running}
                            onClick={() =>
                              updateConfig(game.id, { config: { ...anr.config, maxGenerated: n } })
                            }
                            className={clsx(
                              'num h-7 w-7 rounded-[6px] border text-[11.5px] transition-colors',
                              anr.config.maxGenerated === n
                                ? 'border-accent-line bg-accent-soft font-medium text-accent'
                                : 'border-line text-ink-2 hover:border-surface-3',
                            )}
                          >
                            {n}
                          </button>
                        ))}
                      </div>
                    </ParamRow>
                    <ParamRow label="场景切换保护" hint="避免镜头切换时插帧出错">
                      <Toggle
                        checked={anr.config.sceneCut}
                        disabled={game.running}
                        onChange={() =>
                          updateConfig(game.id, {
                            config: { ...anr.config, sceneCut: !anr.config.sceneCut },
                          })
                        }
                      />
                    </ParamRow>
                    <ParamRow label="HDR 亮度" hint="模型线性 1.0 对应的显示亮度">
                      <Stepper
                        value={anr.config.nits}
                        min={40}
                        max={400}
                        suffix="nit"
                        onChange={(v) =>
                          updateConfig(game.id, { config: { ...anr.config, nits: v } })
                        }
                      />
                    </ParamRow>
                  </div>
                )}
              </div>
            </>
          ) : (
            <div className="rounded-[12px] border border-dashed border-line px-4 py-8 text-center">
              <p className="text-[13px] font-medium">这个游戏还没有安装 ANR</p>
              <p className="mx-auto mt-1 max-w-[280px] text-[11.5px] leading-relaxed text-ink-3">
                {game.thirdPartyOptiScaler
                  ? '将以插件方式加入你已有的 OptiScaler，不会覆盖它的配置。'
                  : '将安装 OptiScaler 并把它作为 dxgi.dll 载入，同时备份原有文件。'}
              </p>
              <button
                className="btn btn-primary mt-4 h-8 px-4"
                disabled={game.running}
                onClick={() => install(game.id)}
              >
                立即安装
              </button>
              <p className="mt-2 text-[10.5px] text-ink-3">安装前会自动检查进程与 VC++ 运行时</p>
            </div>
          )}

          <SnapshotSection gameId={game.id} />
          {anr && <HealthSection game={game} />}
        </div>

        <div className="flex items-center gap-1.5 border-t border-line px-5 py-3">
          <button
            className="btn btn-ghost h-8 text-[11.5px]"
            onClick={() => {
              setLogGame(game.id);
              setView('logs');
              closeDrawer();
            }}
          >
            <ScrollText size={13} />
            查看日志
          </button>
          <button
            className="btn btn-ghost h-8 text-[11.5px]"
            onClick={() => void makeDiag(game.id)}
          >
            <FileArchive size={13} />
            诊断包
          </button>
          <button
            className="btn btn-ghost h-8 w-8 px-0"
            onClick={() => {
              if (isTauri) void call('open_path', { path: game.dir });
              else toast({ kind: 'info', title: '已打开目录', desc: game.dir });
            }}
            aria-label="打开目录"
          >
            <FolderOpen size={13} />
          </button>
          <div className="flex-1" />
          {anr && (
            <button
              className="btn btn-danger h-8 text-[11.5px]"
              disabled={game.running}
              onClick={() => void uninstall(game.id)}
            >
              <Trash2 size={13} />
              卸载
            </button>
          )}
        </div>
      </aside>
    </div>
  );
}

/** 安装体检：列出这个游戏当前所有配置/文件问题，能本地修的直接修 */
function HealthSection({ game }: { game: Game }) {
  const { healthIssues, checkHealth, repair } = useApp();
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const issues = healthIssues[game.id] ?? [];
  const bad = issues.filter((i) => i.level === 'error').length;

  useEffect(() => {
    setOpen(false);
    setLoaded(false);
  }, [game.id]);

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (next && !loaded) {
      setLoaded(true);
      void checkHealth(game.id);
    }
  };

  const runRepair = async () => {
    setBusy(true);
    try {
      await repair(game.id);
      setLoaded(true);
      setOpen(true);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="mt-3 overflow-hidden rounded-[12px] border border-line">
      <button
        onClick={toggle}
        className="flex w-full items-center justify-between px-3.5 py-2.5 text-left transition-colors hover:bg-surface-2/60"
      >
        <span className="flex items-center gap-2">
          <AlertTriangle size={13} className={bad ? 'text-warn' : 'text-ink-3'} />
          <span className="text-[12.5px] font-medium">安装体检</span>
          {issues.length > 0 && (
            <span className="num text-[10.5px] text-ink-3">{issues.length}</span>
          )}
        </span>
        <span className="flex items-center gap-1 text-[11px] text-ink-3">
          {issues.length === 0 && loaded ? '没有发现问题' : '检查文件与配置'}
          <ChevronDown size={14} className={clsx('transition-transform', open && 'rotate-180')} />
        </span>
      </button>
      {open && (
        <div className="border-t border-line-soft px-3.5 py-3">
          {issues.length === 0 ? (
            <p className="py-1 text-[11px] leading-relaxed text-ink-3">
              没有发现问题。安装状态、provider 文件、代理 DLL 与 ini 关键项都在。
            </p>
          ) : (
            <div className="space-y-1.5">
              {issues.map((it) => (
                <div
                  key={it.code}
                  className="rounded-[8px] border border-line-soft px-2.5 py-2"
                >
                  <p className="flex items-center gap-1.5 text-[11.5px] font-medium">
                    <span
                      className="h-[5px] w-[5px] shrink-0 rounded-full"
                      style={{
                        background: it.level === 'error' ? 'var(--s-danger)' : 'var(--s-warn)',
                      }}
                    />
                    {it.title}
                    {it.fixable && (
                      <span className="rounded bg-accent-soft px-1 py-[1px] text-[9px] text-accent">
                        可修复
                      </span>
                    )}
                  </p>
                  <p className="mt-1 text-[10.5px] leading-relaxed text-ink-3">{it.detail}</p>
                </div>
              ))}
            </div>
          )}
          <div className="mt-2.5 flex items-center gap-2">
            <button
              className="btn btn-ghost h-7 text-[11px]"
              onClick={() => void checkHealth(game.id)}
              disabled={busy}
            >
              <RefreshCw size={11} />
              重新检查
            </button>
            <button
              className="btn btn-primary h-7 text-[11px]"
              onClick={() => void runRepair()}
              disabled={busy || game.running}
            >
              {busy ? '修复中…' : '一键修复'}
            </button>
            {game.running && <span className="text-[10px] text-ink-3">游戏运行中，先退出</span>}
          </div>
        </div>
      )}
    </div>
  );
}

function SnapshotSection({ gameId }: { gameId: string }) {
  const { restoreSnapshot, toast } = useApp();
  const [open, setOpen] = useState(false);
  const [items, setItems] = useState<SnapshotInfo[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    setOpen(false);
    setItems(null);
    setError('');
    setLoading(false);
  }, [gameId]);

  const load = async () => {
    if (!isTauri) {
      setItems([]);
      setError('快照只在桌面应用里可用');
      return;
    }
    setLoading(true);
    setError('');
    try {
      setItems(await call<SnapshotInfo[]>('list_snapshots', { id: gameId }));
    } catch (e) {
      setItems([]);
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  };

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (next && items === null) void load();
  };

  const oldSnapshots = (items ?? []).some((s) => s.schema < 2);

  return (
    <div className="mt-5 overflow-hidden rounded-[12px] border border-line">
      <button
        onClick={toggle}
        className="flex w-full items-center justify-between px-3.5 py-2.5 text-left transition-colors hover:bg-surface-2/60"
      >
        <span className="flex items-center gap-2">
          <Archive size={13} className="text-ink-3" />
          <span className="text-[12.5px] font-medium">快照</span>
          {items && items.length > 0 && (
            <span className="num text-[10.5px] text-ink-3">{items.length}</span>
          )}
        </span>
        <span className="flex items-center gap-1 text-[11px] text-ink-3">
          恢复安装前状态
          <ChevronDown size={14} className={clsx('transition-transform', open && 'rotate-180')} />
        </span>
      </button>
      {open && (
        <div className="border-t border-line-soft px-3.5 py-3">
          {loading && <p className="py-1 text-[11px] text-ink-3">正在读取快照…</p>}
          {!loading && error && <p className="py-1 text-[11px] text-ink-3">{error}</p>}
          {!loading && !error && items?.length === 0 && (
            <p className="py-1 text-[11px] leading-relaxed text-ink-3">
              暂无快照。卸载、切换版本或覆盖安装前会自动生成。
            </p>
          )}
          {!loading && !error && !!items?.length && (
            <div className="space-y-1.5">
              {items.map((s) => (
                <div
                  key={s.path}
                  className="flex items-center gap-2 rounded-[8px] border border-line-soft px-2.5 py-2"
                >
                  <div className="min-w-0 flex-1">
                    <p className="mono truncate text-[10.5px] text-ink-2">
                      {fullDate(s.createdAt)}
                      <span className="text-ink-3"> · {(s.sizeBytes / 1048576).toFixed(1)} MB</span>
                      {s.schema < 2 && (
                        <span className="ml-1.5 rounded bg-warn-soft px-1 py-[1px] text-[9px] text-warn">
                          旧版
                        </span>
                      )}
                    </p>
                    <p className="mt-0.5 truncate text-[9.5px] text-ink-3" title={s.fileName}>
                      {s.fileName}
                    </p>
                  </div>
                  <button
                    className="btn btn-ghost h-7 shrink-0 px-2 text-[10.5px]"
                    onClick={() => void restoreSnapshot(gameId, s.path)}
                  >
                    <RotateCcw size={11} />
                    恢复
                  </button>
                </div>
              ))}
              {oldSnapshots && (
                <p className="pt-1 text-[10px] leading-relaxed text-warn">
                  旧版快照只包含 OptiScaler 目录；恢复后可能缺少代理 DLL 和 OptiScaler.ini。
                </p>
              )}
            </div>
          )}
          <button
            className="mt-2 text-[10px] text-ink-3 transition-colors hover:text-ink"
            onClick={() => {
              void load();
              toast({ kind: 'info', title: '已刷新快照列表' });
            }}
          >
            刷新列表
          </button>
        </div>
      )}
    </div>
  );
}

function ParamRow({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="min-w-0">
        <p className="text-[12px]">{label}</p>
        {hint && <p className="text-[10.5px] text-ink-3">{hint}</p>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

export function Banner({
  tone,
  icon,
  title,
  desc,
  action,
}: {
  tone: 'warn' | 'accent' | 'danger';
  icon: React.ReactNode;
  title: string;
  desc: string;
  action?: React.ReactNode;
}) {
  const bg =
    tone === 'warn' ? 'var(--s-warn-soft)' : tone === 'danger' ? 'var(--s-danger-soft)' : 'var(--s-accent-soft)';
  const fg = tone === 'warn' ? 'var(--s-warn)' : tone === 'danger' ? 'var(--s-danger)' : 'var(--s-accent)';
  return (
    <div className="mb-3 rounded-[10px] p-3" style={{ background: bg }}>
      <div className="flex items-start gap-2">
        <span className="mt-[1px] shrink-0" style={{ color: fg }}>
          {icon}
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-[12px] font-medium" style={{ color: fg }}>
            {title}
          </p>
          <p className="mt-0.5 text-[11px] leading-relaxed text-ink-2">{desc}</p>
        </div>
        {action && <div className="shrink-0 self-center">{action}</div>}
      </div>
    </div>
  );
}
