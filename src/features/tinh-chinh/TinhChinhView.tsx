import { useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import { Button, Checkbox, MessageBar, MessageBarBody } from '@fluentui/react-components';
import { Busy } from '../../components/Busy';
import { createStore } from '../../state/store';
import * as c from './controller';
import { ConfirmTweaks, RestorePrompt } from './Dialogs';
import { currentPreset, LEVELS, pendingChanges, revertable } from './presets';
import { initialTState, reducer } from './reducer';
import { RunPanel } from './RunPanel';
import { tt } from './strings';
import { TweakList } from './TweakList';
import type { TweakApi } from './types';
import './tinh-chinh.css';

export interface TinhChinhProps {
  api: TweakApi;
  notify: c.Notify;
  /** Cờ `--dry-run` của app: chỉ xem, không cho áp dụng/hoàn tác. */
  dryRun: boolean;
  /** Báo cho App khoá tab khác khi đang xác nhận / tạo điểm khôi phục / chạy. */
  onBusyChange?: (busy: boolean) => void;
}

export function TinhChinhView({ api, notify, dryRun, onBusyChange }: TinhChinhProps) {
  const [store] = useState(() => createStore(reducer, initialTState));
  const s = useSyncExternalStore(store.subscribe, store.getState);
  const d = useMemo(() => c.createTDeps(api, store, notify), [api, store, notify]);
  useEffect(() => {
    void c.load(d);
  }, [d]);
  const working = s.phase === 'confirm' || s.phase === 'restorePoint' || s.phase === 'running';
  useEffect(() => {
    onBusyChange?.(working);
  }, [working, onBusyChange]);
  const run = (p: Promise<void>) => {
    p.catch((e) => notify('error', c.friendlyT(e)));
  };

  if (!s.data) {
    if (!s.loadError) return <Busy label={tt('tweaks.loading')} size="medium" />;
    // Băng đỏ đã lên qua notify (có thể bị người dùng đóng); vẫn ghi lỗi nguyên văn tại chỗ để màn không trống.
    return (
      <div className="tc-root">
        <div className="tc-row-error">{tt('tweaks.errors.loadFailed', { message: s.loadError })}</div>
        <div className="wfu-actions">
          <Button onClick={() => run(c.load(d))}>{tt('tweaks.retry')}</Button>
        </div>
      </div>
    );
  }

  const tweaks = s.data.tweaks;
  const busy = s.phase !== 'ready' || s.reloading;
  const locked = busy || dryRun;
  const preset = currentPreset(tweaks, s.selected);
  const toApply = pendingChanges(tweaks, s.selected).length;
  const toRevert = revertable(tweaks, s.selected).length;

  return (
    <div className="tc-root">
      {dryRun && (
        <MessageBar intent="info">
          <MessageBarBody>{tt('tweaks.dryRun')}</MessageBarBody>
        </MessageBar>
      )}
      {s.data.system.other_user && (
        <MessageBar intent="warning" className="wfu-canhbao">
          <MessageBarBody>{tt('tweaks.otherUser')}</MessageBarBody>
        </MessageBar>
      )}
      {s.data.system.managed && (
        <MessageBar intent="info">
          <MessageBarBody>{tt('tweaks.managedMachine')}</MessageBarBody>
        </MessageBar>
      )}

      <div className="tc-presets" role="group" aria-label={tt('tweaks.preset.label')}>
        {LEVELS.map((l) => (
          <Button key={l} appearance={preset === l ? 'primary' : 'secondary'} aria-pressed={preset === l} disabled={busy} onClick={() => c.preset(d, l)}>
            {tt(`tweaks.preset.${l}`)}
          </Button>
        ))}
        {preset === 'custom' && <span className="tc-custom">{tt('tweaks.preset.custom')}</span>}
      </div>

      <RestorePrompt state={s} onContinue={() => run(c.continueAfterRestoreFailure(d))} onAbort={() => c.abortAfterRestoreFailure(d)} />
      <RunPanel state={s} onRestartExplorer={() => c.restartExplorer(d)} onClose={() => c.dismissResult(d)} />

      <div className={s.reloading ? 'tc-list tc-dim' : 'tc-list'} aria-busy={s.reloading}>
        {s.reloading && (
          <div className="tc-overlay">
            <Busy label={tt('tweaks.reloading')} size="small" />
          </div>
        )}
        <TweakList tweaks={tweaks} selected={s.selected} locked={busy} onToggle={(id) => c.toggle(d, id)} onReinstall={(id) => run(c.reinstall(d, id))} />
      </div>

      <div className="tc-footer">
        <Checkbox
          checked={s.allUsers}
          disabled={locked}
          onChange={(_, v) => c.setAllUsers(d, v.checked === true)}
          label={
            <>
              {tt('tweaks.allUsers')} <span className="tc-warn">⚠ {tt('tweaks.allUsersWarn')}</span>
            </>
          }
        />
        <div className="wfu-actions">
          <Button disabled={locked || toRevert === 0} onClick={() => run(c.revertSelected(d))}>
            {tt('tweaks.revert', { count: toRevert })}
          </Button>
          <Button appearance="primary" disabled={locked || toApply === 0} onClick={() => run(c.requestApply(d))}>
            {tt('tweaks.apply', { count: toApply })}
          </Button>
        </div>
      </div>

      {s.phase === 'confirm' && <ConfirmTweaks state={s} onAccept={() => run(c.acceptConfirm(d))} onCancel={() => c.cancelConfirm(d)} />}
    </div>
  );
}
