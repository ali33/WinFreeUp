import { useEffect, useId, useRef, useState } from 'react';
import { Button, MessageBar, MessageBarBody } from '@fluentui/react-components';
import { Busy } from '../../components/Busy';
import { itemName, outcomeText } from './labels';
import type { TState } from './reducer';
import { tt } from './strings';

/** Nhãn tiến độ «Đang áp dụng i/n… tên mục» — TinhChinhView vẽ ở chân trang, cạnh nút. */
export function runLabel(state: TState): string | null {
  const run = state.run;
  if (state.phase !== 'running' || !run) return null;
  const label = tt(run.kind === 'apply' ? 'tweaks.run.apply' : 'tweaks.run.revert', { done: run.finished.length, total: run.ids.length });
  return run.current ? `${label} ${itemName(run.current)}` : label;
}

/**
 * Nhãn cho trình đọc màn hình (.tc-sr): chỉ báo khi đổi việc / đổi mục, không đọc lại «i/n» ở mỗi sự kiện.
 * TinhChinhView đặt kết quả vào vùng aria-live="polite" ở chân trang.
 */
export function spokenLabel(state: TState): string {
  if (state.phase === 'restorePoint' && state.restore === null) return tt('tweaks.restore.creating');
  if (state.phase === 'running') return state.run?.current ? itemName(state.run.current) : '';
  if (state.reloading) return tt('tweaks.reloading');
  return '';
}

/**
 * Kết quả từng mục (lỗi nguyên văn) và việc cần khởi động lại khi xong. Kết quả hiện ⇒ cuộn tới và đặt focus vào
 * khung (region mang tên «Kết quả»); vì đã nhận focus nên không dùng aria-live, tránh trình đọc màn hình đọc hai lần.
 * TinhChinhView giữ component này ở cùng vị trí con ở mọi nhánh để `restarting` không bị mất khi dữ liệu bị dọn.
 */
export function RunPanel({ state, onRestartExplorer, onClose }: { state: TState; onRestartExplorer: () => Promise<void>; onClose: () => void }) {
  const [restarting, setRestarting] = useState(false);
  const panel = useRef<HTMLDivElement>(null);
  const titleId = useId();
  const run = state.run;
  const report = state.report;
  const show = state.phase === 'done' && !!run && !!report;
  useEffect(() => {
    if (!show || !panel.current) return;
    panel.current.scrollIntoView?.({ block: 'nearest' });
    panel.current.focus();
  }, [show]);
  if (!show || !run || !report) return null;
  const restartExplorer = async () => {
    // Nút sắp bị thay bằng vòng quay ⇒ chuyển focus về khung kết quả, không để rơi về body.
    panel.current?.focus();
    setRestarting(true);
    try {
      await onRestartExplorer();
    } finally {
      setRestarting(false);
    }
  };
  return (
    <div>
      <div className="tc-panel" ref={panel} tabIndex={-1} role="region" aria-labelledby={titleId}>
        <h3 className="tc-section-title" id={titleId}>
          {tt('tweaks.result.title')}
        </h3>
        <ul className="tc-results">
          {report.outcomes.map((o) => (
            <li key={o.id}>
              <span className="tc-result-mark">{outcomeText(o, run.kind)}</span> {itemName(o.id)}
              {o.store_opened.length > 0 && <div className="wfu-muted">{tt('tweaks.result.storeOpened')}</div>}
              {o.errors.map((e, i) => (
                <div key={i} className="tc-row-error">
                  {e}
                </div>
              ))}
            </li>
          ))}
        </ul>
        {report.restart === 'logoff' && (
          <MessageBar intent="info">
            <MessageBarBody>{tt('tweaks.result.logoff')}</MessageBarBody>
          </MessageBar>
        )}
        {report.restart === 'reboot' && (
          <MessageBar intent="info">
            <MessageBarBody>{tt('tweaks.result.reboot')}</MessageBarBody>
          </MessageBar>
        )}
        <div className="wfu-actions">
          {report.restart === 'explorer' &&
            (restarting ? (
              <Busy label={tt('tweaks.result.explorerBusy')} />
            ) : (
              <Button appearance="primary" onClick={() => void restartExplorer()}>
                {tt('tweaks.result.explorer')}
              </Button>
            ))}
          <Button disabledFocusable={restarting} onClick={onClose}>
            {tt('tweaks.result.close')}
          </Button>
        </div>
      </div>
    </div>
  );
}
