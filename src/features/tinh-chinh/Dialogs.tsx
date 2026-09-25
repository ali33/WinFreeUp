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
import { useEffect, useRef } from 'react';
import { itemName } from './labels';
import { needsConfirm, pendingChanges } from './presets';
import type { TState } from './reducer';
import { tt } from './strings';

/** Chỉ vẽ khi `phase === 'confirm'`. Liệt kê mục caution, và nếu bật «mọi tài khoản» thì cả các app sẽ gỡ. */
export function ConfirmTweaks({ state, onAccept, onCancel }: { state: TState; onAccept: () => void; onCancel: () => void }) {
  const tweaks = state.data?.tweaks ?? [];
  const list = needsConfirm(tweaks, pendingChanges(tweaks, state.selected), state.allUsers);
  const caution = list.filter((t) => t.risk === 'caution');
  // App đã có trong danh sách Cân nhắc thì không lặp lại ở danh sách app.
  const apps = state.allUsers ? list.filter((t) => t.group === 'bloatware' && t.risk !== 'caution') : [];
  const allUsersApps = state.allUsers && list.some((t) => t.group === 'bloatware');
  return (
    <Dialog open modalType="alert" onOpenChange={(_, d) => !d.open && onCancel()}>
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{tt('tweaks.confirm.title')}</DialogTitle>
          <DialogContent>
            {state.data?.system.other_user && <p className="tc-warn">⚠ {tt('tweaks.otherUser')}</p>}
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
            {allUsersApps && (
              <>
                <p className="tc-warn">⚠ {tt('tweaks.confirm.allUsers')}</p>
                {apps.length > 0 && (
                  <ul>
                    {apps.map((t) => (
                      <li key={t.id}>{itemName(t.id)}</li>
                    ))}
                  </ul>
                )}
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

/**
 * Hỏng tạo điểm khôi phục ⇒ băng hổ phách, người dùng chọn tiếp/dừng; cuộn tới và đặt focus vào băng.
 * Vòng quay «đang tạo» nằm ở chân trang (TinhChinhView), cạnh nút, để không trôi khỏi khung nhìn.
 */
export function RestorePrompt({ state, onContinue, onAbort }: { state: TState; onContinue: () => void; onAbort: () => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const failed = state.phase === 'restorePoint' && state.restore?.status === 'failed';
  useEffect(() => {
    if (!failed || !ref.current) return;
    ref.current.scrollIntoView?.({ block: 'nearest' });
    ref.current.focus();
  }, [failed]);
  if (!failed || state.restore?.status !== 'failed') return null;
  return (
    <MessageBar intent="warning" className="wfu-canhbao" ref={ref} tabIndex={-1}>
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
