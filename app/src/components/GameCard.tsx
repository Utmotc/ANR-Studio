import { useState } from 'react';
import {
  Check,
  ChevronRight,
  Copy,
  FolderCog,
  FolderOpen,
  ImageIcon,
  ImageOff,
  Info,
  Pencil,
  RefreshCw,
  Trash2,
} from 'lucide-react';
import { clsx, relTime, SOURCE_LABEL, statusOf, TONE_COLOR } from '../lib';
import { call, isTauri } from '../lib/ipc';
import { useApp } from '../store';
import type { Game } from '../types';
import { ContextMenu, Cover, StatusTag, Toggle, type MenuAction } from './ui';

export function GameCard({ game, compact }: { game: Game; compact?: boolean }) {
  const {
    versions,
    selected,
    toggleSelect,
    openDrawer,
    install,
    toggleEnabled,
    toast,
    refreshCover,
    pickCover,
    setRenameId,
    setMoveDir,
    changeGameDir,
    clearCover,
    removeGame,
    uninstall,
  } = useApp();
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const latest = versions.find((v) => v.latest)?.tag ?? versions[0]?.tag ?? '—';
  const st = statusOf(game, latest);
  const isSelected = selected.includes(game.id);

  const changeDir = async () => {
    if (!isTauri) {
      toast({ kind: 'info', title: '浏览器预览模式不支持改目录' });
      return;
    }
    let picked: string | null = null;
    try {
      picked = await call<string | null>('pick_folder');
    } catch {
      return;
    }
    if (!picked) return;
    // 旧目录装着 ANR 时，让用户选"迁移"还是"只改目录"
    if (game.anr) setMoveDir({ id: game.id, path: picked });
    else await changeGameDir(game.id, picked, false);
  };

  const menuItems: MenuAction[] = [
    // 目录类
    { label: '更改游戏目录', icon: <FolderCog size={12} />, onClick: () => void changeDir() },
    {
      label: '打开游戏目录',
      icon: <FolderOpen size={12} />,
      onClick: () => {
        if (isTauri) void call('open_path', { path: game.dir }).catch(() => undefined);
        else toast({ kind: 'info', title: '已打开目录', desc: game.dir });
      },
    },
    {
      label: '复制目录路径',
      icon: <Copy size={12} />,
      onClick: () => {
        navigator.clipboard?.writeText(game.dir);
        toast({ kind: 'info', title: '路径已复制' });
      },
    },
    game.anr
      ? {
          label: '卸载并删除',
          icon: <Trash2 size={12} />,
          danger: true,
          onClick: () => void uninstall(game.id, true),
        }
      : {
          label: '从列表移除',
          icon: <Trash2 size={12} />,
          danger: true,
          onClick: () => void removeGame(game.id),
        },

    { label: '', onClick: () => undefined, sep: true },

    // 名称与封面
    { label: '修改游戏名', icon: <Pencil size={12} />, onClick: () => setRenameId(game.id) },
    {
      label: '重新搜索封面',
      icon: <RefreshCw size={12} />,
      onClick: () => void refreshCover(game.id),
    },
    {
      label: '删除封面',
      icon: <ImageOff size={12} />,
      disabled: !game.cover,
      onClick: () => void clearCover(game.id),
    },
    {
      label: '选择本地图片作封面',
      icon: <ImageIcon size={12} />,
      onClick: () => void pickCover(game.id),
    },

    { label: '', onClick: () => undefined, sep: true },

    { label: '查看详情', icon: <Info size={12} />, onClick: () => openDrawer(game.id) },
  ];

  return (
    <article
      onContextMenu={(e) => {
        e.preventDefault();
        setMenu({ x: e.clientX, y: e.clientY });
      }}
      className={clsx(
        'group relative overflow-hidden rounded-[14px] border bg-surface transition-colors',
        isSelected ? 'border-accent-line' : 'border-line hover:border-surface-3',
      )}
    >
      <div className="relative">
        <Cover
          name={game.name}
          src={game.cover}
          className="aspect-[16/10]"
          glyph={compact ? 30 : 34}
        />
        <button
          onClick={(e) => {
            e.stopPropagation();
            toggleSelect(game.id);
          }}
          aria-label="选择"
          className={clsx(
            'absolute left-2 top-2 grid h-[18px] w-[18px] place-items-center rounded-full border transition-all',
            isSelected
              ? 'border-transparent bg-accent text-white opacity-100'
              : 'border-white/60 bg-black/25 text-transparent opacity-0 group-hover:opacity-100',
          )}
        >
          <Check size={11} strokeWidth={3} />
        </button>
        <span className="absolute right-2 top-2 inline-flex items-center gap-1.5 rounded-full bg-black/55 px-2 py-[3px] text-[10.5px] text-white/95">
          <span
            className="h-[5px] w-[5px] rounded-full"
            style={{ background: TONE_COLOR[st.tone] }}
          />
          {st.label}
        </span>
      </div>

      <div className="card-pad">
        <button onClick={() => openDrawer(game.id)} className="block w-full text-left">
          <h3 className="truncate text-[12.5px] font-medium text-ink">{game.name}</h3>
          {compact ? (
            <div className="mt-1 flex items-baseline justify-between gap-2 text-[10.5px] text-ink-3">
              <span className="truncate">
                {SOURCE_LABEL[game.source]} · {relTime(game.lastUsed)}
              </span>
              <span className="mono shrink-0 text-ink-2">{game.anr?.version ?? '—'}</span>
            </div>
          ) : (
            <p className="mt-0.5 truncate text-[11px] text-ink-3">
              {SOURCE_LABEL[game.source]}
              {game.anr ? ` · ${game.anr.version}` : ' · 未安装'}
              {game.anr?.kind === 'addon' && ' · 插件'}
            </p>
          )}
        </button>

        {!compact && (
          <div className="mt-3 flex items-center gap-2">
            {game.anr ? (
              <>
                <Toggle checked={game.anr.enabled} onChange={() => toggleEnabled(game.id)} />
                <span className="text-[11px] text-ink-3">帧生成</span>
                <button
                  onClick={() => openDrawer(game.id)}
                  className="btn btn-subtle ml-auto h-7 px-1.5 text-[11.5px]"
                >
                  详情
                  <ChevronRight size={13} />
                </button>
              </>
            ) : (
              <>
                <button
                  onClick={() => install(game.id)}
                  className="btn btn-primary h-7 flex-1 text-[11.5px]"
                >
                  安装
                </button>
                <button
                  onClick={() => {
                    if (isTauri) void call('open_path', { path: game.dir }).catch(() => undefined);
                    else toast({ kind: 'info', title: '已打开目录', desc: game.dir });
                  }}
                  className="btn btn-ghost h-7 w-7 px-0"
                  aria-label="打开目录"
                >
                  <FolderOpen size={13} />
                </button>
              </>
            )}
          </div>
        )}
      </div>

      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={menuItems} onClose={() => setMenu(null)} />
      )}
    </article>
  );
}

export function GameRow({ game, latest }: { game: Game; latest: string }) {
  const { selected, toggleSelect, openDrawer, toggleEnabled } = useApp();
  const st = statusOf(game, latest);
  const isSelected = selected.includes(game.id);

  return (
    <div
      className={clsx(
        'group row-pad flex items-center gap-3 rounded-[10px] border bg-surface px-3 transition-colors',
        isSelected ? 'border-accent-line' : 'border-line hover:border-surface-3',
      )}
    >
      <button
        onClick={() => toggleSelect(game.id)}
        aria-label="选择"
        className={clsx(
          'grid h-[16px] w-[16px] shrink-0 place-items-center rounded-full border transition-all',
          isSelected
            ? 'border-transparent bg-accent text-white'
            : 'border-line text-transparent hover:border-ink-3',
        )}
      >
        <Check size={10} strokeWidth={3} />
      </button>
      <Cover
        name={game.name}
        src={game.cover}
        className="h-9 w-9 shrink-0 rounded-[6px]"
        glyph={14}
      />
      <button onClick={() => openDrawer(game.id)} className="min-w-0 flex-1 text-left">
        <div className="flex items-center gap-2">
          <span className="truncate text-[12.5px] font-medium">{game.name}</span>
          <StatusTag label={st.label} tone={st.tone} />
          {game.running && (
            <span
              className="rounded-full px-1.5 py-[1px] text-[9.5px] leading-none"
              style={{ background: 'var(--s-warn-soft)', color: 'var(--s-warn)' }}
              title="游戏正在运行，安装 / 卸载 / 切换已暂停"
            >
              运行中
            </span>
          )}
        </div>
        <p className="mono mt-0.5 truncate text-[10.5px] text-ink-3">{game.dir}</p>
      </button>
      <div className="mono hidden w-[62px] shrink-0 text-[11px] text-ink-2 md:block">
        {game.anr?.version ?? '—'}
      </div>
      <div className="hidden w-[52px] shrink-0 text-[11px] text-ink-3 md:block">
        {SOURCE_LABEL[game.source]}
      </div>
      <div className="w-[34px] shrink-0">
        {game.anr && (
          <Toggle checked={game.anr.enabled} onChange={() => toggleEnabled(game.id)} />
        )}
      </div>
    </div>
  );
}
