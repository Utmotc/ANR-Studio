import { useEffect, useMemo, useState } from 'react';
import { AlertTriangle, Check, EyeOff, RefreshCw, SlidersHorizontal } from 'lucide-react';
import { clsx } from '../lib';
import { call, errorText, isTauri } from '../lib/ipc';
import { useApp } from '../store';
import type { Game } from '../types';
import { Cover, Empty, Select } from '../components/ui';

type Kind = 'toggle' | 'select' | 'slider' | 'number' | 'text' | 'status';

interface Field {
  tab: string;
  groupCn: string;
  groupEn: string;
  section: string;
  key: string;
  label: string;
  labelEn: string;
  hint: string;
  value: string;
  kind: Kind;
  options: string[];
  min: number;
  max: number;
  step: number;
  known: boolean;
  readonly: boolean;
}

interface TabInfo {
  id: string;
  cn: string;
  en: string;
}

interface Snapshot {
  tabs: TabInfo[];
  fields: Field[];
  running: boolean;
  path: string;
  mtime: number;
}

export function Tuning() {
  const { games, toast } = useApp();
  const installed = useMemo(() => games.filter((g) => g.anr), [games]);
  const [activeId, setActiveId] = useState('');
  const [snap, setSnap] = useState<Snapshot | null>(null);
  const [tab, setTab] = useState('framegen');
  const [loading, setLoading] = useState(false);
  const [savingKey, setSavingKey] = useState('');
  const [savedKey, setSavedKey] = useState('');
  const [showRaw, setShowRaw] = useState<Record<string, boolean>>({});

  const game: Game | undefined = installed.find((g) => g.id === activeId) ?? installed[0];

  const load = async (id: string) => {
    if (!id || !isTauri) return;
    setLoading(true);
    try {
      setSnap(await call<Snapshot>('read_opti_tuning', { id }));
    } catch (e) {
      setSnap(null);
      toast({ kind: 'danger', title: '读取失败', desc: errorText(e) });
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (game?.id) void load(game.id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game?.id]);

  const save = async (f: Field, value: string) => {
    if (!game || f.readonly) return;
    if (value === f.value) return;
    const id = `${f.section}/${f.key}`;
    setSavingKey(id);
    try {
      const normalized = await call<string>('write_opti_tuning', {
        id: game.id,
        section: f.section,
        key: f.key,
        value,
      });
      setSnap((s) =>
        s
          ? {
              ...s,
              fields: s.fields.map((x) =>
                x.section === f.section && x.key === f.key ? { ...x, value: normalized } : x,
              ),
            }
          : s,
      );
      setSavedKey(id);
      window.setTimeout(() => setSavedKey(''), 1200);
    } catch (e) {
      toast({ kind: 'danger', title: `${f.label || f.key} 没保存成功`, desc: errorText(e) });
    } finally {
      setSavingKey('');
    }
  };

  const fields = snap?.fields ?? [];
  const tabs = snap?.tabs ?? [];
  const inTab = fields.filter((f) => f.tab === tab);

  const groups = useMemo(() => {
    const map = new Map<string, Field[]>();
    for (const f of inTab) {
      const k = `${f.groupCn}\u0001${f.groupEn}`;
      const arr = map.get(k) ?? [];
      arr.push(f);
      map.set(k, arr);
    }
    return Array.from(map.entries());
  }, [inTab]);

  if (installed.length === 0) {
    return (
      <div className="grid h-full place-items-center p-8">
        <Empty
          icon={<SlidersHorizontal size={17} />}
          title="还没有安装 ANR 的游戏"
          desc="装好之后，这里按游戏内菜单的顺序，用中英双语调整所有参数。"
        />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0">
      <aside className="flex w-[204px] shrink-0 flex-col border-r border-line">
        <div className="border-b border-line px-3.5 py-2.5">
          <span className="text-[11px] font-medium text-ink-2">
            已安装的 {installed.length} 个游戏
          </span>
        </div>
        <div className="scroll-thin min-h-0 flex-1 overflow-y-auto py-1">
          {installed.map((g) => (
            <button
              key={g.id}
              onClick={() => setActiveId(g.id)}
              className={clsx(
                'flex w-full items-center gap-2.5 px-3 py-2 text-left transition-colors',
                g.id === game?.id ? 'bg-accent-soft/60 text-ink' : 'text-ink-2 hover:bg-surface-2',
              )}
            >
              <Cover name={g.name} src={g.cover} className="h-7 w-7 shrink-0 rounded-[7px]" />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[12px]">{g.name}</span>
                <span className="mono block truncate text-[10px] text-ink-3">{g.anr?.version}</span>
              </span>
            </button>
          ))}
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex shrink-0 items-center gap-2.5 border-b border-line px-5 py-3">
          <div className="min-w-0 flex-1">
            <h2 className="truncate text-[13px] font-medium">{game?.name}</h2>
            <p className="mono mt-0.5 truncate text-[10px] text-ink-3">
              {snap?.path || 'OptiScaler.ini'}
            </p>
          </div>
          <button
            className="btn btn-icon h-7 w-7"
            onClick={() => game && void load(game.id)}
            aria-label="重新读取"
            title="重新读取（游戏里改了设置，刷新一下就同步）"
          >
            <RefreshCw size={12} className={loading ? 'animate-spin' : undefined} />
          </button>
        </div>

        {snap?.running && (
          <div className="mx-5 mt-3 flex items-start gap-2 rounded-[10px] border border-warn/30 bg-warn-soft px-3 py-2">
            <AlertTriangle size={13} className="mt-[1px] shrink-0 text-warn" />
            <p className="text-[11px] leading-relaxed text-warn">
              游戏正在运行，只能看不能改：OptiScaler 退出时会把内存里的设置写回去，覆盖这里的修改。
            </p>
          </div>
        )}

        <div className="scroll-thin flex shrink-0 gap-1 overflow-x-auto px-5 pt-3">
          {tabs.map((t) => {
            const n = fields.filter((f) => f.tab === t.id).length;
            return (
              <button
                key={t.id}
                onClick={() => setTab(t.id)}
                className={clsx(
                  'shrink-0 rounded-[8px] px-3 py-1.5 text-left transition-colors',
                  tab === t.id
                    ? 'bg-accent-soft text-accent'
                    : 'text-ink-3 hover:bg-surface-2 hover:text-ink',
                )}
              >
                <span className="block text-[11.5px] leading-tight">{t.cn}</span>
                <span className="block text-[9.5px] leading-tight opacity-70">
                  {t.en} · {n}
                </span>
              </button>
            );
          })}
        </div>

        <div className="scroll-thin min-h-0 flex-1 space-y-3 overflow-y-auto px-5 py-3">
          {groups.map(([gk, list]) => {
            const [gcn, gen] = gk.split('\u0001');
            const known = list.filter((f) => f.known);
            const unknown = list.filter((f) => !f.known);
            const open = showRaw[gk] ?? false;

            if (known.length === 0 && !open) {
              return (
                <div
                  key={gk}
                  className="flex items-center gap-2 rounded-[10px] border border-line-soft px-3.5 py-2"
                >
                  <span className="text-[11px] text-ink-3">{gcn}</span>
                  <button
                    className="ml-auto text-[10px] text-ink-3 transition-colors hover:text-ink"
                    onClick={() => setShowRaw((s) => ({ ...s, [gk]: true }))}
                  >
                    展开 {unknown.length} 项
                  </button>
                </div>
              );
            }

            if (gcn === '超分输入') {
              const matrix = list.filter((f) =>
                /^(Enable|Use|Fsr2Pattern|Fsr3Pattern)/.test(f.key),
              );
              const rest = list.filter((f) => !matrix.includes(f));
              return (
                <section key={gk} className="overflow-hidden rounded-[12px] border border-line">
                  <div className="flex items-center gap-2 border-b border-line-soft bg-surface-2/40 px-3.5 py-2">
                    <span className="text-[11.5px] font-medium text-ink-2">{gcn}</span>
                    <span className="text-[10px] text-ink-3">{gen} · Upscaler Inputs</span>
                  </div>
                  <InputsMatrix
                    fields={matrix}
                    locked={!!snap?.running}
                    onSave={(f, v) => void save(f, v)}
                  />
                  {rest.length > 0 && (
                    <div className="divide-y divide-line-soft border-t border-line-soft">
                      {rest.map((f) => (
                        <div key={f.key} className="flex items-center gap-3 px-3.5 py-2">
                          <div className="min-w-0 flex-1">
                            <span className="text-[11.5px] text-ink">{f.label}</span>
                            <span className="ml-1.5 text-[10.5px] text-ink-3">{f.labelEn}</span>
                          </div>
                          <Control field={f} disabled={!!snap?.running} onChange={(v) => void save(f, v)} />
                        </div>
                      ))}
                    </div>
                  )}
                </section>
              );
            }

            return (
              <section key={gk} className="overflow-hidden rounded-[12px] border border-line">
                <div className="flex items-center gap-2 border-b border-line-soft bg-surface-2/40 px-3.5 py-2">
                  <span className="text-[11.5px] font-medium text-ink-2">{gcn}</span>
                  <span className="text-[10px] text-ink-3">{gen}</span>
                  {unknown.length > 0 && (
                    <button
                      className="ml-auto inline-flex items-center gap-1 text-[10px] text-ink-3 hover:text-ink"
                      onClick={() => setShowRaw((s) => ({ ...s, [gk]: !open }))}
                    >
                      <EyeOff size={10} />
                      {open ? '隐藏' : `另有 ${unknown.length} 项`}
                    </button>
                  )}
                </div>

                <div className="divide-y divide-line-soft">
                  {[...known, ...(open ? unknown : [])].map((f) => {
                    const id = `${f.section}/${f.key}`;
                    const locked = !!snap?.running || f.readonly;
                    const busy = savingKey === id;
                    const done = savedKey === id;
                    return (
                      <div
                        key={id}
                        className={clsx(
                          'flex items-center gap-3 px-3.5 py-2',
                          (snap?.running || f.readonly) && 'opacity-70',
                        )}
                      >
                        <div className="min-w-0 flex-1">
                          <div className="flex flex-wrap items-baseline gap-x-1.5">
                            {f.known ? (
                              <>
                                <span className="text-[11.5px] text-ink">{f.label}</span>
                                <span className="text-[10.5px] text-ink-3">{f.labelEn}</span>
                              </>
                            ) : (
                              <>
                                <span className="mono text-[11px] text-ink-2">{f.key}</span>
                                <span className="rounded-[4px] bg-surface-2 px-1 text-[9px] text-ink-3">
                                  未翻译
                                </span>
                              </>
                            )}
                            {done && <Check size={11} className="text-ok" />}
                          </div>
                          {f.hint && (
                            <p className="mt-0.5 text-[10.5px] leading-relaxed text-ink-3">
                              {f.hint}
                            </p>
                          )}
                        </div>

                        <div className="flex w-[212px] shrink-0 justify-end">
                          <Control
                            field={f}
                            disabled={locked || busy}
                            onChange={(v) => void save(f, v)}
                          />
                        </div>
                      </div>
                    );
                  })}
                </div>
              </section>
            );
          })}

          {groups.length === 0 && (
            <p className="py-6 text-center text-[11px] text-ink-3">这一页没有可调项</p>
          )}

          <p className="pb-2 pt-1 text-[10.5px] leading-relaxed text-ink-3">
            分组与顺序照抄游戏内 Insert 菜单；改完重启游戏生效。写入游戏目录下的
            OptiScaler.ini，每次保存前都会备份。
          </p>
        </div>
      </div>
    </div>
  );
}


/** 还原游戏内 Upscaler Inputs 那张多选表：行是输入源，列是启用 / 使用 / 匹配串 */
function InputsMatrix({
  fields,
  locked,
  onSave,
}: {
  fields: Field[];
  locked: boolean;
  onSave: (f: Field, v: string) => void;
}) {
  const by = (k: string) => fields.find((f) => f.key === k);
  const rows: { cn: string; en: string; enable?: Field; use?: Field; pattern?: Field }[] = [
    { cn: 'FSR 2', en: 'FSR2', enable: by('EnableFsr2Inputs'), use: by('UseFsr2Inputs'), pattern: by('Fsr2Pattern') },
    { cn: 'FSR 3', en: 'FSR3', enable: by('EnableFsr3Inputs'), use: by('UseFsr3Inputs'), pattern: by('Fsr3Pattern') },
    { cn: 'FFX', en: 'FFX', enable: by('EnableFfxInputs'), use: by('UseFfxInputs') },
    { cn: 'DLSS', en: 'DLSS', enable: by('EnableDlssInputs') },
  ];

  const cycle = (f?: Field) => {
    if (!f || locked) return;
    const v = f.value.toLowerCase();
    onSave(f, v === 'true' ? 'false' : v === 'false' ? 'auto' : 'true');
  };

  const cell = (f?: Field) =>
    f ? (
      <button
        disabled={locked}
        onClick={() => cycle(f)}
        title={f.value}
        className={clsx(
          'grid h-5 w-5 place-items-center rounded-[5px] border transition-colors',
          f.value === 'true'
            ? 'border-accent bg-accent text-white'
            : f.value === 'false'
              ? 'border-line text-ink-3'
              : 'border-line text-accent',
        )}
      >
        {f.value === 'true' ? (
          <Check size={11} />
        ) : f.value === 'auto' ? (
          <span className="text-[8px] leading-none">A</span>
        ) : null}
      </button>
    ) : (
      <span className="text-[10px] text-ink-3">—</span>
    );

  return (
    <div className="px-3.5 py-2">
      <div className="grid grid-cols-[1fr_74px_74px_1fr] items-center gap-2 px-1 pb-1.5 text-[10px] text-ink-3">
        <span>输入源 Input</span>
        <span className="text-center">启用 Enable</span>
        <span className="text-center">使用 Use</span>
        <span>匹配串 Pattern</span>
      </div>
      <div className="divide-y divide-line-soft">
        {rows.map((r) => (
          <div
            key={r.en}
            className="grid grid-cols-[1fr_74px_74px_1fr] items-center gap-2 px-1 py-1.5"
          >
            <div className="min-w-0">
              <span className="text-[11.5px] text-ink">{r.cn}</span>
              <span className="ml-1.5 text-[10px] text-ink-3">{r.en}</span>
            </div>
            <span className="flex justify-center">{cell(r.enable)}</span>
            <span className="flex justify-center">{cell(r.use)}</span>
            <span className="min-w-0">
              {r.pattern ? (
                <TextBox field={r.pattern} disabled={locked} onChange={(v) => onSave(r.pattern!, v)} />
              ) : (
                <span className="text-[10px] text-ink-3">—</span>
              )}
            </span>
          </div>
        ))}
      </div>
      <p className="mt-1.5 px-1 text-[10px] leading-relaxed text-ink-3">
        点方框在 开 / 关 / 自动 之间轮换；A 表示 auto（跟随 OptiScaler 默认值）。
      </p>
    </div>
  );
}

const BOOL: [string, string][] = [
  ['true', '开 On'],
  ['false', '关 Off'],
  ['auto', '自动 Auto'],
];

function Control({
  field,
  disabled,
  onChange,
}: {
  field: Field;
  disabled: boolean;
  onChange: (v: string) => void;
}) {
  const isAuto = field.value.trim().toLowerCase() === 'auto' || field.value.trim() === '';

  if (field.kind === 'status') {
    return (
      <span className="mono max-w-[212px] truncate text-[10.5px] text-ink-2" title={field.value}>
        {field.value || '—'}
      </span>
    );
  }

  if (field.kind === 'toggle') {
    return (
      <div className="flex items-center gap-1">
        {BOOL.map(([v, label]) => (
          <button
            key={v}
            disabled={disabled}
            onClick={() => onChange(v)}
            className={clsx(
              'rounded-[6px] border px-1.5 py-[3px] text-[10px] transition-colors',
              field.value.toLowerCase() === v
                ? 'border-accent-soft bg-accent-soft text-accent'
                : 'border-line text-ink-3 hover:text-ink',
            )}
          >
            {label}
          </button>
        ))}
      </div>
    );
  }

  if (field.kind === 'select' && field.options.length > 0) {
    return (
      <Select
        value={field.value}
        onChange={(v) => onChange(v)}
        options={field.options.map((o) => ({ value: o, label: o }))}
        className="w-[212px]"
        disabled={disabled}
      />
    );
  }

  if (field.kind === 'slider') {
    return (
      <Slider value={field.value} field={field} disabled={disabled} onChange={onChange} auto={isAuto} />
    );
  }

  return <TextBox field={field} disabled={disabled} onChange={onChange} />;
}

function Slider({
  value,
  field,
  disabled,
  onChange,
  auto,
}: {
  value: string;
  field: Field;
  disabled: boolean;
  onChange: (v: string) => void;
  auto: boolean;
}) {
  const num = Number(value);
  const safe = Number.isFinite(num) ? num : field.min;
  const [draft, setDraft] = useState(safe);

  useEffect(() => {
    const n = Number(value);
    setDraft(Number.isFinite(n) ? n : field.min);
  }, [value, field.min]);

  return (
    <div className="flex w-[212px] items-center gap-2">
      <input
        type="range"
        className="range flex-1"
        min={field.min}
        max={field.max}
        step={field.step || 0.01}
        value={auto ? field.min : draft}
        disabled={disabled}
        onChange={(e) => setDraft(Number(e.target.value))}
        onMouseUp={() => onChange(String(draft))}
        onKeyUp={() => onChange(String(draft))}
        onTouchEnd={() => onChange(String(draft))}
      />
      <span
        className={clsx(
          'mono w-[44px] shrink-0 text-right text-[10.5px]',
          auto ? 'text-ink-3' : 'text-ink',
        )}
      >
        {auto ? '自动' : draft.toFixed(field.step >= 1 ? 0 : 2)}
      </span>
      <button
        disabled={disabled || auto}
        onClick={() => onChange('auto')}
        className={clsx(
          'rounded-[5px] border px-1 py-[2px] text-[9.5px] transition-colors',
          auto
            ? 'border-accent-soft bg-accent-soft text-accent'
            : 'border-line text-ink-3 hover:text-ink',
        )}
        title="恢复成 auto，跟随 OptiScaler 的默认值"
      >
        auto
      </button>
    </div>
  );
}

function TextBox({
  field,
  disabled,
  onChange,
}: {
  field: Field;
  disabled: boolean;
  onChange: (v: string) => void;
}) {
  const [draft, setDraft] = useState(field.value);
  useEffect(() => setDraft(field.value), [field.value]);
  return (
    <input
      className="input h-7 w-[212px] text-[11px]"
      value={draft}
      disabled={disabled}
      inputMode={field.kind === 'number' ? 'decimal' : undefined}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => onChange(draft)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
        if (e.key === 'Escape') setDraft(field.value);
      }}
    />
  );
}
