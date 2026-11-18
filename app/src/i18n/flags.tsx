import { Globe } from 'lucide-react';
import type { Locale } from './index';

/** 自绘小国旗：Windows 的 emoji 字体不渲染国旗区域指示符，SVG 更可靠 */
export function Flag({ locale, size = 18 }: { locale: Locale; size?: number }) {
  const common = {
    width: size,
    height: Math.round(size * 0.7),
    viewBox: '0 0 20 14',
    style: { borderRadius: 2, display: 'block' as const },
    'aria-hidden': true as const,
  };

  if (locale === 'zh') {
    return (
      <svg {...common}>
        <rect width="20" height="14" rx="2" fill="#ee1c25" />
        <polygon
          points="4,1.4 4.61,3.16 6.47,3.2 4.99,4.32 5.53,6.1 4,5.04 2.47,6.1 3.01,4.32 1.53,3.2 3.39,3.16"
          fill="#ffde00"
        />
        {[
          [7.4, 1.7],
          [8.7, 2.9],
          [8.7, 4.5],
          [7.4, 5.7],
        ].map(([cx, cy]) => (
          <circle key={`${cx}-${cy}`} cx={cx} cy={cy} r="0.42" fill="#ffde00" />
        ))}
      </svg>
    );
  }

  if (locale === 'en') {
    return (
      <svg {...common}>
        <rect width="20" height="14" rx="2" fill="#fff" />
        {[0, 2, 4, 6, 8, 10, 12].map((i) => (
          <rect key={i} x="0" y={i * (14 / 13)} width="20" height={14 / 13} fill="#b22234" />
        ))}
        <rect x="0" y="0" width="9" height={7 * (14 / 13)} fill="#3c3b6e" />
        {[1.5, 4, 6.5].flatMap((x) =>
          [1.4, 3.2, 5, 6.8].map((y) => (
            <circle key={`${x}-${y}`} cx={x} cy={y} r="0.42" fill="#fff" />
          )),
        )}
      </svg>
    );
  }

  return <Globe size={size} />;
}
