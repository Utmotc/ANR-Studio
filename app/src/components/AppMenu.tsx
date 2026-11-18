import { useEffect, useRef, useState } from 'react';
import { Check, ChevronRight, ExternalLink, FolderOpen, RefreshCw } from 'lucide-react';
import { call, isTauri } from '../lib/ipc';
import { FONT_LABELS, useApp } from '../store';

const APP_VERSION = typeof __APP_VERSION__ === 'string' ? __APP_VERSION__ : '0.1.0';

interface Item {
  label?: string;
  children?: Item[];
  onClick?: () => void;
  checked?: boolean;
  sep?: boolean;
  danger?: boolean;
}

function Panel({ items, onClose }: { items: Item[]; onClose: () => void }) {
  const [sub, setSub] = useState<number | null>(null);

  return (
    <div className="menu-panel" onMouseLeave={() => setSub(null)}>
      {items.map((it, i) =>
        it.sep ? (
          <div key={`sep-${i}`} className="menu-sep" />
        ) : (
          <div key={i} className="relative" onMouseEnter={() => setSub(it.children ? i : null)}>
            <button
              className="menu-item"
              data-danger={it.danger ? 'true' : 'false'}
              onClick={() => {
                if (it.children) {
                  setSub(i);
                  return;
                }
                it.onClick?.();
                onClose();
              }}
            >
              <span className="flex-1 truncate">{it.label}</span>
              {it.checked && <Check size={12} className="text-accent" />}
              {it.children && <ChevronRight size={12} className="text-ink-3" />}
            </button>
            {it.children && sub === i && <Panel items={it.children} onClose={onClose} />}
          </div>
        ),
      )}
    </div>
  );
}

export function AppMenu({ onAbout }: { onAbout: () => void }) {
  const { settings, updateSettings, setTheme, setView, rescan, toast } = useApp();
  const [open, setOpen] = useState<number | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(null);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(null);
    };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, []);

  const openData = async () => {
    try {
      const p = await call<{ data: string }>('app_paths');
      await call('open_path', { path: p.data });
    } catch {
      toast({ kind: 'info', title: '需要在桌面应用里使用' });
    }
  };

  const openUrl = (url: string) => {
    if (isTauri) void call('open_url', { url }).catch(() => undefined);
    else window.open(url, '_blank');
  };

  const menus: { label: string; items: Item[] }[] = [
    {
      label: '界面',
      items: [
        {
          label: '主题',
          children: [
            { label: '深色', checked: settings.theme === 'dark', onClick: () => setTheme('dark') },
            { label: '浅色', checked: settings.theme === 'light', onClick: () => setTheme('light') },
            {
              label: '跟随系统',
              checked: settings.theme === 'system',
              onClick: () => setTheme('system'),
            },
          ],
        },
        {
          label: '界面密度',
          children: [
            {
              label: '标准',
              checked: settings.density === 'cozy',
              onClick: () => updateSettings({ density: 'cozy' }),
            },
            {
              label: '紧凑',
              checked: settings.density === 'compact',
              onClick: () => updateSettings({ density: 'compact' }),
            },
          ],
        },
        {
          label: '字体大小',
          children: FONT_LABELS.map((label, i) => ({
            label,
            checked: settings.fontSize === i,
            onClick: () => updateSettings({ fontSize: i }),
          })),
        },
      ],
    },
    {
      label: '工具',
      items: [
        {
          label: '重新扫描游戏库',
          onClick: () => {
            void rescan();
            toast({ kind: 'info', title: '正在重新扫描' });
          },
        },
        { label: '版本与缓存', onClick: () => setView('versions') },
        { label: '日志诊断', onClick: () => setView('logs') },
        {
          label: '打开数据目录',
          onClick: () => {
            void openData();
          },
        },
      ],
    },
    {
      label: '关于',
      items: [
        { label: '软件说明', onClick: onAbout },
        {
          label: 'ANR 官方仓库',
          onClick: () => openUrl('https://github.com/apex-compute/ANR'),
        },
      ],
    },
  ];

  return (
    <div className="titlebar-menu" ref={ref}>
      {menus.map((m, i) => (
        <div key={m.label} className="relative">
          <button
            className="menu-btn"
            data-open={open === i ? 'true' : 'false'}
            onClick={() => setOpen(open === i ? null : i)}
            onMouseEnter={() => {
              if (open !== null) setOpen(i);
            }}
          >
            {m.label}
          </button>
          {open === i && <Panel items={m.items} onClose={() => setOpen(null)} />}
        </div>
      ))}
    </div>
  );
}

export function AboutDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  if (!open) return null;
  const openUrl = (url: string) => {
    if (isTauri) void call('open_url', { url }).catch(() => undefined);
    else window.open(url, '_blank');
  };

  return (
    <div className="fixed inset-0 z-[80] grid place-items-center">
      <div className="anim-overlay absolute inset-0 bg-black/50" onClick={onClose} aria-hidden />
      <div className="anim-panel relative w-[440px] rounded-[16px] border border-line bg-surface p-5 shadow-[var(--s-shadow)]">
        <h3 className="text-[15px] font-medium">ANR Studio</h3>
        <p className="mono mt-1 text-[11px] text-ink-3">v{APP_VERSION} · 非官方工具</p>

        <p className="mt-3 text-[12px] leading-relaxed text-ink-2">
          给 Apex Neural Rendering 做的图形化管理工具：统一管理多个游戏的安装状态、切换版本、开关帧生成、看日志、找问题。
        </p>
        <p className="mt-2 text-[12px] leading-relaxed text-ink-2">
          不修改游戏本体，不注入内存，安装与卸载都复用官方脚本，卸载前会自动快照。
        </p>

        <div className="mt-4 space-y-1.5 rounded-[10px] border border-line bg-surface-2/50 px-3 py-2.5">
          <Row k="管理对象" v="ANR FG（Apex Neural Rendering）" />
          <Row k="游戏来源" v="Steam · Epic · GOG · 手动添加" />
          <Row k="数据位置" v="%LOCALAPPDATA%\\app.anrstudio.desktop" />
        </div>

        <div className="mt-4 flex flex-wrap gap-3">
          <button
            className="inline-flex items-center gap-1 text-[11.5px] text-accent hover:underline"
            onClick={() => openUrl('https://github.com/apex-compute/ANR')}
          >
            ANR 官方仓库
            <ExternalLink size={11} />
          </button>
          <button
            className="inline-flex items-center gap-1 text-[11.5px] text-accent hover:underline"
            onClick={() => openUrl('https://github.com/optiscaler/OptiScaler')}
          >
            OptiScaler（GPL-3.0）
            <ExternalLink size={11} />
          </button>
        </div>

        <div className="mt-5 flex justify-end">
          <button className="btn btn-primary h-8" onClick={onClose}>
            知道了
          </button>
        </div>
      </div>
    </div>
  );
}

function Row({ k, v }: { k: string; v: string }) {
  return (
    <div className="flex items-baseline justify-between gap-3">
      <span className="shrink-0 text-[11px] text-ink-3">{k}</span>
      <span className="min-w-0 truncate text-right text-[11px] text-ink-2">{v}</span>
    </div>
  );
}
