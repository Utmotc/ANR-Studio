import {
  ArrowRight,
  FolderPlus,
  Plus,
  RefreshCw,
  ScrollText,
  Sparkles,
} from 'lucide-react';
import { clsx, fullDate, needsAttention, relTime, SOURCE_LABEL, statusOf, versionKnown } from '../lib';
import { useApp } from '../store';
import type { Game } from '../types';
import { Cover, Toggle } from '../components/ui';

export function Overview() {
  const {
    games,
    versions,
    setView,
    setFilter,
    openDrawer,
    switchVersion,
    toggleEnabled,
    install,
    addGame,
    rescan,
  } = useApp();

  const latestVersion = versions.find((v) => v.latest);
  const latest = latestVersion?.tag ?? '—';

  const installed = games.filter((g) => g.anr);
  const enabled = games.filter((g) => g.anr?.enabled);
  const attention = games.filter((g) => needsAttention(g));
  const outdated = versionKnown(latest)
    ? games.filter((g) => g.anr && g.anr.version !== latest)
    : [];

  const recent = [...installed]
    .sort((a, b) => +new Date(b.lastUsed) - +new Date(a.lastUsed))
    .slice(0, 6);

  return (
    <div className="scroll-thin h-full overflow-y-auto">
      <div className="mx-auto flex min-h-full w-full max-w-[1180px] flex-col px-[var(--dens-pad-x)] pb-3 pt-[var(--dens-pad-y)]">
        {/* 头部：一个大数字 + 一条拉满宽度的状态带 */}
        <header>
          <div className="flex flex-wrap items-end justify-between gap-x-6 gap-y-2">
            <div className="min-w-0">
              <div className="text-[10px] font-medium tracking-[0.16em] text-ink-3">本机总览</div>
              <div className="mt-2.5 flex items-baseline gap-1.5">
                <span
                  className="num text-[40px] font-medium leading-[0.9] tracking-[-0.02em]"
                  style={{ color: installed.length ? 'var(--s-ink)' : 'var(--s-ink-3)' }}
                >
                  {enabled.length}
                </span>
                <span className="num text-[15px] leading-none text-ink-3">
                  /{installed.length}
                </span>
                <span className="ml-1.5 text-[11.5px] leading-none text-ink-3">
                  已启用
                  <span className="mx-1.5 opacity-40">·</span>
                  游戏库共 {games.length} 个
                </span>
              </div>
            </div>
            <div className="flex items-center gap-3 pb-0.5 text-[10px] text-ink-3">
              <Legend color="var(--s-ok)" label={`启用 ${enabled.length}`} />
              <Legend color="var(--s-ink-3)" label={`未启用 ${installed.length - enabled.length}`} />
              <span className="inline-flex items-center gap-1.5">
                <span className="h-[3px] w-[10px] rounded-full bg-surface-3" />
                未安装 {games.length - installed.length}
              </span>
            </div>
          </div>

          {installed.length > 0 && (
            <div className="mt-4">
              <StatusStrip games={games} />
            </div>
          )}
        </header>

        {/* 主体：左内容 + 右信息栏 */}
        <div className="mt-7 grid grid-cols-1 gap-x-6 gap-y-7 lg:grid-cols-[minmax(0,1fr)_286px]">
          <div className="min-w-0 space-y-7">
            {attention.length > 0 && (
              <section>
                <Rule title="需要处理" count={attention.length} tone="warn">
                  <button
                    onClick={() => {
                      setFilter('attention');
                      setView('library');
                    }}
                    className="inline-flex items-center gap-1 text-[10.5px] text-ink-3 transition-colors hover:text-ink"
                  >
                    全部
                    <ArrowRight size={11} />
                  </button>
                </Rule>
                <div className="overflow-hidden rounded-[12px] border border-line-soft bg-surface">
                  {attention.slice(0, 4).map((g, i) => {
                    const reason =
                      g.anr && g.anr.health !== 'ok'
                        ? (g.anr.healthNote ?? '配置存在问题')
                        : g.thirdPartyOptiScaler && !g.anr
                          ? '已装第三方 OptiScaler，需要以插件方式安装'
                          : `停在 ${g.anr?.version}，可以更新到 ${latest}`;
                    const label = !g.anr ? '安装' : g.anr.health !== 'ok' ? '查看' : '更新';
                    return (
                      <div
                        key={g.id}
                        className={clsx(
                          'group relative flex items-center gap-3 py-2.5 pl-3.5 pr-3',
                          i > 0 && 'border-t border-line-soft',
                        )}
                      >
                        <span className="absolute inset-y-0 left-0 w-[2px] bg-warn opacity-70" />
                        <div className="min-w-0 flex-1">
                          <p className="truncate text-[12.5px] font-medium leading-tight">
                            {g.name}
                          </p>
                          <p className="mt-1 truncate text-[10.5px] leading-tight text-ink-3">
                            {reason}
                          </p>
                        </div>
                        <button
                          onClick={() => {
                            if (!g.anr || g.anr.health !== 'ok') openDrawer(g.id);
                            else switchVersion(g.id, latest);
                          }}
                          className="btn btn-ghost h-7 shrink-0 text-[11px] opacity-80 transition-opacity group-hover:opacity-100"
                        >
                          {label}
                        </button>
                      </div>
                    );
                  })}
                </div>
              </section>
            )}

            {installed.length === 0 && (
              <section className="rounded-[12px] border border-line-soft bg-surface p-5">
                <div className="flex items-start gap-3.5">
                  <span className="grid h-9 w-9 shrink-0 place-items-center rounded-[9px] bg-accent-soft text-accent">
                    <Sparkles size={15} />
                  </span>
                  <div className="min-w-0">
                    <h2 className="text-[13px] font-medium">
                      {games.length === 0 ? '还没有扫描到游戏' : '游戏库里还没有装上 ANR 的游戏'}
                    </h2>
                    <p className="mt-1.5 max-w-[540px] text-[11.5px] leading-relaxed text-ink-3">
                      {games.length === 0
                        ? '本工具会读 Steam 的 libraryfolders.vdf、Epic 清单和 GOG 注册表来找已安装的游戏；扫描不到时可以手动指定游戏所在目录。'
                        : `这里共有 ${games.length} 个游戏。挑一个装上，之后就能看到它的开关状态、版本和日志。`}
                    </p>
                    <div className="mt-3 flex items-center gap-2">
                      <button
                        className="btn btn-primary h-8 text-[11.5px]"
                        onClick={() => {
                          if (games.length === 0) void addGame();
                          else {
                            setFilter('missing');
                            setView('library');
                          }
                        }}
                      >
                        {games.length === 0 ? (
                          <>
                            <Plus size={12} />
                            添加游戏目录
                          </>
                        ) : (
                          <>
                            <ArrowRight size={12} />
                            去游戏库
                          </>
                        )}
                      </button>
                      <button
                        className="btn btn-ghost h-8 text-[11.5px]"
                        onClick={() => void rescan()}
                      >
                        重新扫描
                      </button>
                    </div>
                  </div>
                </div>
              </section>
            )}

            {installed.length === 0 && <AboutAnr />}

            {recent.length > 0 && (
              <section>
                <Rule title="最近使用" count={recent.length}>
                  <button
                    onClick={() => {
                      setFilter('installed');
                      setView('library');
                    }}
                    className="inline-flex items-center gap-1 text-[10.5px] text-ink-3 transition-colors hover:text-ink"
                  >
                    全部
                    <ArrowRight size={11} />
                  </button>
                </Rule>
                <div className="grid grid-cols-1 gap-x-6 xl:grid-cols-2">
                  {recent.map((g) => (
                    <GameRow
                      key={g.id}
                      game={g}
                      latest={latest}
                      onOpen={() => openDrawer(g.id)}
                      onToggle={() => toggleEnabled(g.id)}
                      onInstall={() => install(g.id)}
                    />
                  ))}
                </div>
              </section>
            )}

            {installed.length > 0 && attention.length === 0 && (
              <section className="flex items-center gap-2.5 rounded-[12px] border border-line-soft bg-surface px-4 py-3">
                <span className="h-[6px] w-[6px] rounded-full bg-ok" />
                <p className="text-[11.5px] text-ink-2">
                  全部 {installed.length} 个游戏状态正常，没有需要处理的项。
                </p>
                <button
                  onClick={() => setView('logs')}
                  className="ml-auto inline-flex items-center gap-1 text-[11px] text-ink-3 transition-colors hover:text-ink"
                >
                  <ScrollText size={11} />
                  看日志
                </button>
              </section>
            )}
          </div>

          {/* 右栏 */}
          <aside className="space-y-3 lg:sticky lg:top-0 lg:self-start">
            <div className="rounded-[12px] border border-line-soft bg-surface p-4">
              <div className="flex items-start justify-between">
                <div>
                  <div className="text-[10px] tracking-[0.16em] text-ink-3">当前版本</div>
                  <div className="mono mt-1.5 text-[15px] font-medium leading-none text-ink">
                    {latest}
                  </div>
                </div>
                <span
                  className="num mt-0.5 text-[11px]"
                  style={{ color: outdated.length ? 'var(--s-warn)' : 'var(--s-ok)' }}
                >
                  {outdated.length ? `${outdated.length} 落后` : '全部最新'}
                </span>
              </div>

              <div className="mt-3 space-y-1.5 border-t border-line-soft pt-3 text-[10.5px]">
                <div className="flex items-center justify-between">
                  <span className="text-ink-3">发布时间</span>
                  <span className="mono text-ink-2">
                    {latestVersion ? fullDate(latestVersion.publishedAt) : '—'}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-ink-3">包体积</span>
                  <span className="mono text-ink-2">
                    {latestVersion?.sizeMB ? `${latestVersion.sizeMB} MB` : '—'}
                  </span>
                </div>
                <div className="flex items-center justify-between">
                  <span className="text-ink-3">已缓存</span>
                  <span className="mono text-ink-2">{versions.filter((v) => v.cached).length} 个</span>
                </div>
              </div>

              <button
                onClick={() => setView('versions')}
                className="btn btn-ghost mt-3 h-8 w-full justify-between text-[11.5px]"
              >
                版本与更新日志
                <ArrowRight size={12} />
              </button>
            </div>

            <div className="rounded-[12px] border border-line-soft bg-surface p-1.5">
              <QuickRow icon={<RefreshCw size={12} />} label="重新扫描" onClick={() => void rescan()} />
              <QuickRow
                icon={<FolderPlus size={12} />}
                label="添加游戏目录"
                onClick={() => void addGame()}
              />
              <div className="my-1 h-px bg-line-soft" />
              <QuickRow
                icon={<Plus size={12} />}
                label="装到新游戏"
                onClick={() => {
                  setFilter('missing');
                  setView('library');
                }}
              />
              <QuickRow
                icon={<RefreshCw size={12} />}
                label={
                  !versionKnown(latest)
                    ? '正在获取最新版本…'
                    : outdated.length
                      ? `全部更新到 ${latest}`
                      : '已是最新版本'
                }
                disabled={!outdated.length}
                onClick={() => outdated.forEach((g) => switchVersion(g.id, latest))}
              />
              <QuickRow
                icon={<ScrollText size={12} />}
                label="日志与诊断"
                onClick={() => setView('logs')}
              />
            </div>
          </aside>
        </div>

        <p className="mt-auto pt-8 pb-1 text-center text-[10px] text-ink-3">
          非官方工具，与 Apex Compute 无关联 · 只调用官方安装脚本，不修改游戏本体
        </p>
      </div>
    </div>
  );
}

/* ── 小部件 ── */

/** 空库时用来填住左栏：介绍 ANR 是什么、这个工具做什么、有什么风险 */
function AboutAnr() {
  const { setView, addGame, rescan } = useApp();

  const items = [
    {
      title: 'ANR 是什么',
      body: 'Apex Neural Rendering，一个通过 OptiScaler 注入游戏的帧生成 / 超分 DLL。打开游戏里的帧生成选项，就能在不换显卡的前提下把帧数拉高。',
    },
    {
      title: '这个工具做什么',
      body: '把「装到哪个游戏、装的是哪一版、帧生成开没开」变成看得见的状态。安装和卸载都调用官方脚本，参数读写走 OptiScaler.ini。',
    },
    {
      title: '需要注意',
      body: '注入类工具在带反作弊的联机游戏里有封号风险，建议只用于单机。工具不会修改游戏本体，也不会往内存里写东西。',
    },
  ];

  return (
    <section className="rounded-[12px] border border-line-soft bg-surface px-5 py-4">
      <div className="flex items-center gap-2">
        <span className="text-[11px] font-medium text-ink-2">关于 ANR 与这个工具</span>
        <span className="h-px flex-1 bg-line-soft" />
      </div>

      <div className="mt-3.5 space-y-3.5">
        {items.map((it) => (
          <div key={it.title} className="flex gap-3">
            <span className="mt-[6px] h-[3px] w-[3px] shrink-0 rounded-full bg-ink-3" />
            <div className="min-w-0">
              <p className="text-[11.5px] font-medium leading-tight text-ink">{it.title}</p>
              <p className="mt-1 text-[11px] leading-relaxed text-ink-3">{it.body}</p>
            </div>
          </div>
        ))}
      </div>

      <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-2 border-t border-line-soft pt-3">
        <button
          onClick={() => void addGame()}
          className="inline-flex items-center gap-1 text-[11px] text-accent hover:underline"
        >
          <FolderPlus size={11} />
          添加游戏目录
        </button>
        <button
          onClick={() => void rescan()}
          className="inline-flex items-center gap-1 text-[11px] text-ink-3 transition-colors hover:text-ink"
        >
          <RefreshCw size={11} />
          重新扫描
        </button>
        <button
          onClick={() => setView('versions')}
          className="inline-flex items-center gap-1 text-[11px] text-ink-3 transition-colors hover:text-ink"
        >
          版本与更新日志
          <ArrowRight size={11} />
        </button>
        <a
          href="https://github.com/apex-compute/ANR"
          target="_blank"
          rel="noreferrer"
          className="ml-auto inline-flex items-center gap-1 text-[11px] text-ink-3 transition-colors hover:text-accent"
        >
          ANR 官方仓库
          <ArrowRight size={11} />
        </a>
      </div>
    </section>
  );
}


function StatusStrip({ games }: { games: Game[] }) {
  const shown = games.filter((g) => g.anr);
  if (!shown.length) {
    return <div className="h-[6px] w-full rounded-full bg-surface-3" />;
  }
  return (
    <div className="flex h-[6px] w-full items-stretch gap-[2px] rounded-full bg-surface-3">
      {shown.map((g) => (
        <span
          key={g.id}
          title={`${g.name} · ${g.anr?.enabled ? '已启用' : '未启用'}`}
          className="min-w-[4px] flex-1 rounded-full transition-opacity hover:opacity-80"
          style={{ background: g.anr?.enabled ? 'var(--s-ok)' : 'var(--s-ink-3)' }}
        />
      ))}
    </div>
  );
}

function Legend({ color, label }: { color: string; label: string }) {
  return (
    <span className="inline-flex items-center gap-1.5">
      <span className="h-[3px] w-[10px] rounded-full" style={{ background: color }} />
      {label}
    </span>
  );
}

function Rule({
  title,
  count,
  tone,
  children,
}: {
  title: string;
  count?: number;
  tone?: 'warn';
  children?: React.ReactNode;
}) {
  return (
    <div className="mb-2 flex items-center gap-2.5">
      <span className="text-[11px] font-medium text-ink-2">{title}</span>
      {count != null && (
        <span
          className="num text-[10.5px]"
          style={{ color: tone === 'warn' ? 'var(--s-warn)' : 'var(--s-ink-3)' }}
        >
          {count}
        </span>
      )}
      <span className="h-px flex-1 bg-line-soft" />
      {children}
    </div>
  );
}

function GameRow({
  game,
  latest,
  onOpen,
  onToggle,
  onInstall,
}: {
  game: Game;
  latest: string;
  onOpen: () => void;
  onToggle: () => void;
  onInstall: () => void;
}) {
  const st = statusOf(game, latest);
  const dot =
    st.tone === 'ok' ? 'var(--s-ok)' : st.tone === 'warn' ? 'var(--s-warn)' : 'var(--s-ink-3)';
  return (
    <div className="group flex items-center gap-3 rounded-[10px] px-1.5 py-1.5 transition-colors hover:bg-surface-2/70">
      <button onClick={onOpen} className="shrink-0">
        <Cover
          name={game.name}
          src={game.cover}
          className="h-[34px] w-[56px] rounded-[6px]"
          glyph={13}
        />
      </button>
      <button onClick={onOpen} className="min-w-0 flex-1 text-left">
        <p className="truncate text-[12px] font-medium leading-tight text-ink">{game.name}</p>
        <p className="mt-1 flex items-center gap-1.5 truncate text-[10px] leading-tight text-ink-3">
          <span className="h-[4px] w-[4px] shrink-0 rounded-full" style={{ background: dot }} />
          <span className="mono">{game.anr?.version ?? '未安装'}</span>
          <span className="opacity-40">·</span>
          {SOURCE_LABEL[game.source]}
          <span className="opacity-40">·</span>
          {relTime(game.lastUsed)}
        </p>
      </button>
      {game.anr ? (
        <Toggle checked={game.anr.enabled} onChange={onToggle} />
      ) : (
        <button onClick={onInstall} className="btn btn-subtle h-7 shrink-0 text-[11px]">
          安装
        </button>
      )}
    </div>
  );
}

function QuickRow({
  icon,
  label,
  disabled,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={clsx(
        'flex w-full items-center gap-2.5 rounded-[8px] px-2.5 py-2 text-left text-[11.5px] transition-colors',
        disabled ? 'cursor-default text-ink-3' : 'text-ink-2 hover:bg-surface-2 hover:text-ink',
      )}
    >
      <span className="text-ink-3">{icon}</span>
      <span className="flex-1 truncate">{label}</span>
      {!disabled && <ArrowRight size={11} className="text-ink-3" />}
    </button>
  );
}
