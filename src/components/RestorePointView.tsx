import { Button, MessageBar, MessageBarActions, MessageBarBody, MessageBarTitle } from '@fluentui/react-components';
import { t } from '../i18n';
import type { State } from '../state/machine';
import { Busy } from './Busy';

export function RestorePointView({ state, onContinue, onAbort }: { state: State; onContinue: () => void; onAbort: () => void }) {
  if (state.restore?.status === 'failed') {
    return (
      <MessageBar intent="warning" className="wfu-canhbao" layout="multiline">
        <MessageBarBody>
          <MessageBarTitle>{t('restore.failedTitle')}</MessageBarTitle>
          {t('restore.failedBody', { message: state.restore.message })}
        </MessageBarBody>
        <MessageBarActions>
          <Button appearance="primary" onClick={onContinue}>
            {t('restore.continue')}
          </Button>
          <Button onClick={onAbort}>{t('restore.abort')}</Button>
        </MessageBarActions>
      </MessageBar>
    );
  }
  return (
    <section className="wfu-hero">
      <Busy size="medium" label={t('restore.creating')} />
    </section>
  );
}
