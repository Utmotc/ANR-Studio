import { useEffect, useState } from 'react';
import {
  Download,
  ExternalLink,
  FolderOpen,
  Info,
  Palette,
  RefreshCw,
  RotateCcw,
  Settings2,
  Shield,
  X,
} from 'lucide-react';
import { clsx, openExternal } from '../lib';
import { call } from '../lib/ipc';
import { FONT_LABELS, useApp } from '../store';
import { Segmented, Toggle } from '../components/ui';

type Section = 'look' | 'download' | 'safety' | 'general' | 'about';

const SECTIONS: { id: Section; label: string; icon: typeof Palette }[] = [
  { id: 'look', label: '外观', icon: Palette },
  { id: 'download', label: '下载', icon: Download },
  { id: 'safety', label: '安全', icon: Shield },
  { id: 'general', label: '通用', icon: Settings2 },
  { id: 'about', label: '关于', icon: Info },
];

function Row({
  title,
  desc,
  children,
  last,
}: {
  title: string;
  desc?: string;
  children: React.ReactNode;
  last?: boolean;
}) {
  return (
    <div
      className={clsx(
        'flex items-center gap-4 px-4 py-3.5',
        !last && 'border-b border-line-soft',
      )}
    >
      <div className="min-w-0 flex-1">
        <p className="text-[12.5px]">{title}</p>
        {desc && <p className="mt-0.5 text-[11px] leading-relaxed text-ink-3">{desc}</p>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Group({ title, children }: { title?: string; children: React.ReactNode }) {
  return (
    <section className="mb-5">
      {title && (
        <h2 className="mb-2 text-[11px] font-medium tracking-[0.04em] text-ink-3">{title}</h2>
      )}
      <div className="overflow-hidden rounded-[14px] border border-line bg-surface">{children}</div>
    </section>
  );
}

export function SettingsDialog() {
  const {
    settings,
    updateSettings,
    setTheme,
    toast,
    versions,
    settingsOpen,
    setSettingsOpen,
    refreshCatalog,
    catalogBusy,
    updateInfo,
    updateChecking,
    updateDownloading,
    updateError,
    checkUpdate,
    startUpdate,
  } = useApp();
  const [section, setSection] = useState<Section>('look');
  const close = () => setSettingsOpen(false);

  useEffect(() => {
    if (!settingsOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setSettingsOpen(false);
    };
    document.addEventListener('keydown', onKey);
    return () => document.removeEventListener('keydown', onKey);
  }, [settingsOpen, setSettingsOpen]);

  if (!settingsOpen) return null;

  const openData = async (which: 'data' | 'cache' | 'snapshots') => {
    try {
      const p = await call<Record<string, string>>('app_paths');
      const path = which === 'data' ? p.data : which === 'cache' ? p.cache : p.snapshots;
      await call('open_path', { path });
    } catch {
      toast({ kind: 'info', title: '需要在桌面应用里使用' });
    }
  };

  const pickCache = async () => {
    try {
      const picked = await call<string | null>('pick_folder');
      if (picked) updateSettings({ cacheDir: picked });
    } catch {
      toast({ kind: 'info', title: '需要在桌面应用里使用' });
    }
  };

  return (
    <div className="fixed inset-0 z-[70] grid place-items-center">
      <div className="anim-overlay absolute inset-0 bg-black/50" onClick={close} aria-hidden />
      <div className="anim-panel relative flex h-[min(640px,86vh)] w-[min(880px,calc(100vw-80px))] overflow-hidden rounded-[16px] border border-line bg-surface shadow-[var(--s-shadow)]">
      <aside className="w-[168px] shrink-0 border-r border-line bg-surface-2/30 py-3">
        <p className="px-4 pb-2 text-[10.5px] tracking-[0.04em] text-ink-3">设置</p>
        {SECTIONS.map((s) => {
          const Icon = s.icon;
          return (
            <button
              key={s.id}
              onClick={() => setSection(s.id)}
              data-active={section === s.id}
              className="nav-item mx-2 w-[calc(100%-16px)]"
            >
              <Icon size={15} strokeWidth={1.8} />
              <span className="flex-1 text-left">{s.label}</span>
            </button>
          );
        })}
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex shrink-0 items-center justify-between border-b border-line px-5 py-3">
          <span className="text-[12.5px] font-medium">
            {SECTIONS.find((s) => s.id === section)?.label}
          </span>
          <button className="btn btn-icon h-7 w-7" onClick={close} aria-label="关闭设置">
            <X size={14} />
          </button>
        </div>
        <div className="scroll-thin min-h-0 flex-1 overflow-y-auto">
          <div className="max-w-[640px] px-5 py-4">
          {section === 'look' && (
            <>
              <Group>
                <Row title="主题" desc="深色为默认，浅色完整对齐，不做降级处理">
                  <Segmented
                    options={[
                      { value: 'dark', label: '深色' },
                      { value: 'light', label: '浅色' },
                      { value: 'system', label: '跟随系统' },
                    ]}
                    value={settings.theme}
                    onChange={(v) => setTheme(v)}
                    size="sm"
                  />
                </Row>
                <Row title="界面密度" desc="紧凑模式收窄留白并增加每行卡片数量">
                  <Segmented
                    options={[
                      { value: 'cozy', label: '标准' },
                      { value: 'compact', label: '紧凑' },
                    ]}
                    value={settings.density}
                    onChange={(v) => updateSettings({ density: v as 'cozy' | 'compact' })}
                    size="sm"
                  />
                </Row>
                <Row title="字体大小" desc={`当前：${FONT_LABELS[settings.fontSize] ?? '中等'}（界面元素一起缩放）`} last>
                  <div className="w-[230px]">
                    <input
                      type="range"
                      min={0}
                      max={4}
                      step={1}
                      value={settings.fontSize}
                      onChange={(e) => updateSettings({ fontSize: Number(e.target.value) })}
                      className="slider"
                      aria-label="字体大小"
                    />
                    <div className="mt-1.5 flex justify-between text-[9.5px] text-ink-3">
                      {FONT_LABELS.map((l, i) => (
                        <button
                          key={l}
                          onClick={() => updateSettings({ fontSize: i })}
                          className={clsx(
                            'transition-colors hover:text-ink',
                            settings.fontSize === i && 'font-medium text-accent',
                          )}
                        >
                          {l}
                        </button>
                      ))}
                    </div>
                  </div>
                </Row>
              </Group>

              <Group title="动效">
                <Row title="减少动画" desc="关闭界面过渡与弹出动画，观感更硬朗" last>
                  <Toggle
                    checked={settings.reduceMotion}
                    onChange={() => updateSettings({ reduceMotion: !settings.reduceMotion })}
                  />
                </Row>
              </Group>
            </>
          )}

          {section === 'download' && (
            <>
              <Group title="下载">
                <Row
                  title="下载源"
                  desc="版本列表和安装包共用：直连不走系统代理，镜像走 GitHub 镜像；自动先跟系统代理，失败后自动换线"
                >
                  <Segmented
                    options={[
                      { value: 'auto', label: '自动' },
                      { value: 'github', label: '直连' },
                      { value: 'mirror', label: '镜像' },
                    ]}
                    value={settings.mirror}
                    onChange={(v) => updateSettings({ mirror: v as typeof settings.mirror })}
                    size="sm"
                  />
                </Row>
                <Row
                  title="游戏封面来源"
                  desc="自动：国外能连上就用 Steam 官方封面，连不上直接走国内；国内：360 图片；国外：Steam / GOG"
                >
                  <Segmented
                    options={[
                      { value: 'auto', label: '自动' },
                      { value: 'domestic', label: '国内' },
                      { value: 'global', label: '国外' },
                    ]}
                    value={settings.coverSource}
                    onChange={(v) =>
                      updateSettings({ coverSource: v as typeof settings.coverSource })
                    }
                    size="sm"
                  />
                </Row>
                <Row
                  title="同时进行的任务数"
                  desc="安装器不能同时写同一个目录。1 = 严格串行（推荐），调高后多个安装/下载会并行，出错风险也更高"
                >
                  <Segmented
                    options={[
                      { value: '1', label: '1 串行' },
                      { value: '2', label: '2' },
                      { value: '3', label: '3' },
                    ]}
                    value={String(settings.concurrency || 1)}
                    onChange={(v) => updateSettings({ concurrency: Number(v) })}
                    size="sm"
                  />
                </Row>
                <Row title="安装包缓存目录" desc={settings.cacheDir || '默认位置'} last>
                  <button className="btn btn-ghost h-7 text-[11.5px]" onClick={pickCache}>
                    <FolderOpen size={12} />
                    更改
                  </button>
                </Row>
              </Group>

              <Group title="缓存">
                <Row title="打开缓存目录" desc="下载过的安装包按版本存放在这里，多个游戏复用" last>
                  <button
                    className="btn btn-ghost h-7 text-[11.5px]"
                    onClick={() => void openData('cache')}
                  >
                    <FolderOpen size={12} />
                    打开
                  </button>
                </Row>
              </Group>
            </>
          )}

          {section === 'safety' && (
            <>
              <Group title="安全">
                <Row
                  title="反作弊游戏提醒"
                  desc="在已知带反作弊的游戏中安装前给出警告。不提供任何规避手段"
                >
                  <Toggle
                    checked={settings.warnAntiCheat}
                    onChange={() => updateSettings({ warnAntiCheat: !settings.warnAntiCheat })}
                  />
                </Row>
                <Row
                  title="卸载前自动快照"
                  desc="官方卸载会删除整个 OptiScaler 目录，快照保证可以完整恢复"
                  last
                >
                  <div className="flex items-center gap-2">
                    <span className="text-[10.5px] text-ink-3">强制开启</span>
                    <Toggle checked onChange={() => undefined} disabled />
                  </div>
                </Row>
              </Group>

              <Group title="校验与指纹">
                <Row
                  title="安装包 SHA256 校验"
                  desc="拿不到官方 SHA256SUMS.txt 就拒绝安装，校验不匹配直接删除；手动导入本地包同样校验，不允许跳过"
                >
                  <span className="text-[10.5px] text-ink-3">强制开启</span>
                </Row>
                <Row
                  title="指纹表"
                  desc="重扫本地安装包并尝试拉取最新指纹表，用来修好显示为「未知」的版本"
                  last
                >
                  <button
                    className="btn btn-ghost h-7 text-[11.5px]"
                    disabled={catalogBusy}
                    onClick={() => void refreshCatalog()}
                  >
                    <RefreshCw size={12} className={clsx(catalogBusy && 'animate-spin')} />
                    刷新
                  </button>
                </Row>
              </Group>

              <Group title="数据">
                <Row title="打开快照目录" desc="每次卸载前生成的 OptiScaler 目录压缩包" last>
                  <button
                    className="btn btn-ghost h-7 text-[11.5px]"
                    onClick={() => void openData('snapshots')}
                  >
                    <FolderOpen size={12} />
                    打开
                  </button>
                </Row>
              </Group>
            </>
          )}

          {section === 'general' && (
            <>
              <Group title="启动">
                <Row title="启动时打开的页面" desc="下次启动应用直接进入这一页" last>
                  <Segmented
                    options={[
                      { value: 'overview', label: '总览' },
                      { value: 'library', label: '游戏库' },
                      { value: 'versions', label: '版本' },
                      { value: 'tuning', label: '调校' },
                      { value: 'logs', label: '日志' },
                    ]}
                    value={settings.startupView}
                    onChange={(v) =>
                      updateSettings({ startupView: v as typeof settings.startupView })
                    }
                    size="sm"
                  />
                </Row>
              </Group>

              <Group title="窗口">
                <Row
                  title="关闭窗口时"
                  desc="最小化到托盘后关闭只隐藏窗口，任务继续在后台运行"
                  last
                >
                  <Segmented
                    options={[
                      { value: 'ask', label: '每次询问' },
                      { value: 'tray', label: '最小化到托盘' },
                      { value: 'exit', label: '直接退出' },
                    ]}
                    value={settings.closeAction}
                    onChange={(v) =>
                      updateSettings({ closeAction: v as typeof settings.closeAction })
                    }
                    size="sm"
                  />
                </Row>
              </Group>

              <Group title="本地数据">
                <Row title="打开数据目录" desc="设置、指纹表、封面缓存与快照都在这里">
                  <button
                    className="btn btn-ghost h-7 text-[11.5px]"
                    onClick={() => void openData('data')}
                  >
                    <FolderOpen size={12} />
                    打开
                  </button>
                </Row>
                <Row title="重置全部设置" desc="恢复默认外观与行为设置，不会删除游戏配置" last>
                  <button
                    className="btn btn-danger h-7 text-[11.5px]"
                    onClick={() => {
                      setTheme('dark');
                      updateSettings({
                        density: 'cozy',
                        fontSize: 2,
                        startupView: 'overview',
                        reduceMotion: false,
                        mirror: 'auto',
                        warnAntiCheat: true,
                      });
                      toast({ kind: 'info', title: '设置已恢复默认' });
                    }}
                  >
                    <RotateCcw size={12} />
                    重置
                  </button>
                </Row>
              </Group>
            </>
          )}

          {section === 'about' && (
            <>
              <Group title="更新">
                <Row
                  title="自动检查更新"
                  desc="启动 3 秒后静默检查；自动模式下会直接下载，装好后提示重启"
                >
                  <Segmented
                    size="sm"
                    value={settings.updateMode ?? 'prompt'}
                    onChange={(v) =>
                      updateSettings({ updateMode: v as typeof settings.updateMode })
                    }
                    options={[
                      { value: 'auto', label: '自动' },
                      { value: 'prompt', label: '仅提示' },
                      { value: 'off', label: '关闭' },
                    ]}
                  />
                </Row>
                <Row
                  title="当前版本"
                  desc={
                    updateDownloading
                      ? '正在下载并安装更新，进度见任务中心'
                      : updateChecking
                        ? '正在检查更新…'
                        : updateError
                          ? updateError
                          : updateInfo
                            ? updateInfo.available
                              ? `可用新版本 ${updateInfo.version}，安装包会验签`
                              : `已是最新（v${updateInfo.current}）`
                            : '还没检查过'
                  }
                  last
                >
                  <div className="flex items-center gap-1.5">
                    <span className="mono text-[11px] text-ink-3">
                      v{typeof __APP_VERSION__ === 'string' ? __APP_VERSION__ : '0.4.0'}
                    </span>
                    <button
                      className="btn btn-ghost h-7 text-[11px]"
                      disabled={updateChecking || updateDownloading}
                      onClick={() => void checkUpdate(true)}
                    >
                      <RefreshCw size={12} className={updateChecking ? 'animate-spin' : undefined} />
                      检查
                    </button>
                    {updateInfo?.available && !updateDownloading && (
                      <button
                        className="btn btn-primary h-7 text-[11px]"
                        onClick={() => void startUpdate()}
                      >
                        <Download size={12} />
                        更新到 {updateInfo.version}
                      </button>
                    )}
                  </div>
                </Row>
              </Group>

              <Group title="版本">
                <Row title="ANR Studio" desc="非官方管理工具 · 桌面版">
                  <span className="mono text-[11px] text-ink-3">
                    v{typeof __APP_VERSION__ === 'string' ? __APP_VERSION__ : '0.1.0'}
                  </span>
                </Row>
                <Row
                  title="默认安装版本"
                  desc="新安装时使用的版本；没单独设置就跟随最新版本"
                >
                  <span className="mono text-[11px] text-ink-3">
                    {settings.defaultVersion || versions.find((v) => v.latest)?.tag || '跟随最新'}
                  </span>
                </Row>
                <Row title="ANR 当前版本" desc="Apex Neural Rendering FG Beta，由 Apex Compute 发布" last>
                  <span className="mono text-[11px] text-ink-3">
                    {versions.find((v) => v.latest)?.tag ?? '—'}
                  </span>
                </Row>
              </Group>

              <div className="flex items-start gap-2.5 rounded-[12px] border border-line bg-surface-2/40 p-3.5">
                <Info size={14} className="mt-[2px] shrink-0 text-ink-3" />
                <div className="min-w-0">
                  <p className="text-[11.5px] leading-relaxed text-ink-2">
                    本工具为非官方项目，与 Apex Compute 无关联。不修改游戏本体，不写入内存，仅调用官方安装脚本并读写配置文件。
                  </p>
                  <div className="mt-2 flex flex-wrap gap-3">
                    <button
                      className="inline-flex items-center gap-1 text-[11px] text-accent hover:underline"
                      onClick={() => void openExternal('https://github.com/apex-compute/ANR')}
                    >
                      ANR 官方仓库
                      <ExternalLink size={11} />
                    </button>
                    <button
                      className="inline-flex items-center gap-1 text-[11px] text-accent hover:underline"
                      onClick={() => void openExternal('https://github.com/optiscaler/OptiScaler')}
                    >
                      OptiScaler（GPL-3.0）
                      <ExternalLink size={11} />
                    </button>
                  </div>
                </div>
              </div>
            </>
          )}
          </div>
        </div>
      </div>
      </div>
    </div>
  );
}
