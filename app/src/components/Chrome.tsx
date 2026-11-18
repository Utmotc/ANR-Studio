import { useEffect, useState } from 'react';
import {
  Check,
  FileText,
  LayoutDashboard,
  Library,
  Moon,
  Search,
  Settings as SettingsIcon,
  SlidersHorizontal,
  Sun,
  Tag,
} from 'lucide-react';

import { useApp } from '../store';
import { LOCALES, type Locale } from '../i18n';
import { Flag } from '../i18n/flags';
import type { View } from '../types';

const NAV: { view: View; label: string; icon: typeof Library }[] = [
  { view: 'overview', label: '总览', icon: LayoutDashboard },
  { view: 'library', label: '游戏库', icon: Library },
  { view: 'versions', label: '版本', icon: Tag },
  { view: 'tuning', label: '画面调校', icon: SlidersHorizontal },
  { view: 'logs', label: '日志诊断', icon: FileText },
];


export function Sidebar() {
  const { view, setView, games, tasks, versions, connected, setSettingsOpen } = useApp();
  const running = tasks.filter((t) => t.status === 'running').length;

  return (
    <aside className="flex h-full w-[var(--dens-side)] shrink-0 flex-col border-r border-line bg-surface">
      <div className="px-4 pb-3 pt-4">
        <div className="text-[13px] font-medium leading-tight text-ink">ANR Studio</div>
        <div className="mt-[3px] text-[10px] leading-tight text-ink-3">非官方管理工具</div>
      </div>

      <nav className="flex flex-col gap-0.5 px-2.5 py-2">
        {NAV.map((item) => {
          const Icon = item.icon;
          return (
            <button
              key={item.view}
              onClick={() => setView(item.view)}
              data-active={view === item.view}
              className="nav-item"
            >
              <Icon size={15} strokeWidth={1.8} />
              <span className="flex-1 text-left">{item.label}</span>
              {item.view === 'logs' && running > 0 && (
                <span className="grid h-[16px] min-w-[16px] place-items-center rounded-full bg-accent text-[9.5px] font-medium text-white">
                  {running}
                </span>
              )}
            </button>
          );
        })}
      </nav>

      <div className="mt-auto p-2.5">
        <button
          onClick={() => setSettingsOpen(true)}
          className="nav-item w-full"
        >
          <SettingsIcon size={15} strokeWidth={1.8} />
          <span className="flex-1 text-left">设置</span>
        </button>
      </div>
    </aside>
  );
}


const TITLES: Record<View, { title: string; sub: string }> = {
  overview: { title: '总览', sub: '一眼看清这台机器上的安装情况' },
  library: { title: '游戏库', sub: '统一管理每个游戏的安装状态' },
  versions: { title: '版本', sub: '切换与回滚 ANR FG 版本' },
  logs: { title: '日志诊断', sub: '排查问题并生成可以提交的诊断包' },
  tuning: { title: '画面调校', sub: '用中文调 OptiScaler 的参数，改完重启游戏生效' },
};

/** 右上角语言切换：默认中文，点国旗打开菜单；位置在深浅色图标之前 */
function LanguageSwitch() {
  const { settings, updateSettings } = useApp();
  const [open, setOpen] = useState(false);
  const locale: Locale = settings.locale === 'en' ? 'en' : 'zh';
  const current = LOCALES.find((l) => l.id === locale) ?? LOCALES[0];

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const root = document.getElementById('lang-switch');
      if (root && !root.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  return (
    <div id="lang-switch" className="relative">
      <button
        className="btn btn-icon"
        onClick={() => setOpen((v) => !v)}
        aria-label={locale === 'zh' ? '切换界面语言' : 'Switch language'}
        aria-expanded={open}
        title={current.name}
      >
        <Flag locale={locale} />
      </button>
      {open && (
        <div className="menu-panel left-auto right-0 w-[168px]">
          {LOCALES.map((l) => (
            <button
              key={l.id}
              className="menu-item rounded-[6px] text-[11.5px]"
              onClick={() => {
                updateSettings({ locale: l.id });
                setOpen(false);
              }}
            >
              <Flag locale={l.id} size={16} />
              <span className="flex-1">{l.name}</span>
              {locale === l.id && <Check size={12} className="text-accent" />}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

export function TopBar() {
  const { view, theme, setTheme, setPalette } = useApp();
  const t = TITLES[view];

  return (
    <header className="flex h-[var(--dens-topbar)] shrink-0 items-center gap-3 border-b border-line px-6">
      <div className="min-w-0 flex-1">
        <h1 className="text-[15px] font-medium leading-none text-ink">{t.title}</h1>
        <p className="mt-1 truncate text-[11.5px] text-ink-3">{t.sub}</p>
      </div>

      <button
        onClick={() => setPalette(true)}
        className="flex h-8 w-[188px] items-center gap-2 rounded-[8px] border border-line bg-surface px-2.5 text-[12px] text-ink-3 transition-colors hover:border-surface-3 hover:text-ink-2"
      >
        <Search size={13} />
        <span className="flex-1 text-left">搜索游戏或操作</span>
        <kbd className="kbd">Ctrl K</kbd>
      </button>

      <LanguageSwitch />

      <button
        onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')}
        className="btn btn-icon"
        aria-label="切换主题"
        title={theme === 'dark' ? '切换到浅色' : '切换到深色'}
      >
        {theme === 'dark' ? <Sun size={15} /> : <Moon size={15} />}
      </button>
    </header>
  );
}
