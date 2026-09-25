import { useCallback, useEffect, useRef, useState } from 'react';
import { Button, Dialog, DialogActions, DialogBody, DialogContent, DialogSurface, DialogTitle } from '@fluentui/react-components';
import type { Finding, FindingId, KhamMayApi, Notify } from '../api/types';
import { friendly } from '../fmt';
import { tk } from '../i18n';
import { Spin } from '../ui/Spin';
import { actionsFor, countProblems, FINDING_ORDER, findingSentence, levelLabel, type FindingAction } from './findingText';
import { StartupList } from './StartupList';

export type NavTarget = 'clean' | 'disk' | 'perf';
type Guide = 'backup' | 'ssd' | 'restart' | 'cooling';

export function OverviewView({ api, notify, onNavigate }: { api: KhamMayApi; notify: Notify; onNavigate: (t: NavTarget) => void }) {
  const [findings, setFindings] = useState<Partial<Record<FindingId, Finding>>>({});
  const [checking, setChecking] = useState(false);
  const [throttling, setThrottling] = useState(false);
  const [guide, setGuide] = useState<Guide | null>(null);
  const startupRef = useRef<HTMLDivElement>(null);
  const alive = useRef(true);

  const put = useCallback((f: Finding) => {
    if (alive.current) setFindings((m) => ({ ...m, [f.id]: f }));
  }, []);

  const run = useCallback(async () => {
    setFindings({});
    setChecking(true);
    setThrottling(true);
    const throttle = api
      .healthThrottle()
      .then(put)
      .catch((e) => put({ id: 'cpu_throttle', level: 'unknown', value: null, detail: friendly(e) }))
      .finally(() => alive.current && setThrottling(false));
    try {
      const all = await api.healthCheck(put);
      all.forEach(put);
    } catch (e) {
      const message = friendly(e);
      notify('error', tk('km.overview.failed', { message }));
      // Dòng chưa có kết quả thì ghi rõ «Không đo được» kèm lý do, không để vòng quay chạy mãi.
      if (alive.current)
        setFindings((m) => {
          const next = { ...m };
          for (const id of FINDING_ORDER)
            if (id !== 'cpu_throttle' && !next[id]) next[id] = { id, level: 'unknown', value: null, detail: message };
          return next;
        });
    } finally {
      if (alive.current) setChecking(false);
    }
    await throttle;
  }, [api, notify, put]);

  // StrictMode chạy effect hai lần: chỉ khám một lần mỗi lần mở mục, nếu không lõi sẽ trả «busy».
  const started = useRef(false);
  useEffect(() => {
    alive.current = true;
    if (!started.current) {
      started.current = true;
      void run();
    }
    return () => {
      alive.current = false;
    };
  }, [run]);

  async function act(a: FindingAction) {
    switch (a) {
      case 'clean':
      case 'disk':
      case 'perf':
        onNavigate(a);
        return;
      case 'startup':
        startupRef.current?.scrollIntoView?.({ behavior: 'smooth', block: 'start' });
        return;
      case 'power':
        try {
          await api.openSettings('power');
        } catch (e) {
          notify('warning', tk('km.perf.settingsFailed', { message: friendly(e) }));
        }
        return;
      default:
        setGuide(a);
    }
  }

  const done = FINDING_ORDER.map((id) => findings[id]).filter((f): f is Finding => !!f);
  const { critical, warn } = countProblems(done);
  const finished = !checking && !throttling;

  return (
    <section className="km-root" aria-label={tk('km.tab.overview')}>
      <div className="km-toolbar">
        <h2 style={{ margin: 0 }}>{tk('km.overview.title')}</h2>
        <Button appearance="primary" disabled={!finished} onClick={() => void run()}>
          {tk('km.overview.recheck')}
        </Button>
        {!finished && <Spin label={tk('km.overview.checking')} />}
      </div>
      {finished && (
        <p aria-live="polite">{critical + warn === 0 ? tk('km.overview.allGood') : tk('km.overview.summary', { critical, warn })}</p>
      )}
      <ul className="km-findings">
        {FINDING_ORDER.map((id) => {
          const f = findings[id];
          const waiting = id === 'cpu_throttle' ? throttling : checking;
          return (
            <li key={id} className="km-finding" data-finding={id}>
              {f ? <strong>{levelLabel(f.level)}</strong> : waiting ? <Spin /> : <strong>{levelLabel('unknown')}</strong>}
              <span>
                {f
                  ? findingSentence(f)
                  : !waiting
                    ? ''
                    : id === 'cpu_throttle'
                      ? tk('km.finding.cpu_throttle.pending')
                      : tk('km.overview.checking')}
              </span>
              <span className="km-finding-actions">
                {f &&
                  actionsFor(f).map((a) => (
                    <Button key={a} size="small" onClick={() => void act(a)}>
                      {tk(`km.action.${a}`)}
                    </Button>
                  ))}
              </span>
            </li>
          );
        })}
      </ul>
      <div ref={startupRef}>
        <StartupList api={api} notify={notify} />
      </div>
      <Dialog open={guide !== null} onOpenChange={(_, d) => !d.open && setGuide(null)}>
        <DialogSurface>
          <DialogBody>
            <DialogTitle>{guide && tk(`km.guide.${guide}.title`)}</DialogTitle>
            <DialogContent>{guide && tk(`km.guide.${guide}.body`)}</DialogContent>
            <DialogActions>
              <Button onClick={() => setGuide(null)}>{tk('km.guide.close')}</Button>
            </DialogActions>
          </DialogBody>
        </DialogSurface>
      </Dialog>
    </section>
  );
}
