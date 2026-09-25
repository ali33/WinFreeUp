import type { Pt } from './perfModel';
import { WINDOW_MS } from './perfModel';

export interface Band {
  start_ms: number;
  end_ms: number;
}

/**
 * Biểu đồ đường SVG gọn (không thêm thư viện): trục ngang là 5 phút gần nhất tính tới `now`,
 * trục dọc 0..`max`. `bands` là các cơn giật — tô dải đỏ.
 */
export function LineChart({
  title,
  valueText,
  lines,
  max,
  now,
  bands,
}: {
  title: string;
  valueText: string;
  lines: { segments: Pt[][]; alt?: boolean; label?: string }[];
  max: number;
  now: number;
  bands: Band[];
}) {
  const W = 300;
  const H = 100;
  const start = now - WINDOW_MS;
  const x = (t: number) => Math.min(W, Math.max(0, ((t - start) / WINDOW_MS) * W));
  const y = (v: number) => H - Math.min(H, Math.max(0, (v / (max || 1)) * H));
  return (
    <figure className="km-chart" style={{ margin: 0 }}>
      <figcaption className="km-chart-head">
        <strong>{title}</strong>
        <span className="km-num">{valueText}</span>
      </figcaption>
      <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" role="img" aria-label={`${title}: ${valueText}`}>
        {bands
          .filter((b) => b.end_ms > start)
          .map((b) => (
            <rect
              key={`${b.start_ms}-${b.end_ms}`}
              className="km-chart-band"
              data-band=""
              x={x(b.start_ms)}
              y={0}
              width={Math.max(1, x(b.end_ms) - x(b.start_ms))}
              height={H}
            />
          ))}
        {lines.flatMap((l, li) =>
          l.segments.map((seg, si) => (
            <polyline
              key={`${li}-${si}`}
              className={l.alt ? 'km-chart-line km-alt' : 'km-chart-line'}
              points={seg.map((p) => `${x(p.t).toFixed(1)},${y(p.v).toFixed(1)}`).join(' ')}
            >
              {l.label && <title>{l.label}</title>}
            </polyline>
          )),
        )}
      </svg>
    </figure>
  );
}
