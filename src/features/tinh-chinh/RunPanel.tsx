import { useState } from 'react';
import { Button, MessageBar, MessageBarBody } from '@fluentui/react-components';
import { Busy } from '../../components/Busy';
import { itemName, outcomeText } from './labels';
import type { TState } from './reducer';
import { tt } from './strings';

/** Tiến độ khi đang chạy; kết quả từng mục (lỗi nguyên văn) và việc cần khởi động lại khi xong. */
export function RunPanel({ state, onRestartExplorer, onClose }: { state: TState; onRestartExplorer: () => Promise<void>; onClose: () => void }) {
  const [restarting, setRestarting] = useState(false);
  const run = state.run;
  if (!run) return null;
  if (state.phase === 'running') {
    const label = tt(run.kind === 'apply' ? 'tweaks.run.apply' : 'tweaks.run.revert', { done: run.finished.length, total: run.ids.length });
    return (
      <div className="tc-panel" aria-live="polite">
        <Busy label={run.current ? `${label} ${itemName(run.current)}` : label} size="small" />
      </div>
    );
  }
  const report = state.report;
  if (state.phase !== 'done' || !report) return null;
  const restartExplorer = async () => {
    setRestarting(true);
    try {
      await onRestartExplorer();
    } finally {
      setRestarting(false);
    }
  };
  return (
    <div className="tc-panel" aria-live="polite">
      <h3 className="tc-section-title">{tt('tweaks.result.title')}</h3>
      <ul className="tc-results">
        {report.outcomes.map((o) => (
          <li key={o.id}>
            <span className="tc-result-mark">{outcomeText(o, run.kind)}</span> {itemName(o.id)}
            {o.store_opened.length > 0 && <div className="wfu-muted">{tt('tweaks.result.storeOpened')}</div>}
            {o.errors.map((e) => (
              <div key={e} className="tc-row-error">
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
  );
}
