import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowRight,
  FileText,
  LayoutDashboard,
  Library,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Tag,
} from 'lucide-react';
import { clsx, versionKnown } from '../lib';
import { useApp } from '../store';
import type { View } from '../types';

interface Cmd {
  id: string;
  label: string;
  hint?: string;
  group: string;
  icon: React.ReactNode;
  run: () => void;
}

export function CommandPalette() {
  const {
    paletteOpen,
    setPalette,
    games,
    setView,
    openDrawer,
    addGame,
    setTheme,
    setSettingsOpen,
    theme,
    versions,
    switchVersion,
    toast,
  } = useApp();
  const [q, setQ] = useState('');
  const [idx, setIdx] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (paletteOpen) {
      setQ('');
      setIdx(0);
      setTimeout(() => inputRef.current?.focus(), 30);
    }
  }, [paletteOpen]);

  const latest = versions.find((v) => v.latest)?.tag ?? versions[0]?.tag ?? '—';
  const outdated = versionKnown(latest)
    ? games.filter((g) => g.anr && g.anr.version !== latest)
    : [];

  const items = useMemo<Cmd[]>(() => {
    const go = (v: View) => () => {
      setView(v);
      setPalette(false);
    };
    const all: Cmd[] = [
      {
        id: 'a-add',
        label: '添加游戏目录',
        hint: '手动选择 .exe 所在文件夹',
        group: '操作',
        icon: <Plus size={13} />,
        run: () => {
          addGame();
          setPalette(false);
        },
      },
      {
        id: 'a-update',
        label: `更新全部可更新游戏（${outdated.length}）`,
        hint: `升级到 ${latest}`,
        group: '操作',
        icon: <RefreshCw size={13} />,
        run: () => {
          outdated.forEach((g) => switchVersion(g.id, latest));
          setPalette(false);
        },
      },
      {
        id: 'a-theme',
        label: theme === 'dark' ? '切换到浅色主题' : '切换到深色主题',
        group: '操作',
        icon: <RefreshCw size={13} />,
        run: () => {
          setTheme(theme === 'dark' ? 'light' : 'dark');
          setPalette(false);
        },
      },
      {
        id: 'a-diag',
        label: '生成全部诊断包',
        hint: '日志 + 配置 + 系统信息',
        group: '操作',
        icon: <FileText size={13} />,
        run: () => {
          toast({ kind: 'ok', title: '诊断包已生成', desc: 'anr-studio-diag.zip' });
          setPalette(false);
        },
      },
      {
        id: 'p-overview',
        label: '总览',
        group: '页面',
        icon: <LayoutDashboard size={13} />,
        run: go('overview'),
      },
      { id: 'p-library', label: '游戏库', group: '页面', icon: <Library size={13} />, run: go('library') },
      { id: 'p-versions', label: '版本', group: '页面', icon: <Tag size={13} />, run: go('versions') },
      { id: 'p-logs', label: '日志诊断', group: '页面', icon: <FileText size={13} />, run: go('logs') },
      { id: 'p-settings', label: '打开设置', group: '操作', icon: <Settings size={13} />, run: () => { setSettingsOpen(true); setPalette(false); } },
      ...games.map<Cmd>((g) => ({
        id: `g-${g.id}`,
        label: g.name,
        hint: g.anr ? `${g.anr.version} · ${g.anr.enabled ? '已启用' : '已关闭'}` : '未安装',
        group: '游戏',
        icon: <ArrowRight size={13} />,
        run: () => {
          openDrawer(g.id);
          setPalette(false);
        },
      })),
    ];
    const key = q.trim().toLowerCase();
    if (!key) return all.filter((i) => i.group !== '游戏').concat(all.filter((i) => i.group === '游戏').slice(0, 4));
    return all.filter(
      (i) => i.label.toLowerCase().includes(key) || (i.hint ?? '').toLowerCase().includes(key),
    );
  }, [q, games, theme, outdated, latest, setView, setPalette, addGame, setTheme, openDrawer, toast, switchVersion]);

  useEffect(() => setIdx(0), [q]);

  if (!paletteOpen) return null;

  const groups = ['操作', '页面', '游戏'].filter((g) => items.some((i) => i.group === g));

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-50">
      <div
        className="anim-overlay absolute inset-0 bg-black/45"
        onClick={() => setPalette(false)}
        aria-hidden
      />
      <div className="relative mx-auto mt-[12vh] w-[560px]">
        <div className="anim-panel overflow-hidden rounded-[16px] border border-line bg-surface shadow-[var(--s-shadow)]">
          <div className="flex items-center gap-2.5 border-b border-line px-4">
            <Search size={15} className="text-ink-3" />
            <input
              ref={inputRef}
              value={q}
              onChange={(e) => setQ(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'ArrowDown') {
                  e.preventDefault();
                  setIdx((i) => Math.min(items.length - 1, i + 1));
                } else if (e.key === 'ArrowUp') {
                  e.preventDefault();
                  setIdx((i) => Math.max(0, i - 1));
                } else if (e.key === 'Enter') {
                  e.preventDefault();
                  items[idx]?.run();
                } else if (e.key === 'Escape') {
                  setPalette(false);
                }
              }}
              placeholder="搜索游戏、页面或操作…"
              className="h-[46px] flex-1 bg-transparent text-[13px] outline-none placeholder:text-ink-3 focus-visible:shadow-none"
            />
            <kbd className="kbd">Esc</kbd>
          </div>

          <div className="scroll-thin max-h-[340px] overflow-y-auto p-1.5">
            {items.length === 0 && (
              <p className="px-3 py-8 text-center text-[12px] text-ink-3">没有匹配的结果</p>
            )}
            {groups.map((group) => (
              <div key={group} className="mb-1">
                <p className="px-2.5 py-1.5 text-[10px] font-medium tracking-[0.06em] text-ink-3">
                  {group}
                </p>
                {items
                  .filter((i) => i.group === group)
                  .map((item) => {
                    const active = items[idx]?.id === item.id;
                    return (
                      <button
                        key={item.id}
                        onMouseEnter={() => setIdx(items.findIndex((x) => x.id === item.id))}
                        onClick={item.run}
                        className={clsx(
                          'flex w-full items-center gap-2.5 rounded-[8px] px-2.5 py-2 text-left transition-colors',
                          active ? 'bg-accent-soft text-accent' : 'text-ink-2 hover:bg-surface-2',
                        )}
                      >
                        <span className={active ? 'text-accent' : 'text-ink-3'}>{item.icon}</span>
                        <span
                          className={clsx(
                            'flex-1 truncate text-[12.5px]',
                            active ? 'font-medium' : 'text-ink',
                          )}
                        >
                          {item.label}
                        </span>
                        {item.hint && (
                          <span className="shrink-0 text-[10.5px] text-ink-3">{item.hint}</span>
                        )}
                      </button>
                    );
                  })}
              </div>
            ))}
          </div>

          <div className="flex items-center gap-3 border-t border-line px-4 py-2 text-[10.5px] text-ink-3">
            <span className="flex items-center gap-1">
              <kbd className="kbd">↑</kbd>
              <kbd className="kbd">↓</kbd> 选择
            </span>
            <span className="flex items-center gap-1">
              <kbd className="kbd">↵</kbd> 执行
            </span>
            <span className="ml-auto">共 {items.length} 项</span>
          </div>
        </div>
      </div>
    </div>
  );
}
