import { AlertTriangle, FileArchive, FolderOpen, X } from 'lucide-react';
import { useApp } from '../store';
import { call, isTauri } from '../lib/ipc';
import { fullDate } from '../lib';

function kindLabel(kind: string) {
  if (kind === 'frontend') return '界面异常';
  if (kind === 'panic') return '程序崩溃';
  return kind || '异常';
}

/**
 * v0.4 P0-4：上次运行崩溃了就在启动后提示一次。
 * 「生成诊断包」= 把 crash/*.txt + 系统信息打包并复制路径；「忽略」= 写 .ack。
 */
export function CrashNotice() {
  const { crashPending, makeCrashDiag, ackCrash, toast } = useApp();
  if (!crashPending.length) return null;

  const first = crashPending[0];
  const openDir = async () => {
    const dir = first.path.replace(/[\\/][^\\/]+$/, '');
    try {
      await call('open_path', { path: dir });
    } catch {
      toast({ kind: 'info', title: '崩溃报告目录', desc: dir });
    }
  };

  return (
    <div className="pointer-events-none fixed inset-x-0 top-[44px] z-[75] flex justify-center px-4">
      <div className="pointer-events-auto flex w-[680px] max-w-full items-start gap-3 rounded-[12px] border border-line bg-surface px-4 py-3 shadow-[var(--s-shadow)]">
        <span className="mt-[1px] grid h-7 w-7 shrink-0 place-items-center rounded-[8px] bg-warn-soft text-warn">
          <AlertTriangle size={15} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <p className="text-[12.5px] font-medium">上次运行出现了异常</p>
            <span className="chip h-[17px] text-[10px]">{kindLabel(first.kind)}</span>
            {crashPending.length > 1 && (
              <span className="text-[10.5px] text-ink-3">共 {crashPending.length} 条</span>
            )}
          </div>
          <p className="mono mt-1 truncate text-[11px] text-ink-3">
            {first.createdAt ? fullDate(first.createdAt) : ''} · {first.message || '没有错误信息'}
          </p>
          <p className="mono mt-0.5 truncate text-[10.5px] text-ink-3">{first.location}</p>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <button
            className="btn btn-ghost h-7 text-[11px]"
            onClick={() => void openDir()}
            disabled={!isTauri}
          >
            <FolderOpen size={12} />
            目录
          </button>
          <button className="btn btn-primary h-7 text-[11px]" onClick={() => void makeCrashDiag()}>
            <FileArchive size={12} />
            生成诊断包
          </button>
          <button
            className="btn btn-icon h-7 w-7"
            onClick={() => void ackCrash()}
            aria-label="忽略这次崩溃报告"
            title="忽略这次崩溃报告"
          >
            <X size={13} />
          </button>
        </div>
      </div>
    </div>
  );
}