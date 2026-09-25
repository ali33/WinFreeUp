import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react';
import { Button, Checkbox, MessageBar, MessageBarBody } from '@fluentui/react-components';
import { Busy } from '../../components/Busy';
import { createStore } from '../../state/store';
import * as c from './controller';
import { ConfirmTweaks, RestorePrompt } from './Dialogs';
import { currentPreset, LEVELS, pendingChanges, revertable } from './presets';
import { initialTState, reducer, type Phase } from './reducer';
import { RunPanel, runLabel, spokenLabel } from './RunPanel';
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

  // Đóng kết quả / dừng sau điểm khôi phục hỏng / lượt chạy hỏng ⇒ focus về nút Áp dụng, không rơi về body.
  // Đọc lại hỏng (chưa có dữ liệu) ⇒ nút Áp dụng không được vẽ ⇒ focus về nút Thử lại thay thế.
  const applyBtn = useRef<HTMLButtonElement>(null);
  const retryBtn = useRef<HTMLButtonElement>(null);
  const prevPhase = useRef<Phase>(s.phase);
  useEffect(() => {
    const from = prevPhase.current;
    prevPhase.current = s.phase;
    if (s.phase === 'ready' && (from === 'done' || from === 'restorePoint' || from === 'running')) {
      if (applyBtn.current) applyBtn.current.focus();
      else retryBtn.current?.focus();
    }
  }, [s.phase]);

  const data = s.data;
  const tweaks = data?.tweaks ?? [];
  const busy = s.phase !== 'ready' || s.reloading;
  const locked = busy || dryRun;
  const preset = currentPreset(tweaks, s.selected);
  const toApply = pendingChanges(tweaks, s.selected).length;
  const toRevert = revertable(tweaks, s.selected).length;
  // Chỉ báo ở chân trang dính, cạnh nút: luôn trong khung nhìn dù người dùng cuộn tới đâu.
  const creating = s.phase === 'restorePoint' && s.restore === null;
  const status = creating ? tt('tweaks.restore.creating') : (runLabel(s) ?? (s.reloading ? tt('tweaks.reloading') : null));
  // Trình đọc màn hình: chỉ báo khi đổi việc / đổi mục, không đọc lại «i/n» ở mỗi sự kiện.
  const spoken = spokenLabel(s);

  // RunPanel luôn là con đầu của .tc-root ở mọi nhánh ⇒ không bị dựng lại khi dữ liệu bị dọn
  // (giữ trạng thái đang khởi động lại Explorer và focus). Lệch có chủ ý: bảng kết quả nằm trên cùng.
  return (
    <div className="tc-root">
      <RunPanel state={s} onRestartExplorer={() => c.restartExplorer(d)} onClose={() => c.dismissResult(d)} />

      {!data &&
        (s.loadError ? (
          // Băng đỏ đã lên qua notify (có thể bị người dùng đóng); vẫn ghi lỗi nguyên văn tại chỗ để màn không trống.
          <div className="tc-section">
            <div className="tc-row-error">{tt('tweaks.errors.loadFailed', { message: s.loadError })}</div>
            <div className="wfu-actions">
              <Button ref={retryBtn} onClick={() => run(c.load(d))}>
                {tt('tweaks.retry')}
              </Button>
            </div>
          </div>
        ) : (
          <Busy label={tt('tweaks.loading')} size="medium" />
        ))}

      {data && dryRun && (
        <MessageBar intent="info">
          <MessageBarBody>{tt('tweaks.dryRun')}</MessageBarBody>
        </MessageBar>
      )}
      {data?.system.other_user && (
        <MessageBar intent="warning" className="wfu-canhbao">
          <MessageBarBody>{tt('tweaks.otherUser')}</MessageBarBody>
        </MessageBar>
      )}
      {data?.system.managed && (
        <MessageBar intent="info">
          <MessageBarBody>{tt('tweaks.managedMachine')}</MessageBarBody>
        </MessageBar>
      )}

      {data && (
        <div className="tc-presets" role="group" aria-label={tt('tweaks.preset.label')}>
          {LEVELS.map((l) => (
            <Button key={l} appearance={preset === l ? 'primary' : 'secondary'} aria-pressed={preset === l} disabled={busy} onClick={() => c.preset(d, l)}>
              {tt(`tweaks.preset.${l}`)}
            </Button>
          ))}
          {preset === 'custom' && <span className="tc-custom">{tt('tweaks.preset.custom')}</span>}
        </div>
      )}

      {data && <RestorePrompt state={s} onContinue={() => run(c.continueAfterRestoreFailure(d))} onAbort={() => c.abortAfterRestoreFailure(d)} />}

      {data && (
        <div className={s.reloading ? 'tc-list tc-dim' : 'tc-list'} aria-busy={s.reloading}>
          {s.reloading && (
            <div className="tc-overlay">
              <Busy label={tt('tweaks.reloading')} size="small" />
            </div>
          )}
          <TweakList tweaks={tweaks} selected={s.selected} locked={busy} runLocked={locked} onToggle={(id) => c.toggle(d, id)} onReinstall={(id) => run(c.reinstall(d, id))} />
        </div>
      )}

      {data && (
        <div className="tc-footer">
          <span className="tc-sr" aria-live="polite">
            {spoken}
          </span>
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
            <div className="tc-footer-status">{status && <Busy label={status} size="small" />}</div>
            {/* disabledFocusable: nút đang được focus mà bị khoá thì focus không rơi về body. */}
            <Button disabledFocusable={locked || toRevert === 0} onClick={() => run(c.revertSelected(d))}>
              {tt('tweaks.revert', { count: toRevert })}
            </Button>
            <Button ref={applyBtn} appearance="primary" disabledFocusable={locked || toApply === 0} onClick={() => run(c.requestApply(d))}>
              {tt('tweaks.apply', { count: toApply })}
            </Button>
          </div>
        </div>
      )}

      {s.phase === 'confirm' && <ConfirmTweaks state={s} onAccept={() => run(c.acceptConfirm(d))} onCancel={() => c.cancelConfirm(d)} />}
    </div>
  );
}
