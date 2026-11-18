import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { clsx, coverTints, initial, TONE_COLOR, type Tone } from '../lib';

export function Dot({ tone, size = 6 }: { tone: Tone; size?: number }) {
  return (
    <span
      className="inline-block shrink-0 rounded-full"
      style={{ width: size, height: size, background: TONE_COLOR[tone] }}
    />
  );
}

export function StatusTag({ label, tone }: { label: string; tone: Tone }) {
  return (
    <span className="inline-flex items-center gap-1.5 text-[11px] text-ink-2">
      <Dot tone={tone} />
      {label}
    </span>
  );
}

export function Toggle({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean;
  onChange: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={onChange}
      className={clsx(
        'relative h-[20px] w-[34px] shrink-0 rounded-full border transition-colors',
        checked ? 'border-transparent bg-accent' : 'border-line bg-surface-3',
        disabled && 'cursor-not-allowed opacity-40',
      )}
    >
      <span
        className={clsx(
          'absolute top-1/2 h-[14px] w-[14px] -translate-y-1/2 rounded-full bg-white transition-all duration-150',
          checked ? 'left-[17px]' : 'left-[2px]',
        )}
        style={{ boxShadow: '0 1px 2px rgba(0,0,0,.28)' }}
      />
    </button>
  );
}

export function Segmented<T extends string>({
  options,
  value,
  onChange,
  size = 'md',
}: {
  options: { value: T; label: string; icon?: ReactNode }[];
  value: T;
  onChange: (v: T) => void;
  size?: 'sm' | 'md';
}) {
  return (
    <div
      className={clsx(
        'inline-flex items-center gap-0.5 rounded-[9px] border border-line bg-surface p-0.5',
        size === 'sm' ? 'h-[28px]' : 'h-[32px]',
      )}
    >
      {options.map((o) => (
        <button
          key={o.value}
          onClick={() => onChange(o.value)}
          className={clsx(
            'inline-flex h-full items-center gap-1.5 rounded-[7px] px-2.5 text-[12px] transition-colors',
            value === o.value
              ? 'bg-surface-3 font-medium text-ink'
              : 'text-ink-3 hover:text-ink-2',
          )}
        >
          {o.icon}
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Progress({ value, tone = 'accent' }: { value: number; tone?: Tone }) {
  return (
    <div className="h-[3px] w-full overflow-hidden rounded-full bg-surface-3">
      <div
        className="h-full rounded-full transition-[width] duration-200 ease-out"
        style={{ width: `${Math.min(100, Math.max(0, value))}%`, background: TONE_COLOR[tone] }}
      />
    </div>
  );
}

export function Cover({
  name,
  className,
  glyph = 34,
  src,
}: {
  name: string;
  className?: string;
  glyph?: number;
  src?: string | null;
}) {
  if (src) {
    return (
      <div className={clsx('relative overflow-hidden bg-surface-2', className)}>
        <img
          src={src}
          alt={name}
          loading="lazy"
          className="h-full w-full object-cover"
          draggable={false}
        />
      </div>
    );
  }
  const [a, b] = coverTints(name);
  return (
    <div
      className={clsx('relative overflow-hidden', className)}
      style={{
        backgroundImage: `linear-gradient(135deg, ${a} 0 53%, ${b} 53% 100%)`,
      }}
    >
      <div className="absolute inset-0 grid place-items-center">
        <span
          className="font-medium leading-none text-white/75"
          style={{ fontSize: glyph, letterSpacing: '-0.02em' }}
        >
          {initial(name)}
        </span>
      </div>
    </div>
  );
}

export interface MenuAction {
  label: string;
  icon?: ReactNode;
  onClick: () => void;
  danger?: boolean;
  sep?: boolean;
  disabled?: boolean;
}

export function ContextMenu({
  x,
  y,
  items,
  onClose,
}: {
  x: number;
  y: number;
  items: MenuAction[];
  onClose: () => void;
}) {
  const left = Math.min(x, window.innerWidth - 190);
  const top = Math.min(y, window.innerHeight - (items.length * 30 + 16));

  return createPortal(
    <>
      <div className="fixed inset-0 z-[90]" onMouseDown={onClose} onContextMenu={(e) => e.preventDefault()} />
      <div className="menu-panel fixed z-[91]" style={{ left, top, minWidth: 168 }} role="menu">
        {items.map((it, i) =>
          it.sep ? (
            <div key={`s${i}`} className="menu-sep" />
          ) : (
            <button
              key={i}
              className="menu-item disabled:opacity-40"
              data-danger={it.danger ? 'true' : 'false'}
              disabled={it.disabled}
              onClick={() => {
                it.onClick();
                onClose();
              }}
            >
              <span className="shrink-0 text-ink-3">{it.icon}</span>
              <span className="flex-1 truncate text-left">{it.label}</span>
            </button>
          ),
        )}
      </div>
    </>,
    document.body,
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="kbd">{children}</kbd>;
}

export function SectionLabel({ children, extra }: { children: ReactNode; extra?: ReactNode }) {
  return (
    <div className="mb-2.5 flex items-baseline justify-between">
      <h2 className="text-[11px] font-medium tracking-[0.04em] text-ink-3">{children}</h2>
      {extra}
    </div>
  );
}

export function MetaRow({
  label,
  children,
  mono,
}: {
  label: string;
  children: ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="flex items-start justify-between gap-4 border-b border-line-soft py-2.5 last:border-0">
      <span className="shrink-0 text-[12px] text-ink-3">{label}</span>
      <span
        className={clsx(
          'min-w-0 truncate text-right text-[12px] text-ink',
          mono && 'mono text-[11.5px]',
        )}
      >
        {children}
      </span>
    </div>
  );
}

export function Empty({
  icon,
  title,
  desc,
  action,
}: {
  icon: ReactNode;
  title: string;
  desc?: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center rounded-[14px] border border-dashed border-line px-6 py-14 text-center">
      <div className="mb-3 grid h-10 w-10 place-items-center rounded-[10px] bg-surface-2 text-ink-3">
        {icon}
      </div>
      <p className="text-[13px] font-medium">{title}</p>
      {desc && <p className="mt-1 max-w-[320px] text-[12px] text-ink-3">{desc}</p>}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}

export function Select({
  value,
  onChange,
  options,
  className,
  disabled,
}: {
  value: string;
  onChange: (v: string) => void;
  options: { value: string; label: string }[];
  className?: string;
  disabled?: boolean;
}) {
  return (
    <div className={clsx('relative', className)}>
      <select
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        className="field h-8 w-full appearance-none pr-7 text-[12px] disabled:opacity-50"
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <svg
        className="pointer-events-none absolute right-2 top-1/2 h-3 w-3 -translate-y-1/2 text-ink-3"
        viewBox="0 0 12 12"
        fill="none"
      >
        <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
      </svg>
    </div>
  );
}

export function Stepper({
  value,
  onChange,
  min,
  max,
  suffix,
}: {
  value: number;
  onChange: (v: number) => void;
  min: number;
  max: number;
  suffix?: string;
}) {
  return (
    <div className="inline-flex h-8 items-center rounded-[8px] border border-line bg-surface">
      <button
        className="grid h-full w-7 place-items-center text-ink-3 transition-colors hover:text-ink"
        onClick={() => onChange(Math.max(min, value - 1))}
      >
        −
      </button>
      <span className="num min-w-[34px] text-center text-[12px]">
        {value}
        {suffix && <span className="ml-0.5 text-[10px] text-ink-3">{suffix}</span>}
      </span>
      <button
        className="grid h-full w-7 place-items-center text-ink-3 transition-colors hover:text-ink"
        onClick={() => onChange(Math.min(max, value + 1))}
      >
        +
      </button>
    </div>
  );
}
