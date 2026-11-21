import { Component, type ErrorInfo, type ReactNode } from 'react';
import { AlertTriangle, Copy, RotateCcw } from 'lucide-react';
import { reportFrontendError } from '../lib/ipc';

interface Props {
  children: ReactNode;
}

interface State {
  error: Error | null;
  detail: string;
}

/**
 * v0.4 P0-4：React 渲染炸掉时不再白屏，同时把异常落进 `crash/`。
 * 后端的 panic hook 管不到前端异常，这里补上。
 */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null, detail: '' };

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    const detail = `${error.stack ?? ''}\n--- componentStack ---${info.componentStack ?? ''}`;
    this.setState({ detail });
    reportFrontendError(error.message || String(error), detail);
  }

  private copy = () => {
    const { error, detail } = this.state;
    const text = `${error?.message ?? '未知错误'}\n\n${detail}`;
    void navigator.clipboard?.writeText(text);
  };

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="grid h-full w-full place-items-center bg-bg p-6">
        <div className="w-[560px] max-w-full rounded-[16px] border border-line bg-surface p-6 shadow-[var(--s-shadow)]">
          <div className="flex items-start gap-3">
            <span className="grid h-9 w-9 shrink-0 place-items-center rounded-[10px] bg-danger-soft text-danger">
              <AlertTriangle size={17} />
            </span>
            <div className="min-w-0 flex-1">
              <p className="text-[14px] font-medium">界面渲染出错了</p>
              <p className="mt-1 text-[11.5px] leading-relaxed text-ink-3">
                错误已写入崩溃目录，重新启动后可以在提示里生成诊断包。你的游戏文件没有被改动。
              </p>
            </div>
          </div>
          <pre className="scroll-thin mono mt-4 max-h-[180px] overflow-auto rounded-[10px] border border-line-soft bg-surface-2/50 p-3 text-[11px] leading-relaxed text-ink-2">
            {error.message || String(error)}
          </pre>
          <div className="mt-4 flex items-center justify-end gap-2">
            <button className="btn btn-ghost h-8 text-[11.5px]" onClick={this.copy}>
              <Copy size={12} />
              复制错误信息
            </button>
            <button
              className="btn btn-primary h-8 text-[11.5px]"
              onClick={() => window.location.reload()}
            >
              <RotateCcw size={12} />
              重新加载界面
            </button>
          </div>
        </div>
      </div>
    );
  }
}