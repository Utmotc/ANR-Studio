import type { Game, Health } from './types';

export const clsx = (...parts: (string | false | null | undefined)[]) =>
  parts.filter(Boolean).join(' ');

export function hashInt(s: string): number {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return Math.abs(h);
}

const COVERS: [string, string][] = [
  ['#3a4d78', '#25314f'],
  ['#5c4560', '#3a2c40'],
  ['#2f5a4c', '#1f3a32'],
  ['#6b5030', '#453420'],
  ['#5e3a3a', '#3c2626'],
  ['#2f5060', '#1f3642'],
  ['#4a4a6b', '#31314a'],
  ['#3f5a3a', '#2a3c27'],
];

export const coverTints = (name: string): [string, string] =>
  COVERS[hashInt(name) % COVERS.length];

export const initial = (name: string) => {
  const clean = name.replace(/^The\s+/i, '');
  return clean.slice(0, 1).toUpperCase();
};

export const SOURCE_LABEL: Record<Game['source'], string> = {
  steam: 'Steam',
  epic: 'Epic',
  gog: 'GOG',
  manual: '手动',
};

export function relTime(iso: string): string {
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return '—';
  const diff = Date.now() - t;
  const min = Math.round(diff / 60000);
  if (min < 1) return '刚刚';
  if (min < 60) return `${min} 分钟前`;
  const h = Math.round(min / 60);
  if (h < 24) return `${h} 小时前`;
  const d = Math.round(h / 24);
  if (d < 30) return `${d} 天前`;
  return new Date(iso).toLocaleDateString('zh-CN');
}

export function fullDate(iso: string): string {
  const d = new Date(iso);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(
    d.getDate(),
  ).padStart(2, '0')} ${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(
    2,
    '0',
  )}`;
}

export type Tone = 'ok' | 'warn' | 'danger' | 'muted' | 'accent';

/** 版本表还没拉到时，不能把「—」当成一个真实版本去比较 */
export function versionKnown(latest: string) {
  return !!latest && latest !== '—' && latest !== '未知';
}

export function statusOf(g: Game, latest: string): { label: string; tone: Tone } {
  if (g.running) return { label: '运行中', tone: 'accent' };
  // 5.1 异常态优先于一切：操作前必须先人工确认
  if (g.conflict) return { label: '需确认', tone: 'danger' };
  if (!g.anr) {
    if (g.thirdPartyOptiScaler) return { label: '需插件安装', tone: 'warn' };
    return { label: '未安装', tone: 'muted' };
  }
  if (g.anr.health !== 'ok') return { label: '需注意', tone: 'warn' };
  if (versionKnown(latest) && g.anr.version !== latest) return { label: '可更新', tone: 'warn' };
  if (g.anr.enabled) return { label: '已启用', tone: 'ok' };
  return { label: '已关闭', tone: 'muted' };
}

export const TONE_COLOR: Record<Tone, string> = {
  ok: 'var(--s-ok)',
  warn: 'var(--s-warn)',
  danger: 'var(--s-danger)',
  muted: 'var(--s-ink-3)',
  accent: 'var(--s-accent)',
};

export const HEALTH_TONE: Record<Health, Tone> = { ok: 'ok', warn: 'warn', error: 'danger' };

export function isOutdated(g: Game, latest: string) {
  return versionKnown(latest) && !!g.anr && g.anr.version !== latest;
}

/** 需要用户处理的问题：异常态、配置异常或第三方 OptiScaler 冲突。版本落后单独归入「可更新」。 */
export function needsAttention(g: Game) {
  if (g.conflict) return true;
  if (g.anr && g.anr.health !== 'ok') return true;
  if (!g.anr && g.thirdPartyOptiScaler) return true;
  return false;
}
