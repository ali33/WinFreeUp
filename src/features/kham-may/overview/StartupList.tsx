import { useEffect, useRef, useState } from 'react';
import { Switch } from '@fluentui/react-components';
import type { KhamMayApi, Notify, StartupEntry } from '../api/types';
import { friendly } from '../fmt';
import { tk } from '../i18n';
import { Spin } from '../ui/Spin';

/** Danh sách app khởi động với công tắc bật/tắt (ghi StartupApproved như Task Manager, bật lại được). */
export function StartupList({ api, notify, onChanged }: { api: KhamMayApi; notify: Notify; onChanged?: () => void }) {
  const [entries, setEntries] = useState<StartupEntry[] | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [pending, setPending] = useState<Record<string, boolean>>({});
  const alive = useRef(true);

  useEffect(() => {
    // Cờ riêng từng lần chạy effect: StrictMode chạy effect hai lần, lần đầu bị bỏ thì không được báo lỗi trùng.
    let live = true;
    alive.current = true;
    api
      .startupList()
      .then((l) => {
        if (!live) return;
        setEntries(l.entries);
        setFailed(null);
        for (const e of l.errors) notify('warning', tk('km.startup.failed', { message: e }));
      })
      .catch((e) => {
        if (!live) return;
        const message = tk('km.startup.failed', { message: friendly(e) });
        setEntries([]);
        setFailed(message);
        notify('error', message);
      });
    return () => {
      live = false;
      alive.current = false;
    };
  }, [api, notify]);

  async function toggle(e: StartupEntry, enabled: boolean) {
    if (pending[e.id]) return;
    setPending((p) => ({ ...p, [e.id]: true }));
    try {
      const updated = await api.startupSet(e.id, enabled);
      if (alive.current) setEntries((list) => (list ?? []).map((x) => (x.id === e.id ? { ...x, enabled: updated.enabled } : x)));
      onChanged?.();
    } catch (err) {
      notify('error', tk('km.startup.setFailed', { name: e.name, message: friendly(err) }));
    } finally {
      if (alive.current) setPending((p) => ({ ...p, [e.id]: false }));
    }
  }

  if (entries === null) return <Spin label={tk('km.startup.loading')} />;
  const enabled = entries.filter((e) => e.enabled).length;
  return (
    <section className="km-root" aria-label={tk('km.startup.title')}>
      <h3>
        {tk('km.startup.title')}{' '}
        {failed === null && <span className="km-muted">({tk('km.startup.count', { enabled, total: entries.length })})</span>}
      </h3>
      {failed !== null ? (
        <p role="alert">{failed}</p>
      ) : entries.length === 0 ? (
        <p className="km-muted">{tk('km.startup.empty')}</p>
      ) : (
        <ul className="km-startup">
          {entries.map((e) => (
            <li key={e.id} className="km-startup-item">
              <Switch
                checked={e.enabled}
                disabled={!!pending[e.id]}
                aria-label={e.name}
                onChange={(_, d) => void toggle(e, d.checked)}
              />
              <span>
                <strong>{e.name}</strong> <span className="km-muted">· {tk(`km.startup.source.${e.source}`)}</span>{' '}
                {pending[e.id] && <Spin size="extra-tiny" />}
              </span>
              <span className="km-muted km-startup-cmd" title={e.command}>
                {e.command}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
