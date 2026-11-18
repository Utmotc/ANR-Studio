export type View = 'overview' | 'library' | 'versions' | 'logs' | 'tuning';
export type Theme = 'dark' | 'light' | 'system';
export type Source = 'steam' | 'epic' | 'gog' | 'manual';
export type InstallKind = 'full' | 'addon';
export type Health = 'ok' | 'warn' | 'error';
export type Filter = 'all' | 'installed' | 'missing' | 'outdated' | 'attention';

export interface NrFgConfig {
  path: string;
  modelSize: string;
  nits: number;
  maxGenerated: number;
  sceneCut: boolean;
}

export interface AnrState {
  kind: InstallKind;
  version: string;
  proxy: 'dxgi.dll' | 'winmm.dll' | 'version.dll';
  installedAt: string;
  enabled: boolean;
  /** OptiScaler.ini 的 [FrameGen] FGNvngxReplacement */
  fgReplacement?: string;
  fingerprint: string;
  config: NrFgConfig;
  health: Health;
  healthNote?: string | null;
  logPath: string;
}

export interface Game {
  id: string;
  name: string;
  source: Source;
  dir: string;
  exe: string;
  lastUsed: string;
  running?: boolean;
  sizeGB: number;
  thirdPartyOptiScaler?: boolean;
  anr: AnrState | null;
  cover?: string | null;
  /** 5.1 异常态：需要人工确认后才能安装/卸载 */
  conflict?: string | null;
}

export interface Version {
  tag: string;
  product: string;
  publishedAt: string;
  sizeMB: number;
  cached: boolean;
  latest?: boolean;
  fingerprint: string;
  notes: string;
}

export interface Task {
  id: string;
  title: string;
  subtitle: string;
  stage: string;
  progress: number;
  status: 'running' | 'done' | 'error' | 'canceled';
  gameId?: string;
  cancellable?: boolean;
}

export interface Toast {
  id: string;
  kind: 'ok' | 'warn' | 'danger' | 'info';
  title: string;
  desc?: string;
}

export interface ConfirmRequest {
  title: string;
  body: string;
  items?: string[];
  confirmLabel: string;
  tone: 'danger' | 'normal' | 'warn';
  action: () => void | Promise<void>;
  /** 用户点取消或关掉弹窗时调用 */
  onCancel?: () => void;
}

export interface Settings {
  theme: Theme;
  density: 'cozy' | 'compact';
  fontSize: number;
  startupView: 'overview' | 'library' | 'versions' | 'logs';
  reduceMotion: boolean;
  closeAction: 'ask' | 'tray' | 'exit';
  coverSource: 'auto' | 'domestic' | 'global';
  onboarded?: boolean;
  mirror: 'auto' | 'github' | 'mirror';
  cacheDir: string;
  warnAntiCheat: boolean;
  keepSnapshot: boolean;
  /** 同时进行的安装/下载任务数，1 = 串行 */
  concurrency: number;
  /** 默认安装版本，空字符串 = 跟随最新 */
  defaultVersion: string;
  /** 界面语言 */
  locale: 'zh' | 'en';
  /** 游戏库视图，启动时恢复 */
  layout: 'grid' | 'list';
  manualGames?: string[];
  renames?: Record<string, string>;
  hiddenGames?: string[];
}

export interface ChangelogItem {
  /** 英文原文 */
  en: string;
  /** 中文译文，翻译不可用时为空 */
  zh: string;
}

export interface Changelog {
  tag: string;
  title: string;
  items: ChangelogItem[];
  url: string;
  source: 'github' | 'none';
}

export interface PrecheckIssue {
  code: string;
  level: 'block' | 'warn';
  title: string;
  detail: string;
}

/** 卸载/切换前生成的快照。schema 0 = 旧版只含 OptiScaler 目录。 */
export interface SnapshotInfo {
  path: string;
  fileName: string;
  createdAt: string;
  sizeBytes: number;
  gameId: string;
  gameName: string;
  schema: number;
}

/** P0-2：安装 / 切换失败后的可操作结果 */
export interface FailureInfo {
  gameId: string;
  gameName: string;
  kind: 'install' | 'switch';
  message: string;
  output: string;
  snapshotPath?: string;
  rolledBack: boolean;
  canRollback: boolean;
  retry: () => Promise<void>;
}
