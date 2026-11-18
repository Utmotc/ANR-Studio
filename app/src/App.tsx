import { useEffect, useState } from 'react';
import { TitleBar } from './components/TitleBar';
import { Sidebar, TopBar } from './components/Chrome';
import { Drawer } from './components/Drawer';
import { CommandPalette } from './components/CommandPalette';
import {
  AddGameDialog,
  CloseDialog,
  ConfirmDialog,
  FailureDialog,
  ImportPackageDialog,
  MarkVersionDialog,
  MoveDirDialog,
  Onboarding,
  RenameDialog,
  TaskDock,
} from './components/Overlays';
import { Overview } from './views/Overview';
import { Library } from './views/Library';
import { Versions } from './views/Versions';
import { Tuning } from './views/Tuning';
import { Logs } from './views/Logs';
import { SettingsDialog } from './views/Settings';
import { useApp } from './store';
import { isTauri, onCloseRequest, onCover, onTask } from './lib/ipc';
import { applyLocale, type Locale } from './i18n';
import { installFocusTrap } from './lib/focus';

export default function App() {
  const {
    view,
    setPalette,
    paletteOpen,
    closeDrawer,
    confirm,
    settings,
    init,
    ready,
    install,
    addGame,
    toast,
  } = useApp();
  const locale: Locale = useApp((s) => (s.settings.locale === 'en' ? 'en' : 'zh'));
  const [dropping, setDropping] = useState(false);

  useEffect(() => {
    void init();
  }, [init]);

  // 语言切换后用 key 重新挂载整棵树：中文是源码原样，其它语言在新树上做字典覆盖
  useEffect(() => {
    applyLocale(locale);
  }, [locale]);

  // 6.5.12：弹窗内 Tab 循环 + 打开时自动聚焦
  useEffect(() => installFocusTrap(), []);

  // 把文件夹拖进窗口 = 添加游戏
  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;

    void (async () => {
      const { getCurrentWebview } = await import('@tauri-apps/api/webview');
      const un = await getCurrentWebview().onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === 'over' || p.type === 'enter') {
          setDropping(true);
        } else if (p.type === 'drop') {
          setDropping(false);
          const dirs = p.paths ?? [];
          if (!dirs.length) return;
          void (async () => {
            let ok = 0;
            for (const d of dirs) {
              const before = useApp.getState().games.length;
              await addGame(d);
              if (useApp.getState().games.length > before) ok += 1;
            }
            if (ok > 0) {
              toast({ kind: 'ok', title: `已添加 ${ok} 个游戏` });
            } else {
              toast({
                kind: 'warn',
                title: '没有添加任何游戏',
                desc: '拖进来的文件夹里没有 .exe，或者已经添加过了',
              });
            }
          })();
        } else {
          setDropping(false);
        }
      });
      if (disposed) un();
      else unlisten = un;
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [addGame, toast]);

  useEffect(() => {
    document.documentElement.dataset.theme = settings.theme === 'light' ? 'light' : 'dark';
    document.documentElement.dataset.density = settings.density === 'compact' ? 'compact' : 'cozy';
  }, [settings.theme, settings.density]);

  useEffect(() => {
    let off: (() => void) | undefined;
    void onTask((p) => useApp.getState().upsertTask(p)).then((fn) => {
      off = fn;
    });
    return () => off?.();
  }, []);

  useEffect(() => {
    let off: (() => void) | undefined;
    void onCloseRequest(() => useApp.getState().setClosePrompt(true)).then((fn) => {
      off = fn;
    });
    return () => off?.();
  }, []);

  useEffect(() => {
    let off: (() => void) | undefined;
    void onCover((p) => useApp.getState().setCover(p.id, p.cover)).then((fn) => {
      off = fn;
    });
    return () => off?.();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setPalette(true);
      }
      if (e.key === 'Escape') {
        if (paletteOpen) setPalette(false);
        else if (!confirm) closeDrawer();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [paletteOpen, confirm, setPalette, closeDrawer]);

  useEffect(() => {
    if (isTauri) return;
    const demo = new URLSearchParams(window.location.search).get('demo');
    if (!demo) return;
    const t = setTimeout(() => {
      if (demo === 'install') void install('wukong');
    }, 600);
    return () => clearTimeout(t);
  }, [install]);

  return (
    <div key={locale} className="flex h-full w-full flex-col overflow-hidden bg-bg">
      <TitleBar />
      <div className="flex min-h-0 flex-1 overflow-hidden">
        <Sidebar />
        <div className="flex min-w-0 flex-1 flex-col">
          <TopBar />
          <main className="min-h-0 flex-1">
            {!ready ? (
              <div className="grid h-full place-items-center">
                <p className="text-[12px] text-ink-3">正在扫描游戏库…</p>
              </div>
            ) : (
              <>
                {view === 'overview' && <Overview />}
                {view === 'library' && <Library />}
                {view === 'versions' && <Versions />}
                {view === 'tuning' && <Tuning />}
                {view === 'logs' && <Logs />}
              </>
            )}
          </main>
        </div>
      </div>
      <Drawer />
      <CommandPalette />
      <ConfirmDialog />
      <FailureDialog />
      <SettingsDialog />
      <AddGameDialog />
      <ImportPackageDialog />
      <MarkVersionDialog />
      <RenameDialog />
      <MoveDirDialog />
      <CloseDialog />
      <Onboarding />
      <TaskDock />

      {dropping && (
        <div className="pointer-events-none fixed inset-0 z-[95] grid place-items-center bg-black/40">
          <div className="rounded-[16px] border-2 border-dashed border-white/60 px-10 py-8 text-center">
            <p className="text-[15px] font-medium text-white">松开以添加游戏</p>
            <p className="mt-1 text-[11.5px] text-white/70">把游戏主程序所在的文件夹拖进来</p>
          </div>
        </div>
      )}
    </div>
  );
}
