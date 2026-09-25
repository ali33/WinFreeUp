import { Badge } from '@fluentui/react-components';
import type { StutterView, TopApp } from '../api/types';
import { formatClock, formatMBps, formatPct } from '../fmt';
import { tk } from '../i18n';

function top(apps: TopApp[], cpu: boolean): string {
  return apps.map((a) => `${a.name} ${cpu ? formatPct(a.avg) : `${formatMBps(a.avg)} MB/s`}`).join(', ');
}

/** Spec 5.3: mỗi cơn — giờ bắt đầu, thời lượng, chỉ số vượt, 3 app ngốn nhất, cờ trùng lúc hạ xung. Mới nhất lên đầu. */
export function StutterList({ stutters }: { stutters: StutterView[] }) {
  if (stutters.length === 0) return <p className="km-muted">{tk('km.perf.noStutters')}</p>;
  return (
    <ul className="km-stutters">
      {[...stutters].reverse().map((s) => {
        const metrics = [s.cpu && tk('km.perf.stutterCpu'), s.disk && tk('km.perf.stutterDisk')].filter(Boolean).join(' + ');
        return (
          <li key={s.start_ms}>
            <strong>
              {tk('km.perf.stutterLine', { time: formatClock(s.start_ms), seconds: Math.round((s.end_ms - s.start_ms) / 1000), metrics })}
            </strong>{' '}
            {s.throttled && (
              <Badge appearance="tint" color="warning">
                {tk('km.perf.stutterThrottled')}
              </Badge>
            )}
            {s.top_cpu.length > 0 && <div className="km-muted">{tk('km.perf.stutterTop', { apps: top(s.top_cpu, true) })}</div>}
            {s.top_disk.length > 0 && <div className="km-muted">{tk('km.perf.stutterTop', { apps: top(s.top_disk, false) })}</div>}
          </li>
        );
      })}
    </ul>
  );
}
