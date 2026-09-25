import { Button, Title3 } from '@fluentui/react-components';
import { groupName } from '../catalog';
import { formatBytes } from '../format';
import { t } from '../i18n';
import type { State } from '../state/machine';
import { Busy } from './Busy';

export function ScanningView({ state, onCancel }: { state: State; onCancel: () => void }) {
  return (
    <section className="wfu-hero">
      <Title3>{t('scanning.title')}</Title3>
      <ul className="wfu-groups">
        {state.groups.map((g) => (
          <li key={g.id} className="wfu-group">
            <span>{g.status === 'pending' ? <Busy /> : g.status === 'error' ? '!' : '✓'}</span>
            <div className="wfu-group-main">
              <span className="wfu-group-title">{groupName(g.id)}</span>
            </div>
            <span className="wfu-group-size">
              {g.status === 'pending'
                ? t('scanning.pending')
                : g.status === 'error'
                  ? t('scanning.failed')
                  : formatBytes(g.result?.total_bytes ?? 0)}
            </span>
          </li>
        ))}
      </ul>
      <div className="wfu-actions">
        {state.cancelling ? <Busy label={t('scanning.cancelling')} /> : <Button onClick={onCancel}>{t('scanning.cancel')}</Button>}
      </div>
    </section>
  );
}
