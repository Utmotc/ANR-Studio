import en from './en.json';

export type Locale = 'zh' | 'en';

/** 语言名称用各自母语书写；新增语言只需在这里注册 + 加一份字典 */
export const LOCALES: { id: Locale; name: string; short: string }[] = [
  { id: 'zh', name: '简体中文', short: '中' },
  { id: 'en', name: 'English', short: 'EN' },
];

/** 手工补漏：渲染时才发现、源码扫描不易抓到的片段 */
const PATCH: Record<string, string> = {
  已选: 'Selected',
  查看日志: 'View log',
  诊断包: 'Diagnostic bundle',
  '写入 OptiScaler.ini 的 [FrameGen] Enabled；关闭只是停用，文件仍留在游戏目录':
    'Writes [FrameGen] Enabled in OptiScaler.ini; turning it off only disables it — the files stay in the game folder',
  '写的是 OptiScaler.ini 的 FGNvngxReplacement，重启游戏生效':
    'Writes FGNvngxReplacement in OptiScaler.ini; takes effect after restarting the game',
  已切到自动: 'Switched to auto',
  '当前为 auto，由 OptiScaler 自己决定用哪个 provider；不确定时选这个。':
    'Set to auto — OptiScaler decides which provider to use. Pick this when unsure.',
  // 0.4.0：日志跟随 / 崩溃提示 / 自动更新 / 代理 DLL
  跟随中: 'Following',
  已暂停: 'Paused',
  暂停自动跟随: 'Pause auto-follow',
  恢复自动跟随: 'Resume auto-follow',
  '重新读取（回到日志末尾）': 'Reload (jump to end)',
  上次运行出现了异常: 'Last run crashed',
  界面异常: 'UI error',
  程序崩溃: 'Crash',
  生成诊断包: 'Diagnostic bundle',
  忽略这次崩溃报告: 'Ignore this crash report',
  已忽略这次崩溃报告: 'Crash report ignored',
  崩溃诊断包已生成: 'Crash diagnostic bundle created',
  界面渲染出错了: 'The interface hit an error',
  重新加载界面: 'Reload interface',
  复制错误信息: 'Copy error details',
  自动检查更新: 'Check for updates',
  仅提示: 'Notify only',
  当前版本: 'Current version',
  检查: 'Check',
  开始下载更新: 'Starting update download',
  更新失败: 'Update failed',
  已经是最新版本: 'Already up to date',
  检查更新失败: 'Update check failed',
  崩溃报告目录: 'Crash report folder',
  更换代理DLL失败: 'Failed to switch proxy DLL',
  重新安装并切换: 'Reinstall and switch',
  '更换代理名需要重新执行一次官方安装脚本，安装包没缓存时会先下载。':
    'Changing the proxy name re-runs the official install script, which may download the package first.',
  'dxgi.dll：通用情况，绝大多数游戏用这个':
    'dxgi.dll — the general choice; works for most games',
  'winmm.dll：Vulkan 游戏，或目录里已有别人的 dxgi 代理':
    'winmm.dll — Vulkan games, or when another dxgi proxy already exists',
  'version.dll：前两个都冲突时的兜底':
    'version.dll — fallback when both of the above conflict',
  '切换前会自动生成快照，失败可回滚':
    'A snapshot is taken before switching; failures can roll back',
  通用: 'general',
  '通用，绝大多数游戏选这个': 'General choice for most games',
  'Vulkan 游戏，或目录里已有别人的 dxgi 代理':
    'Vulkan games, or when another dxgi proxy already exists',
  '前两个都冲突时用它兜底': 'Fallback when the other two conflict',
  '游戏运行中不能更换代理名，请先退出游戏。':
    'Cannot change the proxy name while the game is running. Exit the game first.',
  '正在检查更新…': 'Checking for updates…',
  '正在下载并安装更新，进度见任务中心': 'Downloading and installing — see the task center',
  还没检查过: 'Not checked yet',
  '· 每秒刷新': '· refreshing every second',
};

const DICTS: Record<Locale, Record<string, string>> = {
  zh: {},
  en: { ...(en as Record<string, string>), ...PATCH },
};

let current: Locale = 'zh';
export const getLocale = () => current;

const CJK = /[\u4e00-\u9fff]/;

/** 带变量/数字的句子，整句匹配后再交给子串替换 */
type Rule = [RegExp, (m: RegExpMatchArray) => string];
const RULES: Rule[] = [
  [/^已选\s*(\d+)\s*个$/, (m) => `${m[1]} selected`],
  [/^包缓存占用\s*([\d.]+)\s*MB$/, (m) => `Package cache ${m[1]} MB`],
  [/^释放\s*([\d.]+)\s*MB$/, (m) => `Freed ${m[1]} MB`],
  [/^全部更新到\s*(.+)$/, (m) => `Update all to ${m[1]}`],
  [/^可更新到\s*(.+)，切换会保留你的参数$/, (m) => `Update to ${m[1]} — your settings are kept`],
  [/^一键全部安装（(\d+)）$/, (m) => `Install all (${m[1]})`],
  [/^默认安装版本：(.+)$/, (m) => `Default version: ${m[1]}`],
  [/^停在\s*(.+)，可以更新到\s*(.+)$/, (m) => `On ${m[1]}; update to ${m[2]}`],
  [/^装到「(.+)」之前，有\s*(\d+)\s*处需要确认$/, (m) => `Before installing into "${m[1]}": ${m[2]} things to confirm`],
  [/^从\s*(.+)\s*卸载 ANR？$/, (m) => `Uninstall ANR from ${m[1]}?`],
  [/^恢复快照到\s*(.+)\s*？$/, (m) => `Restore snapshot to ${m[1]}?`],
  [/^把\s*(.+)\s*设为默认安装版本$/, (m) => `Set ${m[1]} as the default version`],
  [/^取消\s*(.+)\s*的默认版本$/, (m) => `Clear ${m[1]} as default`],
  [/^删除\s*(.+)\s*的缓存$/, (m) => `Delete cache for ${m[1]}`],
  [/^展开\s*(.+)\s*的更新日志$/, (m) => `Expand changelog for ${m[1]}`],
  [/^收起\s*(.+)\s*的更新日志$/, (m) => `Collapse changelog for ${m[1]}`],
  [/^已添加\s*(\d+)\s*个游戏$/, (m) => `Added ${m[1]} games`],
  [/^新增\s*(\d+)\s*条，共\s*(\d+)\s*条(.+)$/, (m) => `Added ${m[1]}, ${m[2]} total${m[3]}`],
  [/^(\d+)\s*个游戏$/, (m) => `${m[1]} games`],
  [/^版本列表可能不是最新（(.+)）$/, (m) => `Version list may be outdated (${m[1]})`],
  [/^已找到\s*(\d+)\s*个游戏，其中\s*(\d+)\s*个还没装 ANR：$/, (m) => `Found ${m[1]} games; ${m[2]} have no ANR yet:`],
  [/^将移除：(.+)$/, (m) => `Will remove: ${m[1]}`],
  [/^将删除：(.+)$/, (m) => `Will delete: ${m[1]}`],
  [/^将还原：(.+)$/, (m) => `Will restore: ${m[1]}`],
  [/^将恢复：(.+)$/, (m) => `Will restore: ${m[1]}`],
  [/^已缓存\s*([\d.]+)\s*MB$/, (m) => `Cached ${m[1]} MB`],
  [/^(\d+)\s*天前$/, (m) => `${m[1]} days ago`],
  [/^(\d+)\s*落后$/, (m) => `${m[1]} behind`],
  [/^全部\s*(\d+)$/, (m) => `All ${m[1]}`],
  [/^可更新\s*(\d+)$/, (m) => `Updatable ${m[1]}`],
  [/^启用\s*(\d+)$/, (m) => `Enabled ${m[1]}`],
  [/^未启用\s*(\d+)$/, (m) => `Disabled ${m[1]}`],
  [/^未安装\s*(\d+)$/, (m) => `Not installed ${m[1]}`],
  [/^已安装\s*(\d+)$/, (m) => `Installed ${m[1]}`],
  [/^需注意\s*(\d+)$/, (m) => `Needs attention ${m[1]}`],
  [/^游戏库共\s*(\d+)$/, (m) => `Library total ${m[1]}`],
  [/^当前：(.+)（界面元素一起缩放）$/, (m) => `Current: ${m[1]} (scales all UI elements)`],
  [/^更新全部可更新游戏（(\d+)）$/, (m) => `Update all outdated games (${m[1]})`],
  [/^升级到\s*(.+)$/, (m) => `Upgrade to ${m[1]}`],
  [/^(.+)\s*·\s*已启用$/, (m) => `${m[1]} · Enabled`],
  [/^(.+)\s*·\s*已关闭$/, (m) => `${m[1]} · Disabled`],
  [/^(.+)\s*·\s*未启用$/, (m) => `${m[1]} · Disabled`],
  [/^把\s*(.+)\s*设为默认(?:安装)?版本$/, (m) => `Set ${m[1]} as the default version`],
  [/^其中\s*(\d+)\s*个还没装 ANR：$/, (m) => `${m[1]} have no ANR yet:`],
  [/^已经找到\s*(\d+)\s*个游戏，其中\s*(\d+)\s*个还没装 ANR：$/, (m) => `Found ${m[1]} games; ${m[2]} have no ANR yet:`],
  [/^已经找到\s*(\d+)\s*个游戏，都已安装过 ANR。$/, (m) => `Found ${m[1]} games; all of them already have ANR.`],
  [/^可回滚到\s*(.+)$/, (m) => `Roll back to ${m[1]}`],
  [/^继续.+会改动这个目录里现有的 OptiScaler 文件$/, () => 'Continuing will change the existing OptiScaler files in this directory'],
  // 0.3.0：批量操作汇总、任务中心、体检与相对时间
  [/^(\d+)\s*分钟前$/, (m) => `${m[1]} min ago`],
  [/^(\d+)\s*小时前$/, (m) => `${m[1]} h ago`],
  [/^(\d+)\s*个任务进行中$/, (m) => `${m[1]} running`],
  [/^(\d+)\s*个任务失败$/, (m) => `${m[1]} failed`],
  [/^(\d+)\s*条历史$/, (m) => `${m[1]} in history`],
  [/^还有\s*(\d+)\s*条任务$/, (m) => `${m[1]} more tasks`],
  [/^任务中心（(\d+)）$/, (m) => `Task center (${m[1]})`],
  [/^已修复\s*(\d+)\s*项$/, (m) => `Fixed ${m[1]} item(s)`],
  [/^安装\s*(\d+)\s*个$/, (m) => `Install ${m[1]}`],
  [/^卸载\s*(\d+)\s*个$/, (m) => `Uninstall ${m[1]}`],
  [/^从\s*(\d+)\s*个游戏卸载 ANR？$/, (m) => `Uninstall ANR from ${m[1]} games?`],
  [/^把\s*(\d+)\s*个游戏切换到\s*(.+)\s*？$/, (m) => `Switch ${m[1]} games to ${m[2]}?`],
  [/^(打开|关闭)\s*(\d+)\s*个游戏的帧生成？$/, (m) => `${m[1] === '打开' ? 'Enable' : 'Disable'} frame generation for ${m[2]} games?`],
  [/^更新到\s*(.+)$/, (m) => `Update to ${m[1]}`],
  [/^把选中的游戏都切换\/更新到\s*(.+)$/, (m) => `Switch/update selected games to ${m[1]}`],
  [/^将安装：(.+)$/, (m) => `Will install: ${m[1]}`],
  [/^…还有\s*(\d+)\s*个$/, (m) => `…and ${m[1]} more`],
  [/^需注意：(.+)$/, (m) => `Note: ${m[1]}`],
  [/^(.+)（插件安装）$/, (m) => `${m[1]} (add-on)`],
  [/^(.+)（全量安装）$/, (m) => `${m[1]} (full)`],
  [/^批量安装：(\d+)\s*个成功$/, (m) => `Install: ${m[1]} succeeded`],
  [/^批量安装：成功\s*(\d+)，失败\s*(\d+)$/, (m) => `Install: ${m[1]} succeeded, ${m[2]} failed`],
  [/^批量卸载：(\d+)\s*个成功$/, (m) => `Uninstall: ${m[1]} succeeded`],
  [/^批量卸载：成功\s*(\d+)，失败\s*(\d+)$/, (m) => `Uninstall: ${m[1]} succeeded, ${m[2]} failed`],
  [/^批量打开：(\d+)\s*个成功$/, (m) => `Frame generation on: ${m[1]} succeeded`],
  [/^批量打开：成功\s*(\d+)，失败\s*(\d+)$/, (m) => `Frame generation on: ${m[1]} succeeded, ${m[2]} failed`],
  [/^批量关闭：(\d+)\s*个成功$/, (m) => `Frame generation off: ${m[1]} succeeded`],
  [/^批量关闭：成功\s*(\d+)，失败\s*(\d+)$/, (m) => `Frame generation off: ${m[1]} succeeded, ${m[2]} failed`],
  [/^批量切换到\s*(.+)：(\d+)\s*个成功$/, (m) => `Switch to ${m[1]}: ${m[2]} succeeded`],
  [/^批量切换到\s*(.+)：成功\s*(\d+)，失败\s*(\d+)$/, (m) => `Switch to ${m[1]}: ${m[2]} succeeded, ${m[3]} failed`],
  [/^(.+) 正在运行$/, (m) => `${m[1]} is running`],
  // 0.4.0：更新 / 崩溃 / 代理 DLL
  [/^发现新版本\s*(.+)$/, (m) => `New version ${m[1]} available`],
  [/^可用新版本\s*(.+)，安装包会验签$/, (m) => `${m[1]} available; the package is signature-verified`],
  [/^已是最新（v(.+)）$/, (m) => `Up to date (v${m[1]})`],
  [/^把\s*(.+)\s*的代理 DLL 换成\s*(.+)\s*？$/, (m) => `Switch ${m[1]} to proxy ${m[2]}?`],
  [/^代理已切到\s*(.+)$/, (m) => `Proxy switched to ${m[1]}`],
  [/^已切到\s*(.+)$/, (m) => `Switched to ${m[1]}`],
  [/^共\s*(\d+)\s*条$/, (m) => `${m[1]} total`],
  [/^当前 v(.+)$/, (m) => `Current v${m[1]}`],
  [/^可用新版本\s*(.+)$/, (m) => `New version ${m[1]}`],
];

function translate(src: string): string {
  if (!CJK.test(src)) return src;
  const trimmed = src.trim();
  // 精确匹配（字典键就是渲染出来的文本片段）；空串译文也是合法结果
  const exact = DICTS[current][trimmed];
  if (exact !== undefined) return src.replace(trimmed, exact);
  for (const [re, fn] of RULES) {
    const m = trimmed.match(re);
    if (m) return src.replace(trimmed, fn(m));
  }
  // 不做子串替换：把「版本 / 工具 / 游戏」这类短词塞进句子里会翻出
  // 「写入Versions资源」这种坏句子。未收录的片段宁可保留中文。
  return src;
}

/** 精确翻译入口：以后组件里也可以直接用 t('某句中文') */
export function t(text: string): string {
  return DICTS[current][text] ?? text;
}

const ATTRS = ['title', 'aria-label', 'placeholder', 'alt'];
const SKIP_PARENTS = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEXTAREA', 'SVG']);
const writtenText = new WeakMap<Node, string>();
const writtenAttr = new WeakMap<Element, Record<string, string>>();

function applyText(node: Text) {
  const cur = node.nodeValue ?? '';
  if (writtenText.get(node) === cur) return;
  if (!cur || !CJK.test(cur)) return;
  const parent = node.parentElement;
  if (parent && SKIP_PARENTS.has(parent.tagName)) return;
  const next = translate(cur);
  if (next !== cur) {
    writtenText.set(node, next);
    node.nodeValue = next;
  }
}

function applyElement(el: Element) {
  const rec = writtenAttr.get(el) ?? {};
  let changed = false;
  for (const attr of ATTRS) {
    const cur = el.getAttribute(attr) ?? '';
    if (!cur || rec[attr] === cur || !CJK.test(cur)) continue;
    const next = translate(cur);
    if (next !== cur) {
      rec[attr] = next;
      el.setAttribute(attr, next);
      changed = true;
    }
  }
  if (changed) writtenAttr.set(el, rec);
}

function walk(root: Node) {
  const tw = document.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT);
  let node: Node | null = tw.currentNode;
  while (node) {
    if (node.nodeType === Node.TEXT_NODE) applyText(node as Text);
    else if (node.nodeType === Node.ELEMENT_NODE) applyElement(node as Element);
    node = tw.nextNode();
  }
}

let observer: MutationObserver | null = null;
function ensureObserver() {
  if (observer) return;
  observer = new MutationObserver((records) => {
    if (current === 'zh') return;
    for (const r of records) {
      if (r.type === 'characterData') {
        applyText(r.target as Text);
      } else if (r.type === 'childList') {
        for (const n of r.addedNodes) {
          if (n.nodeType === Node.TEXT_NODE) applyText(n as Text);
          else if (n.nodeType === Node.ELEMENT_NODE) walk(n);
        }
      } else if (r.type === 'attributes') {
        applyElement(r.target as Element);
      }
    }
  });
  observer.observe(document.body, {
    subtree: true,
    childList: true,
    characterData: true,
    attributes: true,
    attributeFilter: ATTRS,
  });
}

/**
 * 切换界面语言。中文是源码原样，所以切回中文时由 App 用 key={locale}
 * 重新挂载整棵树即可，不需要反向恢复；其它语言在这棵新树上做一次覆盖。
 */
export function applyLocale(locale: Locale) {
  current = locale;
  if (locale === 'zh') return;
  walk(document.body);
  ensureObserver();
}
