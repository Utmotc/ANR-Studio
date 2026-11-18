import { useState } from 'react';
import {
  Check,
  ChevronRight,
  Download,
  ExternalLink,
  HardDrive,
  Info,
  Pencil,
  RefreshCw,
  Star,
  Trash2,
  Upload,
} from 'lucide-react';
import { clsx, fullDate } from '../lib';
import { call, errorText, isTauri } from '../lib/ipc';
import { useApp } from '../store';
import { SectionLabel } from '../components/ui';

function ChangelogPanel({ tag }: { tag: string }) {
  const { changelogs, changelogPending, changelogError, retryChangelog } = useApp();
  const data = changelogs[tag];
  const loading = changelogPending[tag];
  const failed = changelogError[tag];
  const hasZh = !!data?.items.some((i) => i.zh);

  const openRelease = (url: string) => {
    if (!url) return;
    if (isTauri) void call('open_url', { url }).catch(() => undefined);
    else window.open(url, '_blank');
  };

  return (
    <div className="border-t border-line-soft bg-surface-2/25 px-4 py-3">
      <div className="mb-2 flex items-center gap-2">
        <span className="text-[10.5px] tracking-[0.04em] text-ink-3">更新日志</span>
        {!!data?.url && (
          <button
            className="ml-auto inline-flex items-center gap-1 text-[10.5px] text-ink-3 transition-colors hover:text-accent"
            onClick={() => openRelease(data.url)}
          >
            GitHub 原页
            <ExternalLink size={10} />
          </button>
        )}
      </div>

      {loading && (
        <div className="space-y-2 py-0.5">
          <div className="h-[9px] w-[78%] animate-pulse rounded bg-surface-3" />
          <div className="h-[9px] w-[62%] animate-pulse rounded bg-surface-3" />
          <div className="h-[9px] w-[70%] animate-pulse rounded bg-surface-3" />
        </div>
      )}

      {!loading && !!failed && (
        <div className="flex items-start gap-2 py-0.5">
          <p className="min-w-0 flex-1 text-[11px] leading-relaxed text-ink-3">
            读取更新日志失败：{failed}
          </p>
          <button
            className="btn btn-ghost h-6 shrink-0 px-2 text-[10.5px]"
            onClick={() => void retryChangelog(tag)}
          >
            重试
          </button>
        </div>
      )}

      {!loading && !failed && !data?.items.length && (
        <p className="py-0.5 text-[11px] text-ink-3">
          这个版本没有提供更新日志
          {data?.url ? '，可以点右上角「GitHub 原页」在浏览器里看。' : '。'}
        </p>
      )}

      {!loading && !!data?.items.length && (
        <ul className="space-y-2">
          {data.items.map((it, i) => (
            <li key={i} className="flex gap-2">
              <span className="mt-[7px] h-[3px] w-[3px] shrink-0 rounded-full bg-ink-3" />
              <div className="min-w-0">
                <p className="text-[11.5px] leading-relaxed text-ink-2">{it.zh || it.en}</p>
                {!!it.zh && (
                  <p className="mt-0.5 text-[10.5px] leading-relaxed text-ink-3">{it.en}</p>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}

      {!loading && hasZh && (
        <p className="mt-2.5 text-[10px] text-ink-3">
          中文为自动翻译，仅供参考；以英文原文为准。
        </p>
      )}
    </div>
  );
}

export function Versions() {
  const {
    versions,
    games,
    toast,
    settings,
    versionsLoading,
    versionsError,
    catalogBusy,
    loadVersions,
    deleteVersion,
    loadChangelog,
    setDefaultVersion,
    setImportOpen,
    setMarkVersionFor,
    refreshCatalog,
  } = useApp();
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [refreshing, setRefreshing] = useState(false);
  const cachedMB =
    Math.round(versions.filter((v) => v.cached).reduce((a, v) => a + v.sizeMB, 0) * 10) / 10;
  // 没下载过的版本没有指纹，整行放进对照表只会得到一堆空行
  const fingerprints = versions.filter((v) => v.fingerprint);

  const refresh = async () => {
    setRefreshing(true);
    await loadVersions(true);
    setRefreshing(false);
  };

  const toggle = (tag: string) => {
    setOpen((prev) => {
      const next = new Set(prev);
      if (next.has(tag)) {
        next.delete(tag);
      } else {
        next.add(tag);
        void loadChangelog(tag);
      }
      return next;
    });
  };

  return (
    <div className="scroll-thin h-full overflow-y-auto">
      <div className="mx-auto w-full max-w-[980px] px-[var(--dens-pad-x)] py-[var(--dens-pad-y)]">
        <div className="mb-6 flex items-start gap-2.5 rounded-[12px] border border-line bg-surface-2/40 p-3.5">
          <Info size={14} className="mt-[2px] shrink-0 text-ink-3" />
          <p className="text-[11.5px] leading-relaxed text-ink-2">
            ANR 的 DLL 没有写入版本资源，所以本工具用{' '}
            <span className="text-ink">SHA256 指纹</span>{' '}
            识别每个游戏装的是哪一版。切换版本会重新执行官方安装脚本，你的{' '}
            <span className="mono text-[11px]">nr_fg.ini</span>{' '}
            参数会自动保留。点版本前面的箭头可以展开这一版的更新日志。
          </p>
        </div>

        <SectionLabel
          extra={
            <div className="flex items-center gap-1">
              <button
                className="btn btn-ghost h-6 px-2 text-[10.5px]"
                title="从本地 zip 导入安装包（仍需官方 SHA256 校验）"
                onClick={() => setImportOpen(true)}
              >
                <Upload size={11} />
                导入本地包
              </button>
              <button
                className="btn btn-ghost h-6 px-2 text-[10.5px]"
                title="重新从 GitHub 拉取版本列表"
                disabled={refreshing || versionsLoading}
                onClick={() => void refresh()}
              >
                <RefreshCw
                  size={11}
                  className={clsx((refreshing || versionsLoading) && 'animate-spin')}
                />
                刷新
              </button>
            </div>
          }
        >
          可用版本
        </SectionLabel>
        <p className="mb-2.5 text-[10.5px] text-ink-3">
          只保留最近 5 个版本，以及已下载或正在使用的历史版本。默认安装版本：
          <span className="mono text-ink-2">
            {settings.defaultVersion || versions.find((v) => v.latest)?.tag || '跟随最新'}
          </span>
          {!settings.defaultVersion && '（未单独设置）'}
        </p>

        {!!versionsError && (
          <div className="mb-2.5 flex items-center gap-2 rounded-[10px] border border-line bg-surface-2/60 px-3 py-2">
            <p className="min-w-0 flex-1 text-[11px] text-ink-2">
              版本列表可能不是最新（{versionsError}）
              {versions.length ? '，下面显示的是上次成功获取的数据。' : '。'}
            </p>
            <button
              className="btn btn-ghost h-6 shrink-0 px-2 text-[10.5px]"
              onClick={() => void refresh()}
            >
              重试
            </button>
          </div>
        )}

        <div className="overflow-hidden rounded-[14px] border border-line bg-surface">
          {versionsLoading && versions.length === 0 && (
            <div className="space-y-3 p-4">
              {[0, 1, 2].map((i) => (
                <div key={i} className="flex items-center gap-3">
                  <div className="h-6 w-6 animate-pulse rounded bg-surface-3" />
                  <div className="h-[10px] w-[104px] animate-pulse rounded bg-surface-3" />
                  <div className="h-[10px] flex-1 animate-pulse rounded bg-surface-3" />
                  <div className="h-[10px] w-[80px] animate-pulse rounded bg-surface-3" />
                </div>
              ))}
            </div>
          )}

          {!versionsLoading && versions.length === 0 && (
            <div className="px-4 py-8 text-center">
              <p className="text-[12px] text-ink-2">
                {versionsError ? '没能连上 GitHub，暂时拿不到版本列表' : '暂时没有可用版本'}
              </p>
              <button
                className="btn btn-ghost mt-3 h-7 text-[11.5px]"
                onClick={() => void refresh()}
              >
                <RefreshCw size={12} />
                重新获取
              </button>
            </div>
          )}

          {versions.map((v, i) => {
            const used = games.filter((g) => g.anr?.version === v.tag);
            const expanded = open.has(v.tag);
            const isDefault = settings.defaultVersion === v.tag;
            return (
              <div key={v.tag} className={clsx(i > 0 && 'border-t border-line-soft')}>
                <div className="flex items-center gap-3 px-3 py-3.5">
                  <button
                    className="btn btn-icon h-6 w-6 shrink-0"
                    title={expanded ? '收起更新日志' : '展开更新日志'}
                    aria-label={`${expanded ? '收起' : '展开'} ${v.tag} 的更新日志`}
                    aria-expanded={expanded}
                    onClick={() => toggle(v.tag)}
                  >
                    <ChevronRight
                      size={13}
                      className={clsx('transition-transform duration-150', expanded && 'rotate-90')}
                    />
                  </button>

                  <div className="flex w-[104px] shrink-0 items-center gap-1.5">
                    <span
                      className={clsx(
                        'mono whitespace-nowrap text-[13px]',
                        v.latest ? 'font-medium text-ink' : 'text-ink-2',
                      )}
                    >
                      {v.tag}
                    </span>
                    {v.latest && (
                      <span className="chip h-[17px] bg-accent-soft px-1.5 text-[10px] text-accent">
                        最新
                      </span>
                    )}
                  </div>

                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[12px] text-ink">
                      {isDefault && (
                        <span className="mr-1.5 rounded bg-accent-soft px-1.5 py-[1px] text-[9.5px] text-accent">
                          默认
                        </span>
                      )}
                      {v.product}
                      <span className="text-ink-3"> · </span>
                      <span className="text-ink-2">{v.notes}</span>
                    </p>
                    <p className="mono mt-1 truncate text-[10.5px] text-ink-3">
                      {v.fingerprint ? `${v.fingerprint}… · ` : ''}
                      {v.sizeMB ? `${v.sizeMB} MB · ` : ''}
                      {fullDate(v.publishedAt)}
                    </p>
                  </div>

                  <div className="w-[86px] shrink-0 text-right text-[11px] text-ink-3">
                    {used.length > 0 ? `${used.length} 个游戏` : '未使用'}
                  </div>

                  <div className="flex w-[196px] shrink-0 items-center justify-end gap-1">
                    <button
                      className={clsx(
                        'btn btn-ghost h-6 px-1.5 text-[10px]',
                        isDefault && 'text-accent',
                      )}
                      title={isDefault ? '取消默认，改为跟随最新版本' : `把 ${v.tag} 设为默认安装版本`}
                      aria-label={isDefault ? `取消 ${v.tag} 的默认版本` : `把 ${v.tag} 设为默认版本`}
                      onClick={() => void setDefaultVersion(isDefault ? null : v.tag)}
                    >
                      <Star size={11} fill={isDefault ? 'currentColor' : 'none'} />
                      {isDefault ? '取消默认' : '设为默认'}
                    </button>
                    {v.cached ? (
                      <div className="flex items-center gap-1">
                        <span className="inline-flex items-center gap-1 text-[11px] text-ok">
                          <Check size={12} /> 已缓存
                        </span>
                        <button
                          className="btn btn-icon h-6 w-6"
                          title={`删除 ${v.tag} 的缓存`}
                          aria-label={`删除 ${v.tag} 的缓存`}
                          onClick={() => void deleteVersion(v.tag)}
                        >
                          <Trash2 size={12} />
                        </button>
                      </div>
                    ) : (
                      <button
                        className="btn btn-ghost h-7 text-[11.5px]"
                        onClick={async () => {
                          try {
                            await call('prefetch', { version: v.tag, kind: 'full' });
                            await loadVersions(true);
                            toast({
                              kind: 'ok',
                              title: `${v.tag} 已缓存`,
                              desc: '之后的安装不需要再下载',
                            });
                          } catch (e) {
                            toast({ kind: 'danger', title: '下载失败', desc: errorText(e) });
                          }
                        }}
                      >
                        <Download size={12} />
                        下载
                      </button>
                    )}
                  </div>
                </div>

                {expanded && <ChangelogPanel tag={v.tag} />}
              </div>
            );
          })}
        </div>

        <div className="mt-4 flex items-center gap-2.5 rounded-[12px] border border-line bg-surface px-4 py-3">
          <HardDrive size={14} className="text-ink-3" />
          <div className="flex-1">
            <p className="text-[12px]">包缓存占用 {cachedMB} MB</p>
            <p className="mt-0.5 text-[10.5px] text-ink-3">
              下载过的安装包会在多个游戏之间复用，不需要重复下载
            </p>
          </div>
          <button
            className="btn btn-ghost h-7 text-[11.5px]"
            onClick={async () => {
              try {
                const freed = await call<number>('clear_cache');
                await loadVersions(true);
                toast({
                  kind: 'info',
                  title: '缓存已清理',
                  desc: `释放 ${(freed / 1048576).toFixed(1)} MB`,
                });
              } catch (e) {
                toast({ kind: 'danger', title: '清理失败', desc: errorText(e) });
              }
            }}
          >
            <Trash2 size={12} />
            清理
          </button>
        </div>

        <div className="mt-7">
          <SectionLabel
            extra={
              <div className="flex items-center gap-1">
                <button
                  className="btn btn-ghost h-6 px-2 text-[10.5px]"
                  title="版本显示为「未知」时，手动指定它属于哪一版"
                  onClick={() =>
                    setMarkVersionFor({ gameId: games.find((g) => g.anr)?.id ?? '' })
                  }
                >
                  <Pencil size={11} />
                  手动标记
                </button>
                <button
                  className="btn btn-ghost h-6 px-2 text-[10.5px]"
                  title="重扫本地安装包并尝试拉取最新的指纹表"
                  disabled={catalogBusy}
                  onClick={() => void refreshCatalog()}
                >
                  <RefreshCw size={11} className={clsx(catalogBusy && 'animate-spin')} />
                  刷新指纹表
                </button>
              </div>
            }
          >
            指纹对照表
          </SectionLabel>
        </div>
        <div className="mb-8 overflow-hidden rounded-[14px] border border-line bg-surface">
          {fingerprints.length === 0 ? (
            <p className="px-4 py-3 text-[11px] text-ink-3">
              还没有可用指纹：安装或下载任一版本后，这里会记录该版本的 provider_sha256。
            </p>
          ) : (
            fingerprints.map((v, i) => (
              <div
                key={v.tag}
                className={clsx(
                  'flex items-center justify-between px-4 py-2.5',
                  i > 0 && 'border-t border-line-soft',
                )}
              >
                <span className="mono text-[11px] text-ink-2">{v.fingerprint}</span>
                <span className="mono text-[11px] text-ink-3">→ {v.tag}</span>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}
