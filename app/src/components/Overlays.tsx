import { useEffect, useState } from 'react';
import {
  AlertTriangle,
  Bug,
  Check,
  FileArchive,
  FolderOpen,
  Gamepad2,
  RefreshCw,
  RotateCcw,
  Upload,
  X,
} from 'lucide-react';
import { clsx } from '../lib';
import { call, errorText, isTauri } from '../lib/ipc';
import { useApp } from '../store';
import { Progress } from './ui';

export function TaskDock() {
  const { tasks, toasts, cancelTask, dismissToast } = useApp();

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
            <span className="mt-[5px] h-[6px] w-[6px] shrink-0 rounded-full" style={{ background: color }} />
            <div className="min-w-0 flex-1">
              <p className="truncate text-[12px] font-medium">{t.title}</p>
              {t.desc && <p className="mt-0.5 truncate text-[10.5px] text-ink-3">{t.desc}</p>}
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

      {tasks.map((t) => (
        <div
          key={t.id}
          className="anim-toast pointer-events-auto rounded-[12px] border border-line bg-surface p-3 shadow-[var(--s-shadow)]"
        >
          <div className="flex items-center gap-2">
            <span className="flex-1 truncate text-[12.5px] font-medium">{t.title}</span>
            {t.status === 'running' ? (
              <button
                onClick={() => cancelTask(t.id)}
                className="-mr-1 text-ink-3 transition-colors hover:text-ink"
                aria-label="取消任务"
              >
                <X size={13} />
              </button>
            ) : t.status === 'done' ? (
              <Check size={13} className="text-ok" />
            ) : (
              <AlertTriangle size={13} className="text-warn" />
            )}
          </div>
          <p className="mt-0.5 truncate text-[10.5px] text-ink-3">
            {t.subtitle} · {t.stage}
          </p>
          <div className="mt-2 flex items-center gap-2">
            <div className="flex-1">
              <Progress
                value={t.progress}
                tone={t.status === 'error' ? 'danger' : t.status === 'done' ? 'ok' : 'accent'}
              />
            </div>
            <span className="num w-[26px] text-right text-[10px] text-ink-3">
              {Math.round(t.progress)}%
            </span>
          </div>
        </div>
      ))}
    </div>
  );
}

export function Onboarding() {
  const { settings, ready, games, addGame, updateSettings, setView, installAll } = useApp();
  if (settings.onboarded) return null;
  if (!ready) return null;

  // 6.5.8：列出可安装游戏，支持一键全部安装（后端队列会串行执行）
  const installable = games.filter((g) => !g.anr && !g.running);
  const shown = installable.slice(0, 4);

  const installEverything = () => {
    updateSettings({ onboarded: true });
    setView('library');
    void installAll(installable.map((g) => g.id));
  };

  const step = (n: number, title: string, body: string) => (
    <div key={title} className="flex gap-3">
      <span className="mono mt-[1px] grid h-5 w-5 shrink-0 place-items-center rounded-full border border-line text-[10px] text-ink-3">
        {n}
      </span>
      <div className="min-w-0">
        <p className="text-[12px] font-medium leading-tight text-ink">{title}</p>
        <p className="mt-1 text-[11px] leading-relaxed text-ink-3">{body}</p>
      </div>
    </div>
  );

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-[70] grid place-items-center">
      <div className="absolute inset-0 bg-black/55" aria-hidden />
      <div className="anim-panel relative w-[480px] rounded-[16px] border border-line bg-surface p-5 shadow-[var(--s-shadow)]">
        <div className="flex items-center gap-2">
          <span className="grid h-9 w-9 place-items-center rounded-full border border-line">
            <Gamepad2 size={16} className="text-ink-2" />
          </span>
          <div>
            <h3 className="text-[14px] font-medium leading-tight">欢迎使用 ANR Studio</h3>
            <p className="mt-0.5 text-[11px] text-ink-3">
              非官方工具，只调用官方安装脚本，不修改游戏本体
            </p>
          </div>
        </div>

        <div className="mt-4 space-y-3.5">
          {step(
            1,
            '添加游戏',
            '本机会自动扫描 Steam / Epic / GOG 的库；不在列表里的游戏，点「添加游戏目录」或直接把文件夹拖进窗口。',
          )}
          {step(
            2,
            '装 ANR',
            '在卡片上点「安装」。安装前会检查磁盘空间、运行库和加壳情况，装完随时能一键卸载还原。',
          )}
          {step(
            3,
            '进游戏开帧生成',
            '游戏里把「帧生成」打开即可，参数可以在详情抽屉里调，改完重启游戏生效。',
          )}
        </div>

        {games.length > 0 ? (
          <div className="mt-3.5 rounded-[9px] border border-line bg-surface-2/50 px-3 py-2">
            <p className="text-[11px] text-ink-2">
              {installable.length
                ? `已经找到 ${games.length} 个游戏，其中 ${installable.length} 个还没装 ANR：`
                : `已经找到 ${games.length} 个游戏，都已安装过 ANR。`}
            </p>
            {shown.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1.5">
                {shown.map((g) => (
                  <span key={g.id} className="chip max-w-[190px] truncate bg-surface-3/60 px-1.5 py-[2px] text-[10px] text-ink-2">
                    {g.name}
                  </span>
                ))}
                {installable.length > shown.length && (
                  <span className="chip bg-surface-3/60 px-1.5 py-[2px] text-[10px] text-ink-3">
                    +{installable.length - shown.length}
                  </span>
                )}
              </div>
            )}
          </div>
        ) : (
          <p className="mt-3.5 rounded-[9px] border border-warn/30 bg-warn-soft px-3 py-2 text-[11px] text-warn">
            还没发现游戏，可以手动添加一个目录试试
          </p>
        )}

        <div className="mt-4 flex items-center gap-1.5">
          <button
            className="btn btn-ghost h-8 text-[11.5px]"
            onClick={() => {
              setView('library');
              void addGame();
            }}
          >
            <FolderOpen size={12} />
            添加游戏目录
          </button>
          <div className="flex-1" />
          {installable.length > 0 && (
            <button className="btn btn-ghost h-8 text-[11.5px]" onClick={installEverything}>
              <Check size={12} />
              {`一键全部安装（${installable.length}）`}
            </button>
          )}
          <button
            className="btn btn-primary h-8 text-[11.5px]"
            onClick={() => updateSettings({ onboarded: true })}
          >
            开始使用
          </button>
        </div>
      </div>
    </div>
  );
}

export function MoveDirDialog() {
  const { moveDir, setMoveDir, games, changeGameDir } = useApp();
  const game = games.find((g) => g.id === moveDir?.id);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!moveDir) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setMoveDir(null);
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [moveDir, setMoveDir]);

  if (!moveDir || !game) return null;

  const run = async (migrate: boolean) => {
    setBusy(true);
    await changeGameDir(moveDir.id, moveDir.path, migrate);
    setBusy(false);
  };

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-[80] grid place-items-center">
      <div
        className="anim-overlay absolute inset-0 bg-black/50"
        onClick={() => setMoveDir(null)}
        aria-hidden
      />
      <div className="anim-panel relative w-[470px] rounded-[16px] border border-line bg-surface p-5 shadow-[var(--s-shadow)]">
        <h3 className="text-[14px] font-medium">更改游戏目录</h3>
        <p className="mt-1.5 text-[12px] leading-relaxed text-ink-2">
          旧目录里已经装了 ANR。可以连安装一起搬到新目录，也可以只改列表里记的路径。
        </p>

        <div className="mt-3.5 space-y-2 rounded-[10px] border border-line bg-surface-2/50 px-3 py-2.5">
          <div>
            <div className="text-[10px] text-ink-3">旧目录</div>
            <div className="mono mt-0.5 break-all text-[10.5px] text-ink-2">{game.dir}</div>
          </div>
          <div>
            <div className="text-[10px] text-ink-3">新目录</div>
            <div className="mono mt-0.5 break-all text-[10.5px] text-ink-2">{moveDir.path}</div>
          </div>
        </div>

        <ul className="mt-3 space-y-1.5 text-[11px] leading-relaxed text-ink-3">
          <li>
            · <span className="text-ink-2">卸载并迁移</span>：先在旧目录卸载（按哈希还原原文件），再把 ANR 装到新目录
          </li>
          <li>
            · <span className="text-ink-2">只改目录</span>：旧目录的安装原样保留，新目录不会自动安装
          </li>
        </ul>

        <div className="mt-4 flex items-center gap-1.5">
          <div className="flex-1" />
          <button className="btn btn-ghost h-8" disabled={busy} onClick={() => setMoveDir(null)}>
            取消
          </button>
          <button
            className="btn btn-ghost h-8 text-[11.5px]"
            disabled={busy}
            onClick={() => void run(false)}
          >
            只改目录
          </button>
          <button
            className="btn btn-primary h-8 text-[11.5px]"
            disabled={busy}
            onClick={() => void run(true)}
          >
            {busy ? '处理中…' : '卸载并迁移'}
          </button>
        </div>
      </div>
    </div>
  );
}

export function RenameDialog() {
  const { renameId, setRenameId, games, renameGame, settings } = useApp();
  const game = games.find((g) => g.id === renameId);
  const [value, setValue] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (game) setValue(game.name);
  }, [renameId, game]);

  useEffect(() => {
    if (!renameId) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setRenameId(null);
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [renameId, setRenameId]);

  if (!renameId || !game) return null;

  const submit = async () => {
    setBusy(true);
    await renameGame(renameId, value.trim() || null);
    setBusy(false);
  };

  const renamed = !!settings.renames?.[renameId];

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-[80] grid place-items-center">
      <div
        className="anim-overlay absolute inset-0 bg-black/50"
        onClick={() => setRenameId(null)}
        aria-hidden
      />
      <div className="anim-panel relative w-[440px] rounded-[16px] border border-line bg-surface p-5 shadow-[var(--s-shadow)]">
        <h3 className="text-[14px] font-medium">修改游戏名</h3>
        <p className="mt-1.5 text-[12px] leading-relaxed text-ink-2">
          名字会用于搜索封面。如果游戏目录里的名字认不出来（比如是缩写、代号或中文别名），
          改成官方英文名就能搜到正确的封面；改名不会动游戏文件，只是显示名。
        </p>

        <div className="mt-4">
          <label className="mb-1.5 block text-[10.5px] text-ink-3">显示名称</label>
          <input
            autoFocus
            value={value}
            onChange={(e) => setValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void submit();
            }}
            placeholder="例如 Cyberpunk 2077"
            className="h-9 w-full rounded-[8px] border border-line bg-surface px-3 text-[12.5px] text-ink outline-none transition-colors focus:border-accent-line"
          />
          <p className="mono mt-1.5 truncate text-[10px] text-ink-3">{game.dir}</p>
        </div>

        <div className="mt-4 flex items-center gap-1.5">
          {renamed && (
            <button
              className="btn btn-ghost h-8 text-[11.5px]"
              disabled={busy}
              onClick={() => void renameGame(renameId, null)}
            >
              恢复原名
            </button>
          )}
          <div className="flex-1" />
          <button className="btn btn-ghost h-8" onClick={() => setRenameId(null)}>
            取消
          </button>
          <button className="btn btn-primary h-8 text-[11.5px]" disabled={busy} onClick={() => void submit()}>
            {busy ? '保存中…' : '保存并搜索封面'}
          </button>
        </div>
      </div>
    </div>
  );
}

export function AddGameDialog() {
  const { addOpen, setAddOpen, addGame } = useApp();
  const [path, setPath] = useState('');
  const [busy, setBusy] = useState(false);

  if (!addOpen) return null;

  const browse = async () => {
    try {
      const picked = await call<string | null>('pick_folder');
      if (picked) setPath(picked);
    } catch {
      /* 浏览器预览模式没有原生选择器 */
    }
  };

  const submit = async () => {
    if (!path.trim()) return;
    setBusy(true);
    await addGame(path.trim());
    setBusy(false);
    setPath('');
  };

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-50 grid place-items-center">
      <div
        className="anim-overlay absolute inset-0 bg-black/50"
        onClick={() => setAddOpen(false)}
        aria-hidden
      />
      <div className="anim-panel relative w-[460px] rounded-[16px] border border-line bg-surface p-5 shadow-[var(--s-shadow)]">
        <h3 className="text-[14px] font-medium">添加游戏目录</h3>
        <p className="mt-1.5 text-[12px] leading-relaxed text-ink-2">
          选择游戏主程序（.exe）所在的文件夹。Steam / Epic / GOG 的游戏会自动扫描到，这里只需要补充扫描不到的游戏。
        </p>

        <div className="mt-4 flex items-center gap-1.5">
          <input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void submit();
            }}
            placeholder="D:\Games\SomeGame"
            className="field h-9 flex-1 text-[12px]"
            autoFocus
          />
          {isTauri && (
            <button className="btn btn-ghost h-9 shrink-0 text-[12px]" onClick={browse}>
              <FolderOpen size={13} />
              浏览
            </button>
          )}
        </div>

        <div className="mt-4 flex items-center justify-end gap-1.5">
          <button className="btn btn-ghost h-8" onClick={() => setAddOpen(false)}>
            取消
          </button>
          <button
            className="btn btn-primary h-8"
            disabled={busy || !path.trim()}
            onClick={() => void submit()}
          >
            {busy ? '添加中…' : '添加'}
          </button>
        </div>
      </div>
    </div>
  );
}

export function CloseDialog() {
  const { closePrompt, setClosePrompt, toast, updateSettings } = useApp();
  const [remember, setRemember] = useState(false);

  useEffect(() => {
    if (closePrompt) setRemember(false);
  }, [closePrompt]);

  useEffect(() => {
    if (!closePrompt) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setClosePrompt(false);
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [closePrompt, setClosePrompt]);

  if (!closePrompt) return null;

  const choose = async (action: 'tray' | 'exit') => {
    setClosePrompt(false);
    if (remember) updateSettings({ closeAction: action });
    if (!isTauri) {
      toast({ kind: 'info', title: action === 'tray' ? '已最小化到托盘（演示）' : '已退出（演示）' });
      return;
    }
    try {
      await call('close_window', { action, remember });
    } catch (e) {
      toast({ kind: 'danger', title: '操作失败', desc: errorText(e) });
    }
  };

  return (
    <div className="fixed inset-0 z-[85] grid place-items-center">
      <div
        className="anim-overlay absolute inset-0 bg-black/50"
        onClick={() => setClosePrompt(false)}
        aria-hidden
      />
      <div className="anim-panel relative w-[440px] rounded-[16px] border border-line bg-surface p-5 shadow-[var(--s-shadow)]">
        <h3 className="text-[14px] font-medium">关闭 ANR Studio？</h3>
        <p className="mt-1.5 text-[12px] leading-relaxed text-ink-2">
          最小化到托盘后程序仍在后台运行，下载和安装任务不会中断，可以从任务栏托盘图标重新打开。
        </p>

        <button
          className="mt-3.5 flex w-full items-center gap-2.5 rounded-[10px] border border-line bg-surface-2/40 px-3 py-2.5 text-left transition-colors hover:border-surface-3"
          onClick={() => setRemember(!remember)}
        >
          <span
            className={clsx(
              'grid h-[16px] w-[16px] shrink-0 place-items-center rounded-[4px] border transition-colors',
              remember
                ? 'border-transparent bg-accent text-white'
                : 'border-line text-transparent',
            )}
          >
            <Check size={11} strokeWidth={3} />
          </span>
          <span className="text-[11.5px] text-ink-2">记住我的选择，下次不再询问</span>
        </button>

        <div className="mt-4 flex items-center gap-1.5">
          <button className="btn btn-ghost h-8" onClick={() => setClosePrompt(false)}>
            取消
          </button>
          <div className="flex-1" />
          <button className="btn btn-ghost h-8 text-[11.5px]" onClick={() => void choose('exit')}>
            直接退出
          </button>
          <button className="btn btn-primary h-8 text-[11.5px]" onClick={() => void choose('tray')}>
            最小化到托盘
          </button>
        </div>
      </div>
    </div>
  );
}

export function ConfirmDialog() {
  const { confirm, resolveConfirm } = useApp();
  if (!confirm) return null;
  const danger = confirm.tone === 'danger';
  const warn = confirm.tone === 'warn';

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-[95] grid place-items-center">
      <div
        className="anim-overlay absolute inset-0 bg-black/50"
        onClick={() => resolveConfirm(false)}
        aria-hidden
      />
      <div className="anim-panel relative w-[430px] rounded-[16px] border border-line bg-surface p-5 shadow-[var(--s-shadow)]">
        <div className="flex items-start gap-2.5">
          {(danger || warn) && (
            <span
              className="mt-0.5 grid h-6 w-6 shrink-0 place-items-center rounded-full"
              style={{
                background: warn ? 'var(--s-warn-soft)' : 'var(--s-danger-soft)',
                color: warn ? 'var(--s-warn)' : 'var(--s-danger)',
              }}
            >
              <AlertTriangle size={13} />
            </span>
          )}
          <div className="min-w-0">
            <h3 className="text-[14px] font-medium leading-snug">{confirm.title}</h3>
            <p className="mt-1.5 text-[12px] leading-relaxed text-ink-2">{confirm.body}</p>
          </div>
        </div>

        {confirm.items && confirm.items.length > 0 && (
          <ul className="mt-3.5 space-y-1.5 rounded-[10px] border border-line bg-surface-2/50 px-3 py-2.5">
            {confirm.items.map((it, i) => (
              <li key={i} className="flex items-start gap-2 text-[11.5px] text-ink-2">
                <span className="mono mt-[3px] text-[9px] text-ink-3">{String(i + 1).padStart(2, '0')}</span>
                <span className="min-w-0 break-all">{it}</span>
              </li>
            ))}
          </ul>
        )}

        <div className="mt-4 flex items-center justify-end gap-1.5">
          <button className="btn btn-ghost h-8" onClick={() => resolveConfirm(false)}>
            取消
          </button>
          <button
            className={clsx(
              'btn h-8',
              danger ? 'bg-danger text-white hover:opacity-90' : warn ? 'bg-warn text-white hover:opacity-90' : 'btn-primary',
            )}
            onClick={() => resolveConfirm(true)}
          >
            {confirm.confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

/** R5 / R6：GitHub 拉不动时导入官方 zip，校验规则与在线下载完全一致 */
export function ImportPackageDialog() {
  const { importOpen, setImportOpen, versions, pickZip, officialSum, importLocalPackage, toast } =
    useApp();
  const [path, setPath] = useState('');
  const [tag, setTag] = useState('');
  const [kind, setKind] = useState('full');
  const [sha, setSha] = useState('');
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!importOpen) return;
    const v = useApp.getState().versions;
    setTag(v.find((x) => x.latest)?.tag ?? v[0]?.tag ?? '');
    setPath('');
    setSha('');
    setKind('full');
  }, [importOpen]);

  if (!importOpen) return null;

  const browse = async () => {
    const p = await pickZip();
    if (p) {
      setPath(p);
      setSha('');
    }
  };

  const autoSum = async () => {
    const filename = path.split(/[\\/]/).pop() ?? '';
    if (!path || !tag || !filename) return;
    try {
      setSha(await officialSum(tag, filename));
      toast({ kind: 'ok', title: '已从官方 SHA256SUMS.txt 填入校验值' });
    } catch (e) {
      toast({
        kind: 'warn',
        title: '没能自动获取校验值',
        desc: `${errorText(e)}；请手动粘贴`,
      });
    }
  };

  const submit = async () => {
    if (!path.trim() || !tag) return;
    setBusy(true);
    await importLocalPackage(path.trim(), tag, kind, sha.trim());
    setBusy(false);
  };

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-[80] grid place-items-center">
      <div
        className="anim-overlay absolute inset-0 bg-black/50"
        onClick={() => setImportOpen(false)}
        aria-hidden
      />
      <div className="anim-panel relative w-[520px] rounded-[16px] border border-line bg-surface p-5 shadow-[var(--s-shadow)]">
        <h3 className="text-[14px] font-medium">导入本地安装包</h3>
        <p className="mt-1.5 text-[12px] leading-relaxed text-ink-2">
          GitHub 下载不动时，可以先下好官方 zip 再在这里导入。导入包与在线下载走同一套 SHA256
          校验，校验不过直接拒绝，不提供「跳过」选项。
        </p>

        <div className="mt-4 space-y-3">
          <div>
            <label className="mb-1.5 block text-[10.5px] text-ink-3">安装包（.zip）</label>
            <div className="flex items-center gap-1.5">
              <input
                value={path}
                onChange={(e) => {
                  setPath(e.target.value);
                  setSha('');
                }}
                placeholder="D:\Downloads\anr-fg-V.B18-full.zip"
                className="field h-9 min-w-0 flex-1 text-[11.5px]"
              />
              <button
                className="btn btn-ghost h-9 shrink-0 text-[11.5px]"
                onClick={() => void browse()}
              >
                <Upload size={13} />
                浏览
              </button>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="mb-1.5 block text-[10.5px] text-ink-3">对应版本</label>
              <select
                value={tag}
                onChange={(e) => setTag(e.target.value)}
                className="field h-9 w-full appearance-none text-[11.5px]"
              >
                {versions.map((v) => (
                  <option key={v.tag} value={v.tag}>
                    {v.tag}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="mb-1.5 block text-[10.5px] text-ink-3">包型</label>
              <select
                value={kind}
                onChange={(e) => setKind(e.target.value)}
                className="field h-9 w-full appearance-none text-[11.5px]"
              >
                <option value="full">full（完整包）</option>
                <option value="addon">addon（已有 OptiScaler）</option>
              </select>
            </div>
          </div>

          <div>
            <label className="mb-1.5 block text-[10.5px] text-ink-3">
              SHA256（必填，可从官方 SHA256SUMS.txt 复制）
            </label>
            <div className="flex items-center gap-1.5">
              <input
                value={sha}
                onChange={(e) => setSha(e.target.value.trim())}
                placeholder="64 位十六进制"
                className="field mono h-9 min-w-0 flex-1 text-[11px]"
              />
              <button
                className="btn btn-ghost h-9 shrink-0 text-[11.5px]"
                disabled={!path || !tag}
                onClick={() => void autoSum()}
              >
                <RefreshCw size={12} />
                官方获取
              </button>
            </div>
            <p className="mt-1 text-[10px] text-ink-3">
              留空时点「官方获取」会自动查；拉不到就必须手动填，否则拒绝导入。
            </p>
          </div>
        </div>

        <div className="mt-5 flex items-center gap-1.5">
          <div className="flex-1" />
          <button className="btn btn-ghost h-8" onClick={() => setImportOpen(false)}>
            取消
          </button>
          <button
            className="btn btn-primary h-8 text-[11.5px]"
            disabled={busy || !path.trim() || !tag}
            onClick={() => void submit()}
          >
            {busy ? '校验并导入…' : '校验并导入'}
          </button>
        </div>
      </div>
    </div>
  );
}

/** R7：版本显示「未知」时，把某个游戏的 provider 指纹手工指向一个版本 */
export function MarkVersionDialog() {
  const { markVersionFor, setMarkVersionFor, games, versions, markVersion } = useApp();
  const [id, setId] = useState('');
  const [tag, setTag] = useState('');
  const installed = games.filter((g) => g.anr);

  useEffect(() => {
    if (!markVersionFor) return;
    const v = useApp.getState().versions;
    setId(markVersionFor.gameId || games.find((g) => g.anr)?.id || '');
    setTag(v.find((x) => x.latest)?.tag ?? v[0]?.tag ?? '');
  }, [markVersionFor]);

  if (!markVersionFor) return null;

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-[80] grid place-items-center">
      <div
        className="anim-overlay absolute inset-0 bg-black/50"
        onClick={() => setMarkVersionFor(null)}
        aria-hidden
      />
      <div className="anim-panel relative w-[480px] rounded-[16px] border border-line bg-surface p-5 shadow-[var(--s-shadow)]">
        <h3 className="text-[14px] font-medium">手动标记版本</h3>
        <p className="mt-1.5 text-[12px] leading-relaxed text-ink-2">
          ANR 的 DLL 没有版本资源，工具靠 provider DLL 的 SHA256 指纹反查版本。指纹表里没有记录时会显示「未知」，在这里指定一次即可：所有装了同一份 DLL 的游戏都会一起识别出来。
        </p>

        {installed.length === 0 ? (
          <p className="mt-4 rounded-[10px] border border-line bg-surface-2/50 px-3 py-3 text-[11.5px] text-ink-3">
            还没有已安装 ANR 的游戏，先装一个再来标记。
          </p>
        ) : (
          <div className="mt-4 space-y-3">
            <div>
              <label className="mb-1.5 block text-[10.5px] text-ink-3">游戏</label>
              <select
                value={id}
                onChange={(e) => setId(e.target.value)}
                className="field h-9 w-full appearance-none text-[11.5px]"
              >
                {installed.map((g) => (
                  <option key={g.id} value={g.id}>
                    {g.name}（当前：{g.anr?.version}）
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="mb-1.5 block text-[10.5px] text-ink-3">标记为</label>
              <select
                value={tag}
                onChange={(e) => setTag(e.target.value)}
                className="field h-9 w-full appearance-none text-[11.5px]"
              >
                {versions.map((v) => (
                  <option key={v.tag} value={v.tag}>
                    {v.tag}
                  </option>
                ))}
              </select>
            </div>
          </div>
        )}

        <div className="mt-5 flex items-center gap-1.5">
          <div className="flex-1" />
          <button className="btn btn-ghost h-8" onClick={() => setMarkVersionFor(null)}>
            取消
          </button>
          <button
            className="btn btn-primary h-8 text-[11.5px]"
            disabled={!id || !tag}
            onClick={() => void markVersion(id, tag)}
          >
            写入指纹表
          </button>
        </div>
      </div>
    </div>
  );
}

/**
 * P0-2：安装/切换失败后的可操作出口。
 * 重试、生成诊断包、回滚到操作前快照，并把安装器原始输出放在可滚动区域里。
 */
export function FailureDialog() {
  const { failure, setFailure, makeDiag, restoreSnapshot, toast } = useApp();

  useEffect(() => {
    if (!failure) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setFailure(null);
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [failure, setFailure]);

  if (!failure) return null;

  const title = failure.kind === 'switch' ? '切换版本失败' : '安装失败';
  const retry = () => {
    const fn = failure.retry;
    setFailure(null);
    void fn();
  };
  const rollback = () => {
    if (!failure.snapshotPath) return;
    void restoreSnapshot(failure.gameId, failure.snapshotPath);
  };
  const diag = () => {
    void makeDiag(failure.gameId);
  };

  return (
    <div className="fixed inset-x-0 bottom-0 top-[var(--titlebar-h)] z-[80] grid place-items-center">
      <div className="anim-overlay absolute inset-0 bg-black/55" onClick={() => setFailure(null)} aria-hidden />
      <div className="anim-panel relative flex max-h-[min(720px,90vh)] w-[min(620px,calc(100vw-80px))] flex-col overflow-hidden rounded-[16px] border border-line bg-surface shadow-[var(--s-shadow)]">
        <div className="flex items-start gap-3 border-b border-line px-5 py-4">
          <span className="mt-0.5 grid h-8 w-8 shrink-0 place-items-center rounded-full bg-danger-soft text-danger">
            <AlertTriangle size={15} />
          </span>
          <div className="min-w-0 flex-1">
            <h3 className="text-[14px] font-medium">{title}</h3>
            <p className="mt-0.5 truncate text-[11px] text-ink-3">{failure.gameName}</p>
          </div>
          <button className="btn btn-icon -mr-1 -mt-1 h-7 w-7" onClick={() => setFailure(null)} aria-label="关闭">
            <X size={14} />
          </button>
        </div>

        <div className="scroll-thin min-h-0 flex-1 overflow-y-auto px-5 py-4">
          {failure.rolledBack && (
            <div className="mb-3 flex items-start gap-2 rounded-[10px] bg-ok-soft px-3 py-2.5">
              <Check size={13} className="mt-[1px] shrink-0 text-ok" />
              <p className="text-[11.5px] leading-relaxed text-ink-2">
                已自动回滚到操作前的状态。可以修正原因后重试，或先生成诊断包。
              </p>
            </div>
          )}

          <p className="mb-3 whitespace-pre-wrap rounded-[10px] border border-danger/25 bg-danger-soft/40 px-3 py-2.5 text-[11.5px] leading-relaxed text-ink-2">
            {failure.message}
          </p>

          <div className="flex items-center gap-2">
            <Bug size={12} className="text-ink-3" />
            <span className="text-[10.5px] font-medium tracking-[0.04em] text-ink-3">原始输出</span>
          </div>
          {failure.output ? (
            <pre className="scroll-thin mono mt-1.5 max-h-[220px] overflow-auto whitespace-pre-wrap break-all rounded-[9px] border border-line bg-surface-2/50 px-3 py-2.5 text-[10.5px] leading-relaxed text-ink-2">
              {failure.output}
            </pre>
          ) : (
            <p className="mt-1.5 rounded-[9px] border border-line bg-surface-2/40 px-3 py-2.5 text-[11px] text-ink-3">
              安装器没有产生额外输出。
            </p>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-1.5 border-t border-line px-5 py-3">
          <button
            className="btn btn-ghost h-8 text-[11.5px]"
            title={failure.canRollback ? '用操作前的快照覆盖当前文件' : '没有可用的安装前快照'}
            disabled={!failure.canRollback}
            onClick={rollback}
          >
            <RotateCcw size={12} />
            回滚到快照
          </button>
          <button className="btn btn-ghost h-8 text-[11.5px]" onClick={diag}>
            <FileArchive size={12} />
            生成诊断包
          </button>
          <div className="flex-1" />
          <button className="btn btn-ghost h-8" onClick={() => setFailure(null)}>
            关闭
          </button>
          <button className="btn btn-primary h-8 text-[11.5px]" onClick={retry}>
            <RefreshCw size={12} />
            重试
          </button>
        </div>
      </div>
    </div>
  );
}
