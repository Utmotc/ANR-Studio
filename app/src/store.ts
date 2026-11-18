import { create } from 'zustand';
import { GAMES, VERSIONS } from './data';
import { call, errorText, isTauri, type TaskPayload } from './lib/ipc';
import type {
  Changelog,
  ConfirmRequest,
  FailureInfo,
  Filter,
  Game,
  InstallKind,
  PrecheckIssue,
  Settings,
  Task,
  Theme,
  Toast,
  View,
} from './types';

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const canceled = new Set<string>();
let seq = 0;
const uid = (p: string) => `${p}${Date.now().toString(36)}${(seq++).toString(36)}`;

type Steps = [string, number][];

const INSTALL_STEPS: Steps = [
  ['前置检查', 12],
  ['下载安装包', 44],
  ['校验 SHA256', 56],
  ['解压到缓存', 68],
  ['执行安装脚本', 92],
  ['后置校验', 100],
];
const REMOVE_STEPS: Steps = [
  ['生成快照', 34],
  ['执行卸载脚本', 74],
  ['校验文件还原', 100],
];
const SWITCH_STEPS: Steps = [
  ['准备目标版本', 26],
  ['覆盖安装', 78],
  ['指纹校验', 100],
];

const DEFAULT_SETTINGS: Settings = {
  theme: 'dark',
  density: 'cozy',
  fontSize: 2,
  startupView: 'overview',
  reduceMotion: false,
  closeAction: 'ask',
  coverSource: 'auto',
  onboarded: false,
  mirror: 'auto',
  cacheDir: '',
  warnAntiCheat: true,
  keepSnapshot: true,
  concurrency: 1,
  defaultVersion: '',
  locale: 'zh',
  layout: 'grid',
};

export const FONT_SCALES = [0.9, 0.95, 1, 1.08, 1.15];
export const FONT_LABELS = ['最小', '偏小', '中等', '偏大', '最大'];

const SNAPSHOT_MARKER = '[[ANR_SNAPSHOT]]';
const ROLLED_BACK_MARKER = '[[ANR_ROLLED_BACK]]';
const OUTPUT_HEADER = '--- 安装器输出 ---';

/** 后端把原始输出和快照标记附在错误字符串里；这里拆成 UI 可用的结构。 */
function parseFailure(text: string): Pick<
  FailureInfo,
  'message' | 'output' | 'snapshotPath' | 'rolledBack' | 'canRollback'
> {
  let snapshotPath: string | undefined;
  let rolledBack = false;
  const kept: string[] = [];
  for (const line of text.split('\n')) {
    if (line.startsWith(SNAPSHOT_MARKER)) {
      snapshotPath = line.slice(SNAPSHOT_MARKER.length).trim() || undefined;
      continue;
    }
    if (line.startsWith(ROLLED_BACK_MARKER)) {
      rolledBack = true;
      continue;
    }
    kept.push(line);
  }
  let body = kept.join('\n').trim();
  let output = '';
  const at = body.indexOf(OUTPUT_HEADER);
  if (at >= 0) {
    output = body.slice(at + OUTPUT_HEADER.length).trim();
    body = body.slice(0, at).trim();
  }
  return {
    message: body || '未知错误',
    output,
    snapshotPath,
    rolledBack,
    canRollback: !!snapshotPath && !rolledBack,
  };
}

export function applyUiScale(fontSize: number) {
  const i = Math.max(0, Math.min(FONT_SCALES.length - 1, Math.round(fontSize)));
  document.documentElement.style.zoom = String(FONT_SCALES[i]);
}

interface State {
  ready: boolean;
  connected: boolean;
  theme: Theme;
  view: View;
  games: Game[];
  versions: typeof VERSIONS;
  versionsLoading: boolean;
  versionsError: string;
  importOpen: boolean;
  markVersionFor: { gameId: string } | null;
  catalogBusy: boolean;
  tasks: Task[];
  toasts: Toast[];
  selected: string[];
  filter: Filter;
  query: string;
  layout: 'grid' | 'list';
  drawerId: string | null;
  paletteOpen: boolean;
  confirm: ConfirmRequest | null;
  logGameId: string;
  logText: string;
  logLoading: boolean;
  addOpen: boolean;
  settingsOpen: boolean;
  closePrompt: boolean;
  failure: FailureInfo | null;
  renameId: string | null;
  moveDir: { id: string; path: string } | null;
  settings: Settings;
  changelogs: Record<string, Changelog>;
  changelogPending: Record<string, boolean>;
  changelogError: Record<string, string>;

  init: () => Promise<void>;
  rescan: () => Promise<void>;
  loadVersions: (refresh?: boolean) => Promise<void>;
  loadLog: (id: string) => Promise<void>;
  makeDiag: (id: string) => Promise<void>;
  repair: (id: string) => Promise<void>;
  refreshCover: (id: string) => Promise<void>;
  pickCover: (id: string) => Promise<void>;
  setCover: (id: string, uri: string | null) => void;
  deleteVersion: (version: string) => Promise<void>;
  loadChangelog: (tag: string) => Promise<void>;
  retryChangelog: (tag: string) => Promise<void>;

  setImportOpen: (open: boolean) => void;
  setMarkVersionFor: (v: { gameId: string } | null) => void;
  setDefaultVersion: (tag: string | null) => Promise<void>;
  markVersion: (id: string, tag: string) => Promise<void>;
  refreshCatalog: () => Promise<void>;
  pickZip: () => Promise<string | null>;
  officialSum: (tag: string, filename: string) => Promise<string>;
  importLocalPackage: (path: string, tag: string, kind: string, sha256: string) => Promise<void>;
  checkPackageStructure: () => Promise<void>;
  installAll: (ids: string[]) => Promise<void>;

  setTheme: (t: Theme) => void;
  setView: (v: View) => void;
  setFilter: (f: Filter) => void;
  setQuery: (q: string) => void;
  setLayout: (l: 'grid' | 'list') => void;
  openDrawer: (id: string) => void;
  closeDrawer: () => void;
  setPalette: (open: boolean) => void;
  setLogGame: (id: string) => void;
  setAddOpen: (open: boolean) => void;
  setSettingsOpen: (open: boolean) => void;
  setClosePrompt: (open: boolean) => void;
  setFailure: (failure: FailureInfo | null) => void;
  restoreSnapshot: (id: string, path: string) => Promise<void>;
  setRenameId: (id: string | null) => void;
  setMoveDir: (v: { id: string; path: string } | null) => void;
  changeGameDir: (id: string, path: string, migrate: boolean) => Promise<void>;
  clearCover: (id: string) => Promise<void>;
  renameGame: (id: string, name: string | null) => Promise<void>;
  updateSettings: (patch: Partial<Settings>) => void;

  toggleSelect: (id: string) => void;
  selectMany: (ids: string[]) => void;
  clearSelect: () => void;

  toast: (t: Omit<Toast, 'id'>) => void;
  dismissToast: (id: string) => void;
  upsertTask: (p: TaskPayload) => void;

  askConfirm: (req: ConfirmRequest) => void;
  resolveConfirm: (ok: boolean) => void;

  install: (id: string) => Promise<void>;
  uninstall: (id: string, thenRemove?: boolean) => Promise<void>;
  switchVersion: (id: string, tag: string) => Promise<void>;
  toggleEnabled: (id: string) => Promise<void>;
  setFgReplacement: (id: string, mode: 'anr' | 'native' | 'auto') => Promise<void>;
  updateConfig: (id: string, patch: Partial<Game['anr']>) => void;
  cancelTask: (id: string) => void;
  addGame: (path?: string) => Promise<void>;
  removeGame: (id: string) => Promise<void>;
}


/** 浏览器预览用的假更新日志 */
const DEMO_CHANGELOG = (tag: string): Changelog => ({
  tag,
  title: `ANR FG Beta, ${tag}`,
  url: 'https://github.com/apex-compute/ANR/releases',
  source: 'github',
  items: [
    {
      en: 'New weights for the frame generation model.',
      zh: '帧生成模型换了新权重。',
    },
    {
      en: "In third-person driving the car's rear no longer tears or doubles, and the road under the car no longer flickers.",
      zh: '第三人称驾驶时车尾不再撕裂或重影，车下路面不再闪烁。',
    },
    {
      en: 'The provider (nvngx_dlssg.dll) is now signed by Apex Compute, Inc.',
      zh: 'provider（nvngx_dlssg.dll）现在由 Apex Compute, Inc. 签名。',
    },
  ],
});

export const useApp = create<State>()((set, get) => {
  const patchTask = (id: string, patch: Partial<Task>) =>
    set((s) => ({ tasks: s.tasks.map((t) => (t.id === id ? { ...t, ...patch } : t)) }));

  const patchGame = (id: string, fn: (g: Game) => Game) =>
    set((s) => ({ games: s.games.map((g) => (g.id === id ? fn(g) : g)) }));

  /**
   * 5.1 异常态统一门禁：任何会改动游戏目录的操作都要先人工确认。
   * 返回值同时作为后端的 force 参数（后端还会独立校验一次）。
   */
  const allowOperation = async (g: Game | undefined, action: string): Promise<boolean> => {
    if (!g?.conflict) return true;
    return new Promise<boolean>((resolve) => {
      get().askConfirm({
        title: `${g.name} 的安装状态需要先确认`,
        body: g.conflict ?? '',
        items: [
          `继续${action}会改动这个目录里现有的 OptiScaler 文件`,
          '不确定来源时建议先取消，并用「生成诊断包」留一份记录',
        ],
        confirmLabel: '确认继续',
        tone: 'warn',
        action: () => resolve(true),
        onCancel: () => resolve(false),
      });
    });
  };

  async function runTask(
    title: string,
    subtitle: string,
    steps: Steps,
    gameId: string,
    onDone: () => void,
  ) {
    const id = uid('task');
    set((s) => ({
      tasks: [
        ...s.tasks,
        { id, title, subtitle, stage: steps[0][0], progress: 3, status: 'running', gameId },
      ],
    }));
    let current = 3;
    for (const [stage, target] of steps) {
      patchTask(id, { stage });
      const from = current;
      for (let i = 1; i <= 4; i++) {
        await sleep(110 + Math.random() * 90);
        if (canceled.has(id)) {
          patchTask(id, { status: 'canceled', stage: '已取消' });
          canceled.delete(id);
          get().toast({ kind: 'warn', title: '任务已取消', desc: subtitle });
          return;
        }
        current = from + ((target - from) * i) / 4;
        patchTask(id, { progress: current });
      }
    }
    onDone();
    patchTask(id, { progress: 100, status: 'done', stage: '已完成' });
    await sleep(2600);
    set((s) => ({ tasks: s.tasks.filter((t) => t.id !== id) }));
  }

  return {
    ready: false,
    connected: false,
    theme: 'dark',
    view: 'overview',
    games: isTauri ? [] : GAMES,
    // 真实环境里版本表一律以后端为准：预置演示版本会在 GitHub 拉不到时
    // 冒充「最新版本」，让用户看到 V.B15 这种早已过期的假数据。
    versions: isTauri ? [] : VERSIONS,
    versionsLoading: isTauri,
    versionsError: '',
    importOpen: false,
    markVersionFor: null,
    catalogBusy: false,
    tasks: [],
    toasts: [],
    selected: [],
    filter: 'all',
    query: '',
    layout: 'grid',
    drawerId: null,
    paletteOpen: false,
    confirm: null,
    logGameId: '',
    logText: '',
    logLoading: false,
    addOpen: false,
    settingsOpen: false,
    closePrompt: false,
    failure: null,
    renameId: null,
    moveDir: null,
    settings: DEFAULT_SETTINGS,
    changelogs: {},
    changelogPending: {},
    changelogError: {},

    init: async () => {
      if (!isTauri) {
        const params = new URLSearchParams(window.location.search);
        const light = params.get('theme') === 'light';
        document.documentElement.dataset.theme = light ? 'light' : 'dark';
        const v = params.get('view');
        const views = ['overview', 'library', 'versions', 'logs'];
        set({
          ready: true,
          connected: false,
          logGameId: 'deathstranding',
          theme: light ? 'light' : 'dark',
          settings: { ...DEFAULT_SETTINGS, theme: light ? 'light' : 'dark' },
          settingsOpen: params.get('settings') === '1',
          view: (v && views.includes(v) ? v : 'overview') as View,
        });
        return;
      }
      try {
        const s = await call<Settings>('get_settings');
        set({
          settings: { ...DEFAULT_SETTINGS, ...s },
          theme: (s.theme as Theme) ?? 'dark',
          layout: s.layout === 'list' ? 'list' : 'grid',
        });
        document.documentElement.dataset.theme = s.theme === 'light' ? 'light' : 'dark';
        document.documentElement.dataset.density = s.density === 'compact' ? 'compact' : 'cozy';
        document.documentElement.dataset.motion = s.reduceMotion ? 'reduced' : 'full';
        applyUiScale(s.fontSize ?? 2);
        const boot = (s.startupView ?? 'overview') as View;
        if (['overview', 'library', 'versions', 'logs'].includes(boot)) set({ view: boot });
        const games = await call<Game[]>('scan_library');
        set({ games, connected: true, ready: true });
        const first = games.find((g) => g.anr)?.id;
        if (first) set({ logGameId: first });
        void get().loadVersions();
        // R8：启动时自检已缓存包的结构
        void get().checkPackageStructure();
      } catch (e) {
        set({ connected: false, ready: true, versionsLoading: false, versionsError: errorText(e) });
        get().toast({ kind: 'danger', title: '后端连接失败', desc: errorText(e) });
      }
    },

    rescan: async () => {
      if (!isTauri) return;
      try {
        const games = await call<Game[]>('scan_library');
        set((s) => ({
          games,
          logGameId:
            s.logGameId && games.some((g) => g.id === s.logGameId)
              ? s.logGameId
              : (games.find((g) => g.anr)?.id ?? ''),
        }));
      } catch (e) {
        get().toast({ kind: 'danger', title: '刷新失败', desc: errorText(e) });
      }
    },

    loadVersions: async (refresh) => {
      if (!isTauri) return;
      set({ versionsLoading: true });
      try {
        const v = await call<typeof VERSIONS>('list_releases', { refresh: refresh ?? false });
        // 即使后端返回空列表也要写回，不能让上一轮的旧数据继续冒充最新版本
        set({ versions: v, versionsLoading: false, versionsError: '' });
      } catch (e) {
        // 离线时保留上一次成功拉取的版本表，但页面会明确标注「可能不是最新」
        set({ versionsLoading: false, versionsError: errorText(e) });
      }
    },

    loadLog: async (id) => {
      if (!isTauri) {
        set({ logLoading: false, logText: '' });
        return;
      }
      set({ logLoading: true });
      try {
        const text = await call<string>('get_log', { id });
        set({ logText: text, logLoading: false });
      } catch (e) {
        set({ logText: '', logLoading: false });
        get().toast({ kind: 'warn', title: '读取日志失败', desc: errorText(e) });
      }
    },

    makeDiag: async (id) => {
      const g = get().games.find((x) => x.id === id);
      if (!isTauri) {
        get().toast({ kind: 'ok', title: '诊断包已生成', desc: `${g?.name}-diag.zip` });
        return;
      }
      try {
        const path = await call<string>('make_diag', { id });
        get().toast({ kind: 'ok', title: '诊断包已生成', desc: path });
      } catch (e) {
        get().toast({ kind: 'danger', title: '生成诊断包失败', desc: errorText(e) });
      }
    },

    repair: async (id) => {
      if (!isTauri) {
        patchGame(id, (game) =>
          game.anr ? { ...game, anr: { ...game.anr, health: 'ok', healthNote: null } } : game,
        );
        get().toast({ kind: 'ok', title: '已修复配置', desc: '演示模式' });
        return;
      }
      try {
        await call('repair_install', { id });
        await get().rescan();
        get().toast({ kind: 'ok', title: '已修复配置', desc: 'FGNvngxReplacement 已写入' });
      } catch (e) {
        get().toast({ kind: 'danger', title: '修复失败', desc: errorText(e) });
      }
    },

    pickCover: async (id) => {
      if (!isTauri) {
        get().toast({ kind: 'info', title: '浏览器预览模式不支持选本地图片' });
        return;
      }
      try {
        const file = await call<string | null>('pick_cover_image');
        if (!file) return;
        const uri = await call<string | null>('set_cover_from_file', { id, path: file });
        set((s) => ({ games: s.games.map((x) => (x.id === id ? { ...x, cover: uri } : x)) }));
        get().toast({ kind: 'ok', title: '封面已更新' });
      } catch (e) {
        get().toast({ kind: 'danger', title: '设置封面失败', desc: errorText(e) });
      }
    },

    refreshCover: async (id) => {
      if (!isTauri) {
        get().toast({ kind: 'info', title: '浏览器预览模式不支持抓封面' });
        return;
      }
      const g = get().games.find((x) => x.id === id);
      get().toast({ kind: 'info', title: '正在重新搜索封面', desc: g?.name });
      try {
        const uri = await call<string | null>('refresh_cover', { id });
        set((s) => ({
          games: s.games.map((x) => (x.id === id ? { ...x, cover: uri } : x)),
        }));
        if (uri) get().toast({ kind: 'ok', title: '封面已更新' });
      } catch (e) {
        get().toast({ kind: 'danger', title: '没有找到封面', desc: errorText(e) });
      }
    },

    setCover: (id, uri) =>
      set((s) => ({ games: s.games.map((x) => (x.id === id ? { ...x, cover: uri } : x)) })),

    renameGame: async (id, name) => {
      const clean = (name ?? '').trim();
      if (!isTauri) {
        set((s) => ({
          games: s.games.map((g) => (g.id === id ? { ...g, name: clean || g.name } : g)),
          renameId: null,
        }));
        return;
      }
      try {
        const games = await call<Game[]>('rename_game', { id, name: clean || null });
        set({ games, renameId: null });
        void get().loadVersions();
        get().toast({
          kind: 'ok',
          title: clean ? '已修改游戏名' : '已恢复原名',
          desc: clean ? '封面会按新名字重新搜索，没搜到可以右键重试' : undefined,
        });
      } catch (e) {
        get().toast({ kind: 'danger', title: '修改失败', desc: errorText(e) });
      }
    },

    loadChangelog: async (tag) => {
      const s = get();
      if (s.changelogs[tag] || s.changelogPending[tag]) return;
      set((x) => ({ changelogPending: { ...x.changelogPending, [tag]: true } }));

      const finish = (c: Changelog) => {
        set((x) => ({
          changelogs: { ...x.changelogs, [tag]: c },
          changelogPending: { ...x.changelogPending, [tag]: false },
          changelogError: { ...x.changelogError, [tag]: '' },
        }));
      };
      const fail = (msg: string) => {
        set((x) => ({
          changelogPending: { ...x.changelogPending, [tag]: false },
          changelogError: { ...x.changelogError, [tag]: msg },
        }));
      };

      if (!isTauri) {
        finish(DEMO_CHANGELOG(tag));
        return;
      }
      try {
        const c = await call<Changelog>('get_changelog', { tag });
        finish(c);
      } catch (e) {
        fail(errorText(e));
      }
    },

    retryChangelog: async (tag) => {
      set((x) => ({
        changelogs: { ...x.changelogs, [tag]: undefined as unknown as Changelog },
        changelogError: { ...x.changelogError, [tag]: '' },
      }));
      await get().loadChangelog(tag);
    },

    deleteVersion: async (version) => {
      if (!isTauri) return;
      try {
        const freed = await call<number>('delete_version', { version });
        await get().loadVersions(true);
        get().toast({
          kind: 'info',
          title: `已删除 ${version} 的缓存`,
          desc: `释放 ${(freed / 1048576).toFixed(1)} MB`,
        });
      } catch (e) {
        get().toast({ kind: 'danger', title: '删除失败', desc: errorText(e) });
      }
    },

    setImportOpen: (importOpen) => set({ importOpen }),
    setMarkVersionFor: (markVersionFor) => set({ markVersionFor }),

    setDefaultVersion: async (tag) => {
      const clean = tag?.trim() ?? '';
      const title = clean ? `默认版本已设为 ${clean}` : '已恢复跟随最新版本';
      if (!isTauri) {
        set((s) => ({ settings: { ...s.settings, defaultVersion: clean } }));
        get().toast({ kind: 'ok', title });
        return;
      }
      try {
        const s = await call<Settings>('set_default_version', { version: clean || null });
        set({ settings: { ...get().settings, ...s } });
        get().toast({ kind: 'ok', title });
      } catch (e) {
        get().toast({ kind: 'danger', title: '设置失败', desc: errorText(e) });
      }
    },

    markVersion: async (id, tag) => {
      if (!isTauri) {
        set((s) => ({
          games: s.games.map((g) =>
            g.id === id && g.anr ? { ...g, anr: { ...g.anr, version: tag } } : g,
          ),
          markVersionFor: null,
        }));
        get().toast({ kind: 'ok', title: '已标记版本', desc: '演示模式' });
        return;
      }
      try {
        const games = await call<Game[]>('mark_version', { id, tag });
        set({ games, markVersionFor: null });
        void get().loadVersions();
        get().toast({ kind: 'ok', title: '已标记版本', desc: `指纹已写入对照表，指向 ${tag}` });
      } catch (e) {
        get().toast({ kind: 'danger', title: '标记失败', desc: errorText(e) });
      }
    },

    refreshCatalog: async () => {
      if (!isTauri) {
        get().toast({ kind: 'info', title: '浏览器预览模式不读取本地指纹表' });
        return;
      }
      set({ catalogBusy: true });
      try {
        const r = await call<{ added: number; total: number; remote: boolean }>('refresh_catalog');
        await get().rescan();
        get().toast({
          kind: 'ok',
          title: '指纹表已刷新',
          desc: `新增 ${r.added} 条，共 ${r.total} 条${r.remote ? '（含远程表）' : '（远程表不可用，已扫描本地包）'}`,
        });
      } catch (e) {
        get().toast({ kind: 'danger', title: '刷新失败', desc: errorText(e) });
      } finally {
        set({ catalogBusy: false });
      }
    },

    pickZip: async () => {
      if (!isTauri) return null;
      try {
        return await call<string | null>('pick_zip_file');
      } catch (e) {
        get().toast({ kind: 'danger', title: '选择文件失败', desc: errorText(e) });
        return null;
      }
    },

    officialSum: async (tag, filename) => {
      if (!isTauri) return '';
      return await call<string>('official_sum', { tag, filename });
    },

    importLocalPackage: async (path, tag, kind, sha256) => {
      if (!isTauri) {
        set({ importOpen: false });
        get().toast({ kind: 'info', title: '浏览器预览模式无法导入本地包' });
        return;
      }
      try {
        await call('import_local_package', { path, tag, kind, sha256: sha256 || null });
        await get().loadVersions(true);
        set({ importOpen: false });
        get().toast({
          kind: 'ok',
          title: `${tag} 已导入`,
          desc: '已通过 SHA256 校验，安装时不会再下载',
        });
      } catch (e) {
        get().toast({ kind: 'danger', title: '导入失败', desc: errorText(e) });
      }
    },

    checkPackageStructure: async () => {
      if (!isTauri) return;
      try {
        const bad = await call<string[]>('check_package_structure');
        if (bad.length) {
          get().toast({
            kind: 'warn',
            title: '官方包结构已变更，请更新本工具',
            desc: `缓存中的 ${bad.join('、')} 缺少 install.ps1`,
          });
        }
      } catch {
        /* 自检失败不打断启动 */
      }
    },

    installAll: async (ids) => {
      // 顺序 await，避免多个确认弹窗叠在一起；后端队列同样保证串行执行
      for (const id of ids) {
        await get().install(id);
      }
    },

    setTheme: (theme) => {
      const resolved =
        theme === 'system'
          ? window.matchMedia('(prefers-color-scheme: light)').matches
            ? 'light'
            : 'dark'
          : theme;
      document.documentElement.dataset.theme = resolved;
      const settings = { ...get().settings, theme };
      set({ theme, settings });
      if (isTauri) void call('set_settings', { settings }).catch(() => undefined);
    },
    setView: (view) => {
      if (!isTauri) {
        const url = window.location.pathname + window.location.search + `#${view}`;
        window.history.replaceState(null, '', url);
      }
      set({ view, selected: [] });
    },
    setFilter: (filter) => set({ filter }),
    setQuery: (query) => set({ query }),
    setLayout: (layout) => {
      set({ layout });
      // 6.4②：写进设置，下次启动恢复上次选的网格/列表
      get().updateSettings({ layout });
    },
    openDrawer: (drawerId) => set({ drawerId }),
    closeDrawer: () => set({ drawerId: null }),
    setPalette: (paletteOpen) => set({ paletteOpen }),
    setAddOpen: (addOpen) => set({ addOpen }),
    setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
    setClosePrompt: (closePrompt) => set({ closePrompt }),
    setFailure: (failure) => set({ failure }),
    setRenameId: (renameId) => set({ renameId }),
    setMoveDir: (moveDir) => set({ moveDir }),
    restoreSnapshot: async (id, path) => {
      const g = get().games.find((x) => x.id === id);
      if (!g) return;
      if (g.running) {
        get().toast({ kind: 'warn', title: `${g.name} 正在运行`, desc: '请先退出游戏再恢复快照' });
        return;
      }
      const ok = await new Promise<boolean>((resolve) => {
        get().askConfirm({
          title: `恢复快照到 ${g.name}？`,
          body: '会用快照里的 OptiScaler 目录、代理 DLL 和 OptiScaler.ini 覆盖当前游戏目录里的文件。',
          items: [
            '恢复前不会删除快照文件，可以多次恢复',
            '当前的 ANR 文件会被覆盖，游戏需要先退出',
            '恢复完成后会重新扫描该游戏的状态',
          ],
          confirmLabel: '恢复快照',
          tone: 'danger',
          action: () => resolve(true),
          onCancel: () => resolve(false),
        });
      });
      if (!ok) return;
      try {
        const games = await call<Game[]>('restore_snapshot', { id, path });
        set({ games, failure: null });
        get().toast({ kind: 'ok', title: '快照已恢复', desc: '已重新扫描游戏状态' });
      } catch (e) {
        get().toast({ kind: 'danger', title: '恢复快照失败', desc: errorText(e) });
      }
    },
    setLogGame: (logGameId) => {
      set({ logGameId, logText: '' });
      void get().loadLog(logGameId);
    },
    updateSettings: (patch) => {
      const settings = { ...get().settings, ...patch };
      if (patch.density) {
        document.documentElement.dataset.density =
          patch.density === 'compact' ? 'compact' : 'cozy';
      }
      if (patch.fontSize !== undefined) applyUiScale(patch.fontSize);
      if (patch.reduceMotion !== undefined) {
        document.documentElement.dataset.motion = patch.reduceMotion ? 'reduced' : 'full';
      }
      set({ settings });
      if (isTauri) void call('set_settings', { settings }).catch(() => undefined);
    },

    toggleSelect: (id) =>
      set((s) => ({
        selected: s.selected.includes(id)
          ? s.selected.filter((x) => x !== id)
          : [...s.selected, id],
      })),
    selectMany: (ids) => set({ selected: ids }),
    clearSelect: () => set({ selected: [] }),

    toast: (t) => {
      const id = uid('toast');
      set((s) => ({ toasts: [...s.toasts, { ...t, id }] }));
      setTimeout(() => {
        set((s) => ({ toasts: s.toasts.filter((x) => x.id !== id) }));
      }, 4200);
    },
    dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((x) => x.id !== id) })),

    upsertTask: (p) => {
      set((s) => {
        const exists = s.tasks.some((t) => t.id === p.id);
        const task: Task = {
          id: p.id,
          title: p.title,
          subtitle: p.subtitle,
          stage: p.stage,
          progress: p.progress <= 1 ? p.progress * 100 : p.progress,
          status: p.status,
          cancellable: false,
        };
        return {
          tasks: exists
            ? s.tasks.map((t) => (t.id === p.id ? { ...t, ...task } : t))
            : [...s.tasks, task],
        };
      });
      const cur = get().tasks.find((t) => t.id === p.id);
      if (cur && (cur.status === 'done' || cur.status === 'error')) {
        setTimeout(() => {
          set((s) => ({ tasks: s.tasks.filter((t) => t.id !== p.id) }));
        }, 3200);
      }
    },

    askConfirm: (confirm) => set({ confirm }),
    resolveConfirm: (ok) => {
      const req = get().confirm;
      set({ confirm: null });
      if (!req) return;
      if (ok) void req.action();
      else req.onCancel?.();
    },

    install: async (id) => {
      const g = get().games.find((x) => x.id === id);
      if (!g) return;
      if (g.running) {
        get().toast({
          kind: 'warn',
          title: `${g.name} 正在运行`,
          desc: '请先关闭游戏，安装器会拒绝写入正在被占用的文件',
        });
        return;
      }
      const force = await allowOperation(g, '安装');
      if (!force) return;
      if (!isTauri) {
        const kind: InstallKind = g.thirdPartyOptiScaler ? 'addon' : 'full';
        runTask(kind === 'addon' ? '插件安装' : '全量安装', g.name, INSTALL_STEPS, id, () => {
          const latest = get().versions.find((v) => v.latest)!;
          patchGame(id, (game) => ({
            ...game,
            anr: {
              kind,
              version: latest.tag,
              proxy: 'dxgi.dll',
              installedAt: new Date().toISOString(),
              enabled: true,
              fingerprint: latest.fingerprint,
              config: {
                path: 'auto',
                modelSize: 'large',
                nits: 80,
                maxGenerated: 5,
                sceneCut: true,
              },
              health: 'ok',
              logPath: '',
            },
          }));
          get().toast({ kind: 'ok', title: `${g.name} 安装完成` });
        });
        return;
      }
      // 前置检查：block 直接拦下，warn 让用户确认后再继续
      let issues: PrecheckIssue[] = [];
      try {
        issues = await call<PrecheckIssue[]>('precheck_install', { id });
      } catch {
        issues = [];
      }
      const block = issues.find((i) => i.level === 'block');
      if (block) {
        get().toast({ kind: 'danger', title: block.title, desc: block.detail });
        return;
      }
      const warns = issues.filter((i) => i.level === 'warn');
      if (warns.length) {
        const go = await new Promise<boolean>((resolve) => {
          get().askConfirm({
            title: `装到「${g.name}」之前，有 ${warns.length} 处需要确认`,
            body: warns.map((w) => w.title).join('；'),
            items: warns.map((w) => w.detail),
            confirmLabel: '仍然安装',
            tone: 'warn',
            action: () => resolve(true),
            onCancel: () => resolve(false),
          });
        });
        if (!go) return;
      }

      try {
        await call('install', { id, tag: null, force });
        await get().rescan();
        get().toast({
          kind: 'ok',
          title: `${g.name} 安装完成`,
          desc: '进入游戏开启帧生成即可',
        });
      } catch (e) {
        await get().rescan();
        const info = parseFailure(errorText(e));
        set({
          failure: {
            ...info,
            gameId: id,
            gameName: g.name,
            kind: 'install',
            retry: () => get().install(id),
          },
        });
        get().toast({
          kind: 'danger',
          title: '安装失败',
          desc: info.message.split('\n')[0].slice(0, 120),
        });
      }
    },

    uninstall: async (id, thenRemove = false) => {
      const g = get().games.find((x) => x.id === id);
      if (!g || !g.anr) return;
      if (g.running) {
        get().toast({ kind: 'warn', title: `${g.name} 正在运行`, desc: '请先关闭游戏再卸载' });
        return;
      }
      const force = await allowOperation(g, '卸载');
      if (!force) return;
      const kind = g.anr.kind;
      const proxy = g.anr.proxy;
      get().askConfirm({
        title: `从 ${g.name} 卸载 ANR？`,
        body:
          kind === 'addon'
            ? '插件安装的卸载只会移除 ANR 目录，并把 OptiScaler.ini 恢复成安装前的值，不会动你原有的 OptiScaler。'
            : '全量安装的卸载会删除该游戏的整个 OptiScaler 目录，并还原安装前的 dxgi.dll 与 OptiScaler.ini。',
        items:
          kind === 'addon'
            ? [
                '将移除：OptiScaler\\custom\\Apex Neural Rendering FG',
                '将恢复：OptiScaler.ini 的 FGInput / FGNvngxReplacement',
                '不会删除：你原有的 OptiScaler 本体',
              ]
            : [
                '将删除：OptiScaler\\custom\\Apex Neural Rendering FG',
                `将还原：${proxy}（安装前备份）`,
                '将还原：OptiScaler.ini（安装前备份）',
                '卸载前会自动生成快照，可随时恢复',
              ],
        confirmLabel: '卸载',
        tone: 'danger',
        action: async () => {
          if (!isTauri) {
            runTask('卸载', g.name, REMOVE_STEPS, id, () => {
              patchGame(id, (game) => ({ ...game, anr: null }));
              get().toast({ kind: 'ok', title: '已卸载' });
            });
            return;
          }
          try {
            await call('uninstall', { id, force });
            await get().rescan();
            get().toast({
              kind: 'ok',
              title: `${g.name} 已卸载`,
              desc: '原文件已按哈希校验还原',
            });
            if (thenRemove) await get().removeGame(id);
          } catch (e) {
            get().toast({ kind: 'danger', title: '卸载失败', desc: errorText(e) });
          }
        },
      });
    },

    setFgReplacement: async (id, mode) => {
      const g = get().games.find((x) => x.id === id);
      if (!g || !g.anr) return;
      if (g.running) {
        get().toast({ kind: 'warn', title: `${g.name} 正在运行`, desc: '请先关闭游戏再改' });
        return;
      }
      const force = await allowOperation(g, '切换帧生成实现');
      if (!force) return;
      try {
        if (isTauri) await call('set_fg_replacement', { id, mode, force });
        else
          patchGame(id, (game) =>
            game.anr
              ? {
                  ...game,
                  anr: {
                    ...game.anr,
                    fgReplacement:
                      mode === 'anr' ? 'Apex Neural Rendering FG' : mode === 'native' ? 'None' : 'auto',
                  },
                }
              : game,
          );
        if (isTauri) await get().rescan();
        get().toast({
          kind: 'ok',
          title:
            mode === 'anr' ? '已切到 ANR' : mode === 'native' ? '已切到原生 DLSS-G' : '已切到自动',
          desc: '重启游戏生效',
        });
      } catch (e) {
        get().toast({ kind: 'danger', title: '切换失败', desc: errorText(e) });
      }
    },

    switchVersion: async (id, tag) => {
      const g = get().games.find((x) => x.id === id);
      if (!g || !g.anr) return;
      if (g.anr.version === tag) return;
      if (g.running) {
        get().toast({ kind: 'warn', title: `${g.name} 正在运行`, desc: '请先关闭游戏再切换版本' });
        return;
      }
      const force = await allowOperation(g, '切换版本');
      if (!force) return;
      if (!isTauri) {
        runTask('切换版本', `${g.name} → ${tag}`, SWITCH_STEPS, id, () => {
          const v = get().versions.find((x) => x.tag === tag);
          patchGame(id, (game) =>
            game.anr
              ? {
                  ...game,
                  anr: {
                    ...game.anr,
                    version: tag,
                    fingerprint: v?.fingerprint ?? game.anr.fingerprint,
                    installedAt: new Date().toISOString(),
                    health: 'ok',
                    healthNote: undefined,
                  },
                }
              : game,
          );
          get().toast({ kind: 'ok', title: `已切换到 ${tag}` });
        });
        return;
      }
      try {
        await call('switch_version', { id, tag, force });
        await get().rescan();
        get().toast({ kind: 'ok', title: `已切换到 ${tag}`, desc: 'nr_fg.ini 参数已保留' });
      } catch (e) {
        await get().rescan();
        const info = parseFailure(errorText(e));
        set({
          failure: {
            ...info,
            gameId: id,
            gameName: g.name,
            kind: 'switch',
            retry: () => get().switchVersion(id, tag),
          },
        });
        get().toast({
          kind: 'danger',
          title: '切换版本失败',
          desc: info.message.split('\n')[0].slice(0, 120),
        });
      }
    },

    toggleEnabled: async (id) => {
      const g = get().games.find((x) => x.id === id);
      if (!g || !g.anr) return;
      const force = await allowOperation(g, '改帧生成开关');
      if (!force) return;
      const next = !g.anr.enabled;
      patchGame(id, (game) => (game.anr ? { ...game, anr: { ...game.anr, enabled: next } } : game));
      if (!isTauri) {
        get().toast({
          kind: next ? 'ok' : 'info',
          title: `${g.name} 帧生成已${next ? '开启' : '关闭'}`,
        });
        return;
      }
      try {
        await call('set_fg_enabled', { id, enabled: next, force });
        get().toast({
          kind: next ? 'ok' : 'info',
          title: `${g.name} 帧生成已${next ? '开启' : '关闭'}`,
          desc: g.running ? '已写入配置，重启游戏后生效' : '已写入 OptiScaler.ini',
        });
      } catch (e) {
        patchGame(id, (game) =>
          game.anr ? { ...game, anr: { ...game.anr, enabled: !next } } : game,
        );
        get().toast({ kind: 'danger', title: '写入失败', desc: errorText(e) });
      }
    },

    updateConfig: (id, patch) => {
      patchGame(id, (game) => (game.anr ? { ...game, anr: { ...game.anr, ...patch } } : game));
      if (!isTauri) return;
      const g = get().games.find((x) => x.id === id);
      if (g?.anr) void call('write_config', { id, config: g.anr.config }).catch(() => undefined);
    },

    cancelTask: (id) => {
      canceled.add(id);
      if (isTauri) void call('cancel_task', { taskId: id }).catch(() => undefined);
      patchTask(id, { stage: '正在取消…' });
    },

    addGame: async (path) => {
      if (!path) {
        set({ addOpen: true });
        return;
      }
      if (!isTauri) {
        const id = uid('manual');
        set((s) => ({
          games: [
            {
              id,
              name: 'Nioh 3',
              source: 'manual',
              dir: path,
              exe: 'nioh3.exe',
              lastUsed: new Date().toISOString(),
              sizeGB: 0,
              anr: null,
            },
            ...s.games,
          ],
        }));
        get().toast({ kind: 'info', title: '已添加游戏', desc: '演示数据' });
        return;
      }
      try {
        const warn = await call<string | null>('add_manual_game', { path });
        await get().rescan();
        set({ addOpen: false });
        get().toast({ kind: 'ok', title: '已添加游戏' });
        if (warn) {
          get().toast({ kind: 'warn', title: '这个目录在 Steam 库里', desc: warn });
        }
      } catch (e) {
        get().toast({ kind: 'danger', title: '添加失败', desc: errorText(e) });
      }
    },

    removeGame: async (id) => {
      const g = get().games.find((x) => x.id === id);
      if (!g) return;
      if (isTauri) {
        try {
          const games = await call<Game[]>('remove_game', { id });
          set({ games });
        } catch (e) {
          get().toast({ kind: 'danger', title: '移除失败', desc: errorText(e) });
          return;
        }
      } else {
        set((s) => ({ games: s.games.filter((x) => x.id !== id) }));
      }
      get().toast({ kind: 'info', title: '已移出列表', desc: '游戏文件未做任何改动' });
    },

    changeGameDir: async (id, path, migrate) => {
      const g = get().games.find((x) => x.id === id);
      if (!g) return;
      if (!isTauri) {
        set({ moveDir: null });
        return;
      }
      try {
        const games = await call<Game[]>('change_game_dir', { id, path, migrate });
        // 目录变了 id 也变了，旧抽屉里的数据作废
        set((s) => ({ games, moveDir: null, drawerId: s.drawerId === id ? null : s.drawerId }));
        const normalized = path.replace(/\\/g, '/').toLowerCase();
        const next = games.find((x) => x.dir.replace(/\\/g, '/').toLowerCase() === normalized);
        if (migrate && next) {
          await get().install(next.id);
        }
        get().toast({
          kind: 'ok',
          title: '游戏目录已更改',
          desc: migrate ? '旧目录已卸载，正在安装到新目录' : '旧目录的安装没有改动',
        });
      } catch (e) {
        get().toast({ kind: 'danger', title: '更改目录失败', desc: errorText(e) });
      }
    },

    clearCover: async (id) => {
      try {
        if (isTauri) await call('clear_cover', { id });
        set((s) => ({ games: s.games.map((x) => (x.id === id ? { ...x, cover: null } : x)) }));
        get().toast({ kind: 'info', title: '封面已删除', desc: '下次搜索会重新抓一张' });
      } catch (e) {
        get().toast({ kind: 'danger', title: '删除封面失败', desc: errorText(e) });
      }
    },
  };
});
