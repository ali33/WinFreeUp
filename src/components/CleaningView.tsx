import { ProgressBar, Title3 } from '@fluentui/react-components';
import { groupName } from '../catalog';
import { formatBytes } from '../format';
import { t } from '../i18n';
import type { CleanRow, State } from '../state/machine';
import { Busy } from './Busy';

function rowLabel(r: CleanRow): string {
  if (r.status === 'waiting') return t('cleaning.waiting');
  if (r.status === 'running') return r.percent !== null ? t('cleaning.percent', { percent: Math.round(r.percent) }) : t('cleaning.running');
  if (r.result?.error || !r.result?.report) return t('cleaning.failed');
  return formatBytes(r.result.report.bytes_freed);
}

export function CleaningView({ state }: { state: State }) {
  return (
    <section className="wfu-hero">
      <div className="wfu-header">
        <Title3>{t('cleaning.title')}</Title3>
        <Busy />
      </div>
      <ul className="wfu-groups">
        {state.cleaning.map((r) => (
          <li key={r.id} className="wfu-group">
            <span>{r.status === 'running' ? <Busy /> : r.status === 'done' ? (r.result?.error ? '!' : '✓') : ''}</span>
            <div className="wfu-group-main">
              <span className="wfu-group-title">{groupName(r.id)}</span>
              {r.status === 'running' && r.percent !== null && (
                <ProgressBar value={Math.min(1, r.percent / 100)} thickness="large" aria-label={groupName(r.id)} />
              )}
            </div>
            <span className="wfu-group-size">{rowLabel(r)}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
