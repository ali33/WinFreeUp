import {
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  MessageBar,
  MessageBarActions,
  MessageBarBody,
  MessageBarTitle,
} from '@fluentui/react-components';
import { Busy } from '../../components/Busy';
import { itemName } from './labels';
import { needsConfirm, pendingChanges } from './presets';
import type { TState } from './reducer';
import { tt } from './strings';

/** Chỉ vẽ khi `phase === 'confirm'`. Liệt kê mục caution, và nếu bật «mọi tài khoản» thì cả các app sẽ gỡ. */
export function ConfirmTweaks({ state, onAccept, onCancel }: { state: TState; onAccept: () => void; onCancel: () => void }) {
  const tweaks = state.data?.tweaks ?? [];
  const list = needsConfirm(tweaks, pendingChanges(tweaks, state.selected), state.allUsers);
  const caution = list.filter((t) => t.risk === 'caution');
  const apps = state.allUsers ? list.filter((t) => t.group === 'bloatware') : [];
  return (
    <Dialog open modalType="alert" onOpenChange={(_, d) => !d.open && onCancel()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{tt('tweaks.confirm.title')}</DialogTitle>
          <DialogContent>
            {caution.length > 0 && (
              <>
                <p>{tt('tweaks.confirm.caution')}</p>
                <ul>
                  {caution.map((t) => (
                    <li key={t.id}>{itemName(t.id)}</li>
                  ))}
                </ul>
              </>
            )}
            {apps.length > 0 && (
              <>
                <p className="tc-warn">⚠ {tt('tweaks.confirm.allUsers')}</p>
                <ul>
                  {apps.map((t) => (
                    <li key={t.id}>{itemName(t.id)}</li>
                  ))}
                </ul>
              </>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onCancel}>
              {tt('tweaks.confirm.cancel')}
            </Button>
            <Button appearance="primary" onClick={onAccept}>
              {tt('tweaks.confirm.accept')}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}

/** Đang tạo điểm khôi phục ⇒ vòng quay; hỏng ⇒ băng hổ phách, người dùng chọn tiếp/dừng. */
export function RestorePrompt({ state, onContinue, onAbort }: { state: TState; onContinue: () => void; onAbort: () => void }) {
  if (state.phase !== 'restorePoint') return null;
  if (state.restore?.status !== 'failed') return <Busy label={tt('tweaks.restore.creating')} size="small" />;
  return (
    <MessageBar intent="warning" className="wfu-canhbao">
      <MessageBarBody>
        <MessageBarTitle>{tt('tweaks.restore.failedTitle')}</MessageBarTitle>
        {tt('tweaks.restore.failedBody', { message: state.restore.message })}
      </MessageBarBody>
      <MessageBarActions>
        <Button onClick={onAbort}>{tt('tweaks.restore.abort')}</Button>
        <Button appearance="primary" onClick={onContinue}>
          {tt('tweaks.restore.continue')}
        </Button>
      </MessageBarActions>
    </MessageBar>
  );
}
