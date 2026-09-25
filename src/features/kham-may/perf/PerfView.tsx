import { useEffect, useRef, useState } from 'react';
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle } from '@fluentui/react-components';
import type { AppRow, KhamMayApi, Notify, PerfTick, Sample } from '../api/types';
import { formatBytes, formatPct, formatRate, friendly } from '../fmt';
import { tk } from '../i18n';
import { Spin } from '../ui/Spin';
import { AppTable } from './AppTable';
import { LineChart } from './LineChart';
import { niceMax, pushSample, segments } from './perfModel';
import { StutterList } from './StutterList';

/** Mục Bộ nhớ & Hiệu năng. Chỉ lấy mẫu khi đang hiển thị: gắn vào ⇒ `perfStart`, gỡ ra ⇒ `perfStop` (spec 5). */
export function PerfView({ api, notify }: { api: KhamMayApi; notify: Notify }) {
  const [samples, setSamples] = useState<Sample[]>([]);
  const [last, setLast] = useState<PerfTick | null>(null);
  const [icons, setIcons] = useState<Record<string, string | null>>({});
  const [confirm, setConfirm] = useState<AppRow | null>(null);
  const [killing, setKilling] = useState(false);
  const [status, setStatus] = useState('');
  const warned = useRef<Set<string>>(new Set());
  const requested = useRef<Set<string>>(new Set());
  const alive = useRef(true);

  function warnOnce(key: string, message: string) {
    if (warned.current.has(key)) return;
    warned.current.add(key);
    notify('warning', message);
  }

  useEffect(() => {
    alive.current = true;
    // Cờ riêng của lần gắn này: StrictMode gắn → gỡ → gắn lại, mẫu muộn của lần đầu không được lọt vào.
    let active = true;
    const onTick = (t: PerfTick) => {
      if (!active) return;
      setLast(t);
      setSamples((s) => pushSample(s, t.sample));
      if (t.net_app_error) warnOnce('net', tk('km.perf.netAppMissing', { message: t.net_app_error }));
      if (t.disk_error) warnOnce('disk', tk('km.perf.diskMissing', { message: t.disk_error }));
    };
    const onError = (m: string) => active && notify('warning', tk('km.perf.sampleFailed', { message: m }));
    api
      .perfStart(onTick, onError)
      .then((history) => active && setSamples((s) => history.reduce(pushSample, s)))
      .catch((e) => active && notify('error', tk('km.perf.startFailed', { message: friendly(e) })));
    return () => {
      active = false;
      alive.current = false;
      api.perfStop().catch((e) => notify('warning', friendly(e)));
    };
  }, [api, notify]);

  // Biểu tượng app: xin một lần cho mỗi đường dẫn, lõi nhớ đệm.
  useEffect(() => {
    for (const a of last?.apps ?? []) {
      const p = a.path;
      if (!p || requested.current.has(p)) continue;
      requested.current.add(p);
      api
        .appIcon(p)
        .then((url) => alive.current && setIcons((m) => ({ ...m, [p]: url })))
        .catch((e) => {
          if (!alive.current) return;
          setIcons((m) => ({ ...m, [p]: null }));
          // Thiếu biểu tượng không chặn việc đọc bảng ⇒ hổ phách, báo một lần.
          warnOnce('icon', friendly(e));
        });
    }
  }, [api, last]);

  async function kill() {
    const a = confirm;
    if (!a) return;
    setKilling(true);
    try {
      const r = await api.appKill(a.key);
      if (r.errors.length > 0) notify('warning', tk('km.perf.killPartial', { name: a.name, count: r.errors.length, first: r.errors[0] }));
      else setStatus(tk('km.perf.killed', { name: a.name }));
      if (r.log_error) notify('warning', tk('km.err.logWrite', { message: r.log_error }));
    } catch (e) {
      notify('error', tk('km.perf.killFailed', { name: a.name, message: friendly(e) }));
    } finally {
      setKilling(false);
      setConfirm(null);
    }
  }

  async function reveal(path: string) {
    try {
      await api.revealPath(path);
    } catch (e) {
      notify('warning', tk('km.perf.revealFailed', { message: friendly(e) }));
    }
  }

  if (!last) return <Spin label={tk('km.perf.starting')} />;

  const s = last.sample;
  const now = s.t_ms;
  const bands = last.stutters;
  const netMax = niceMax(samples.flatMap((x) => [x.net_up_bps ?? 0, x.net_down_bps ?? 0]));

  return (
    <section className="km-root" aria-label={tk('km.tab.perf')}>
      <div className="km-charts">
        <LineChart title={tk('km.perf.chart.cpu')} valueText={formatPct(s.cpu)} max={100} now={now} bands={bands} lines={[{ segments: segments(samples, (x) => x.cpu) }]} />
        <LineChart
          title={tk('km.perf.chart.ram')}
          valueText={tk('km.perf.ramValue', { pct: Math.round(s.ram_pct), used: formatBytes(s.ram_used), total: formatBytes(s.ram_total) })}
          max={100}
          now={now}
          bands={bands}
          lines={[{ segments: segments(samples, (x) => x.ram_pct) }]}
        />
        <LineChart
          title={tk('km.perf.chart.disk')}
          valueText={s.disk_active === null ? tk('km.perf.noValue') : formatPct(s.disk_active)}
          max={100}
          now={now}
          bands={bands}
          lines={[{ segments: segments(samples, (x) => x.disk_active) }]}
        />
        <LineChart
          title={tk('km.perf.chart.net')}
          valueText={
            s.net_up_bps === null || s.net_down_bps === null
              ? tk('km.perf.noValue')
              : `↑ ${formatRate(s.net_up_bps)} · ↓ ${formatRate(s.net_down_bps)}`
          }
          max={netMax}
          now={now}
          bands={bands}
          lines={[
            { segments: segments(samples, (x) => x.net_down_bps), label: tk('km.perf.netDown') },
            { segments: segments(samples, (x) => x.net_up_bps), alt: true, label: tk('km.perf.netUp') },
          ]}
        />
      </div>

      <div className="km-toolbar">
        {last.throttled === true && <strong>🟠 {tk('km.perf.throttled')}</strong>}
        {last.throttled === false && <span className="km-muted">{tk('km.perf.notThrottled')}</span>}
        {last.throttled === null && last.perf_error && <span className="km-muted">{tk('km.perf.throttleMissing', { message: last.perf_error })}</span>}
        <span className={last.temp.state === 'ok' ? undefined : 'km-muted'} title={last.temp.state === 'ok' ? undefined : last.temp.detail}>
          {last.temp.state === 'ok'
            ? tk('km.perf.temp', { celsius: Math.round(last.temp.celsius) })
            : tk('km.perf.tempMissing')}
        </span>
        {last.interval_ms > 1000 && <span className="km-muted">{tk('km.perf.slow')}</span>}
      </div>

      <h3>{tk('km.perf.stutters')}</h3>
      <StutterList stutters={last.stutters} />

      <h3>{tk('km.perf.apps')}</h3>
      <p className="km-muted" aria-live="polite">
        {status}
      </p>
      <AppTable apps={last.apps} icons={icons} busy={killing} onKill={setConfirm} onReveal={(p) => void reveal(p)} />

      <Dialog open={confirm !== null} onOpenChange={(_, d) => !d.open && !killing && setConfirm(null)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{confirm && tk('km.perf.killTitle', { name: confirm.name })}</DialogTitle>
            <DialogContent>{tk('km.perf.killBody')}</DialogContent>
            <DialogActions>
              {killing ? (
                <Spin label={tk('km.perf.killing')} />
              ) : (
                <>
                  <Button appearance="primary" onClick={() => void kill()}>
                    {tk('km.perf.killConfirm')}
                  </Button>
                  <Button onClick={() => setConfirm(null)}>{tk('km.perf.killCancel')}</Button>
                </>
              )}
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </section>
  );
}
