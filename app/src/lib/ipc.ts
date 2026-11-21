import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export const isTauri =
  typeof window !== 'undefined' &&
  ('__TAURI_INTERNALS__' in window || '__TAURI__' in (window as unknown as object));

/** 在浏览器里直接打开时抛 NOT_TAURI，调用方据此回退到演示数据 */
export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri) throw new Error('NOT_TAURI');
  return invoke<T>(cmd, args);
}

export interface TaskPayload {
  id: string;
  title: string;
  subtitle: string;
  stage: string;
  progress: number;
  status: 'running' | 'done' | 'error' | 'canceled';
}

export function onTask(cb: (p: TaskPayload) => void): Promise<UnlistenFn> {
  if (!isTauri) return Promise.resolve(() => {});
  return listen<TaskPayload>('task', (e) => cb(e.payload));
}

export function onCover(cb: (p: { id: string; cover: string }) => void): Promise<UnlistenFn> {
  if (!isTauri) return Promise.resolve(() => {});
  return listen<{ id: string; cover: string }>('cover', (e) => cb(e.payload));
}

export function errorText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  return String(e);
}

export function onCloseRequest(cb: () => void): Promise<UnlistenFn> {
  if (!isTauri) return Promise.resolve(() => {});
  return listen('close-requested', () => cb());
}

/** v0.4 P0-4：前端异常也落到 crash/，让「闪退没有信息」不再只指后端 panic。 */
export function reportFrontendError(message: string, detail: string) {
  if (!isTauri) return;
  void call('report_frontend_error', { message, detail }).catch(() => undefined);
}
