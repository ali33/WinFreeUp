import { Button, MessageBar, MessageBarActions, MessageBarBody } from '@fluentui/react-components';
import type { Notice } from '../errors/errors';
import { t } from '../i18n';

export function NoticeBar({ notices, onDismiss }: { notices: Notice[]; onDismiss: (id: number) => void }) {
  if (notices.length === 0) return null;
  return (
    <div className="wfu-notices" aria-live="polite">
      {notices.map((n) => (
        <MessageBar key={n.id} intent={n.severity === 'error' ? 'error' : 'warning'} className={n.severity === 'error' ? 'wfu-loi' : 'wfu-canhbao'}>
          <MessageBarBody>
            {n.message}
            {n.count > 1 ? ` ${t('notice.repeat', { count: n.count })}` : ''}
          </MessageBarBody>
          <MessageBarActions
            containerAction={
              <Button appearance="transparent" size="small" aria-label={t('notice.dismiss')} onClick={() => onDismiss(n.id)}>
                ×
              </Button>
            }
          />
        </MessageBar>
      ))}
    </div>
  );
}
