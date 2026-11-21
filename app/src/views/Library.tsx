import { useMemo } from 'react';
import { Check, LayoutGrid, Library as LibraryIcon, List, Plus, Search, X } from 'lucide-react';
import { clsx, needsAttention, versionKnown } from '../lib';
import { useApp } from '../store';
import { GameCard, GameRow } from '../components/GameCard';
import { Empty, Segmented } from '../components/ui';
import type { Filter } from '../types';

const FILTERS: { value: Filter; label: string }[] = [
  { value: 'all', label: '全部' },
  { value: 'installed', label: '已安装' },
  { value: 'missing', label: '未安装' },
  { value: 'outdated', label: '可更新' },
  { value: 'attention', label: '需注意' },
];

export function Library() {
  const {
    games,
    versions,
    filter,
    setFilter,
    query,
    setQuery,
    layout,
    setLayout,
    selected,
    clearSelect,
    selectMany,
    addGame,
    installMany,
    uninstallMany,
    toggleMany,
    switchMany,
  } = useApp();
  const latest = versions.find((v) => v.latest)?.tag ?? versions[0]?.tag ?? '—';

  const counts: Record<Filter, number> = {
    all: games.length,
    installed: games.filter((g) => g.anr).length,
    missing: games.filter((g) => !g.anr).length,
    outdated: versionKnown(latest)
      ? games.filter((g) => g.anr && g.anr.version !== latest).length
      : 0,
    attention: games.filter((g) => needsAttention(g)).length,
  };

  const list = useMemo(() => {
    const key = query.trim().toLowerCase();
    return games
      .filter((g) => {
        if (key && !g.name.toLowerCase().includes(key) && !g.dir.toLowerCase().includes(key))
          return false;
        if (filter === 'installed') return !!g.anr;
        if (filter === 'missing') return !g.anr;
        if (filter === 'outdated') return versionKnown(latest) && !!g.anr && g.anr.version !== latest;
        if (filter === 'attention') return needsAttention(g);
        return true;
      })
      .sort((a, b) => {
        if (!!a.anr !== !!b.anr) return a.anr ? -1 : 1;
        return +new Date(b.lastUsed) - +new Date(a.lastUsed);
      });
  }, [games, filter, query, latest]);

  const allSelected = list.length > 0 && list.every((g) => selected.includes(g.id));

  return (
    <div className="relative flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-line px-[var(--dens-pad-x)] py-3">
        <Segmented
          options={FILTERS.map((f) => ({ value: f.value, label: `${f.label} ${counts[f.value]}` }))}
          value={filter}
          onChange={setFilter}
          size="sm"
        />

        <div className="relative ml-auto">
          <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-ink-3" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="搜索游戏"
            className="field h-8 w-[190px] pl-7 text-[12px]"
          />
          {query && (
            <button
              onClick={() => setQuery('')}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-ink-3 hover:text-ink"
              aria-label="清除搜索"
            >
              <X size={12} />
            </button>
          )}
        </div>

        <button
          onClick={() => (allSelected ? clearSelect() : selectMany(list.map((g) => g.id)))}
          className="btn btn-ghost h-8 px-2 text-[11px]"
          title={allSelected ? '取消全选' : '全选当前列表'}
          aria-pressed={allSelected}
        >
          <span
            className={clsx(
              'grid h-[13px] w-[13px] place-items-center rounded-[4px] border',
              allSelected ? 'border-transparent bg-accent text-white' : 'border-line',
            )}
          >
            {allSelected && <Check size={9} strokeWidth={3} />}
          </span>
          全选
        </button>

        <Segmented
          options={[
            { value: 'grid', label: '', icon: <LayoutGrid size={13} /> },
            { value: 'list', label: '', icon: <List size={13} /> },
          ]}
          value={layout}
          onChange={setLayout}
          size="sm"
        />

        <button onClick={() => void addGame()} className="btn btn-primary h-8 text-[12px]">
          <Plus size={13} />
          添加游戏
        </button>
      </div>

      <div className="scroll-thin flex-1 overflow-y-auto px-[var(--dens-pad-x)] py-[var(--dens-pad-y)]">
        {list.length === 0 ? (
          <Empty
            icon={<LibraryIcon size={17} />}
            title="这里还没有游戏"
            desc="添加游戏目录，或换一个筛选条件。真实版本会自动扫描 Steam / Epic / GOG 库。"
            action={
              <button onClick={() => void addGame()} className="btn btn-primary h-8 text-[12px]">
                <Plus size={13} />
                添加游戏目录
              </button>
            }
          />
        ) : layout === 'grid' ? (
          <div className="card-grid">
            {list.map((g) => (
              <GameCard key={g.id} game={g} />
            ))}
          </div>
        ) : (
          <div className="space-y-1.5">
            <div className="flex items-center gap-3 px-3 pb-1.5 text-[10.5px] text-ink-3">
              <button
                onClick={() => (allSelected ? clearSelect() : selectMany(list.map((g) => g.id)))}
                className="w-[16px] shrink-0"
                aria-label="全选"
              >
                <span
                  className={clsx(
                    'block h-[14px] w-[14px] rounded-full border',
                    allSelected ? 'border-transparent bg-accent' : 'border-line',
                  )}
                />
              </button>
              <span className="flex-1">游戏</span>
              <span className="w-[62px] md:w-[62px]">版本</span>
              <span className="hidden w-[52px] md:block">来源</span>
              <span className="w-[34px]">开关</span>
            </div>
            {list.map((g) => (
              <GameRow key={g.id} game={g} latest={latest} />
            ))}
          </div>
        )}
      </div>

      {selected.length > 0 && (
        <div className="anim-toast absolute bottom-5 left-1/2 z-30 -translate-x-1/2">
          <div className="flex items-center gap-2 rounded-[12px] border border-line bg-surface px-3 py-2 shadow-[var(--s-shadow)]">
            <span className="text-[12px] text-ink-2">
              已选 <span className="num font-medium text-ink">{selected.length}</span> 个
            </span>
            <span className="mx-1 h-4 w-px bg-line" />
            <button
              className="btn btn-primary h-7 text-[11.5px]"
              onClick={() => void installMany(selected)}
            >
              批量安装
            </button>
            {versionKnown(latest) && (
              <button
                className="btn btn-ghost h-7 text-[11.5px]"
                onClick={() => void switchMany(selected, latest)}
                title={`把选中的游戏都切换/更新到 ${latest}`}
              >
                更新到 {latest}
              </button>
            )}
            <button
              className="btn btn-ghost h-7 text-[11.5px]"
              onClick={() => void toggleMany(selected)}
            >
              批量开关
            </button>
            <button
              className="btn btn-danger h-7 text-[11.5px]"
              onClick={() => void uninstallMany(selected)}
            >
              批量卸载
            </button>
            <button className="btn btn-icon h-7 w-7" onClick={clearSelect} aria-label="取消选择">
              <X size={13} />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
