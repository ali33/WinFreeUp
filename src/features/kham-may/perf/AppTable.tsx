import { Fragment, useState } from 'react';
import { Button } from '@fluentui/react-components';
import type { AppRow } from '../api/types';
import { formatBytes, formatKBps, formatMBps, formatPct } from '../fmt';
import { tk } from '../i18n';
import { netTotal, sortApps, type AppSortKey } from './perfModel';

const COLUMNS: { key: AppSortKey; label: string; numeric: boolean }[] = [
  { key: 'name', label: 'km.perf.col.name', numeric: false },
  { key: 'ram', label: 'km.perf.col.ram', numeric: true },
  { key: 'cpu', label: 'km.perf.col.cpu', numeric: true },
  { key: 'disk', label: 'km.perf.col.disk', numeric: true },
  { key: 'net', label: 'km.perf.col.net', numeric: true },
];

function net(v: { net_up_bps: number | null; net_down_bps: number | null }): string {
  const n = netTotal(v);
  return n === null ? '–' : formatKBps(n);
}

export function AppTable({
  apps,
  icons,
  busy,
  onKill,
  onReveal,
}: {
  apps: AppRow[];
  icons: Record<string, string | null>;
  /** Đang kết thúc một app ⇒ khóa mọi nút Kết thúc khác. */
  busy: boolean;
  onKill: (a: AppRow) => void;
  onReveal: (path: string) => void;
}) {
  const [sort, setSort] = useState<{ key: AppSortKey; desc: boolean }>({ key: 'ram', desc: true });
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const rows = sortApps(apps, sort.key, sort.desc);

  function by(key: AppSortKey) {
    setSort((s) => (s.key === key ? { key, desc: !s.desc } : { key, desc: key !== 'name' }));
  }

  return (
    <table className="km-apps">
      <thead>
        <tr>
          {COLUMNS.map((c) => (
            <th
              key={c.key}
              className={c.numeric ? 'km-num' : undefined}
              aria-sort={sort.key === c.key ? (sort.desc ? 'descending' : 'ascending') : 'none'}
            >
              <Button appearance="transparent" size="small" onClick={() => by(c.key)}>
                {tk(c.label)}
                {sort.key === c.key ? (sort.desc ? ' ▼' : ' ▲') : ''}
              </Button>
            </th>
          ))}
          <th />
        </tr>
      </thead>
      <tbody>
        {rows.map((a) => (
          <Fragment key={a.key}>
            <tr>
              <td>
                <span className="km-tree-name">
                  <Button
                    size="small"
                    appearance="transparent"
                    aria-label={tk(open[a.key] ? 'km.perf.collapse' : 'km.perf.expand', { name: a.name })}
                    onClick={() => setOpen((o) => ({ ...o, [a.key]: !o[a.key] }))}
                  >
                    {open[a.key] ? '▾' : '▸'}
                  </Button>
                  {a.path && icons[a.path] ? <img className="km-app-icon" src={icons[a.path] ?? undefined} alt="" /> : <span className="km-app-icon" />}
                  <span className="km-tree-label" title={a.path ?? a.name}>
                    {a.name}
                  </span>
                  {a.procs.length > 1 && <span className="km-muted">({a.procs.length})</span>}
                </span>
              </td>
              <td className="km-num">{formatBytes(a.ram)}</td>
              <td className="km-num">{formatPct(a.cpu)}</td>
              <td className="km-num">{formatMBps(a.disk_bps)}</td>
              <td className="km-num">{net(a)}</td>
              <td>
                <span className="km-row-actions">
                  <Button
                    size="small"
                    appearance="subtle"
                    disabled={a.essential || busy}
                    title={a.essential ? tk('km.perf.essential') : undefined}
                    onClick={() => onKill(a)}
                  >
                    {tk('km.perf.kill')}
                  </Button>
                  {a.path && (
                    <Button size="small" appearance="subtle" onClick={() => onReveal(a.path as string)}>
                      {tk('km.perf.reveal')}
                    </Button>
                  )}
                </span>
              </td>
            </tr>
            {open[a.key] &&
              a.procs.map((p) => (
                <tr key={`${a.key}-${p.pid}-${p.create_time}`} className="km-proc">
                  <td className="km-muted">
                    {p.name} · PID {p.pid}
                    {p.essential ? ' 🔒' : ''}
                  </td>
                  <td className="km-num km-muted">{formatBytes(p.ram)}</td>
                  <td className="km-num km-muted">{formatPct(p.cpu)}</td>
                  <td className="km-num km-muted">{formatMBps(p.disk_bps)}</td>
                  <td className="km-num km-muted">{net(p)}</td>
                  <td />
                </tr>
              ))}
          </Fragment>
        ))}
      </tbody>
    </table>
  );
}
