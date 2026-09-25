import { useEffect, useRef, useState } from 'react';
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
 * Kết quả từng mục (lỗi nguyên văn) và việc cần khởi động lại khi xong. Vùng aria-live luôn tồn tại để trình đọc
 * màn hình báo khi kết quả hiện; kết quả hiện ⇒ cuộn tới và đặt focus vào khung.
 */
export function RunPanel({ state, onRestartExplorer, onClose }: { state: TState; onRestartExplorer: () => Promise<void>; onClose: () => void }) {
  const [restarting, setRestarting] = useState(false);
  const panel = useRef<HTMLDivElement>(null);
  const run = state.run;
  const report = state.report;
  const show = state.phase === 'done' && !!run && !!report;
  useEffect(() => {
    if (!show || !panel.current) return;
    panel.current.scrollIntoView?.({ block: 'nearest' });
    panel.current.focus();
  }, [show]);
  if (!show || !run || !report) return <div aria-live="polite" />;
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
    <div aria-live="polite">
      <div className="tc-panel" ref={panel} tabIndex={-1}>
        <h3 className="tc-section-title">{tt('tweaks.result.title')}</h3>
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
          <Button disabled={restarting} onClick={onClose}>
            {tt('tweaks.result.close')}
          </Button>
        </div>
      </div>
    </div>
  );
}
