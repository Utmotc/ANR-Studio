import { useEffect, useState } from 'react';
import { Copy, Minus, Square, X } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauri } from '../lib/ipc';
import { AboutDialog, AppMenu } from './AppMenu';

const APP_VERSION = typeof __APP_VERSION__ === 'string' ? __APP_VERSION__ : '0.1.0';

export function TitleBar() {
  const [maxed, setMaxed] = useState(false);
  const [about, setAbout] = useState(false);

  useEffect(() => {
    if (!isTauri) return;
    const w = getCurrentWindow();
    let alive = true;
    let un: (() => void) | undefined;

    w.isMaximized()
      .then((v) => alive && setMaxed(v))
      .catch(() => undefined);
    w.onResized(() => {
      w.isMaximized()
        .then((v) => alive && setMaxed(v))
        .catch(() => undefined);
    })
      .then((f) => {
        if (alive) un = f;
        else f();
      })
      .catch(() => undefined);

    return () => {
      alive = false;
      un?.();
    };
  }, []);

  const call = (fn: (w: ReturnType<typeof getCurrentWindow>) => Promise<void>) => {
    if (!isTauri) return;
    void fn(getCurrentWindow()).catch(() => undefined);
  };

  return (
    <>
      <div className="titlebar">
        <div
          className="titlebar-drag"
          data-tauri-drag-region
          onDoubleClick={() => call((w) => w.toggleMaximize())}
        />

        <AppMenu onAbout={() => setAbout(true)} />

        <div className="titlebar-title">
          <span>ANR Studio</span>
          <span className="titlebar-ver mono text-[10.5px] text-ink-3">v{APP_VERSION}</span>
        </div>

        <div className="titlebar-controls">
          <button
            className="titlebar-btn"
            onClick={() => call((w) => w.minimize())}
            aria-label="最小化"
            title="最小化"
          >
            <Minus size={14} strokeWidth={1.6} />
          </button>
          <button
            className="titlebar-btn"
            onClick={() => call((w) => w.toggleMaximize())}
            aria-label={maxed ? '还原' : '最大化'}
            title={maxed ? '还原' : '最大化'}
          >
            {maxed ? (
              <Copy size={11} strokeWidth={1.6} className="-scale-x-100" />
            ) : (
              <Square size={11} strokeWidth={1.6} />
            )}
          </button>
          <button
            className="titlebar-btn close"
            onClick={() => call((w) => w.close())}
            aria-label="关闭"
            title="关闭"
          >
            <X size={15} strokeWidth={1.6} />
          </button>
        </div>
      </div>

      <AboutDialog open={about} onClose={() => setAbout(false)} />
    </>
  );
}
