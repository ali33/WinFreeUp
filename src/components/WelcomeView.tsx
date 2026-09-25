import { Body1, Button, Title3 } from '@fluentui/react-components';
import { formatBytes } from '../format';
import { t } from '../i18n';
import type { State } from '../state/machine';
import { Busy } from './Busy';

export function WelcomeView({ state, onScan }: { state: State; onScan: () => void }) {
  const drive = state.systemDrive.replace(/\\$/, '');
  return (
    <section className="wfu-hero">
      <Body1>{t('welcome.intro')}</Body1>
      <div>
        {state.freeBefore !== null ? (
          <Title3>{t('welcome.freeSpace', { drive, size: formatBytes(state.freeBefore) })}</Title3>
        ) : state.freeFailed ? (
          <Body1 className="wfu-muted">{t('welcome.freeUnknown')}</Body1>
        ) : (
          <Busy label={t('welcome.loadingFree')} />
        )}
      </div>
      <div className="wfu-actions">
        <Button appearance="primary" size="large" onClick={onScan}>
          {t('welcome.scan')}
        </Button>
      </div>
    </section>
  );
}
