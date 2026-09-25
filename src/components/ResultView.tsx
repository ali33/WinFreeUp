import { useState } from 'react';
import { Body1, Button, Caption1 } from '@fluentui/react-components';
import { groupName } from '../catalog';
import { formatBytes } from '../format';
import { t } from '../i18n';
import type { State } from '../state/machine';
import { dryRunBytes, reclaimedBytes } from '../state/selectors';
import { Busy } from './Busy';

export function ResultView({ state, onOpenLog, onHome }: { state: State; onOpenLog: () => Promise<void>; onHome: () => void }) {
  const [opening, setOpening] = useState(false);
  const summary = state.summary;
  if (!summary) return null;
  const amount = summary.dry_run ? dryRunBytes(summary) : reclaimedBytes(state.freeBefore, state.freeAfter);
  const measuring = !summary.dry_run && amount === null && !state.freeAfterFailed;
  const openLog = async () => {
    setOpening(true);
    try {
      await onOpenLog();
    } finally {
      setOpening(false);
    }
  };
  return (
    <section className="wfu-hero">
      <Body1>{summary.dry_run ? t('result.titleDryRun') : t('result.title')}</Body1>
      {measuring ? (
        <Busy label={t('result.measuring')} />
      ) : amount === null ? (
        <Body1 className="wfu-muted">{t('result.measureFailed')}</Body1>
      ) : (
        <span className="wfu-hero-number">{formatBytes(amount)}</span>
      )}
      <ul className="wfu-groups">
        {summary.groups.map((g) => (
          <li key={g.id} className="wfu-group">
            <span>{g.error ? '!' : '✓'}</span>
            <div className="wfu-group-main">
              <span className="wfu-group-title">{groupName(g.id)}</span>
              {g.report && g.report.skipped_locked > 0 && <span className="wfu-muted">{t('result.skipped', { count: g.report.skipped_locked })}</span>}
              {g.report && g.report.errors.length > 0 && <span className="wfu-muted">{t('result.groupItemErrors', { count: g.report.errors.length })}</span>}
              {g.error && <span className="wfu-muted">{t('result.groupError', { message: g.error })}</span>}
            </div>
            <span className="wfu-group-size">
              {g.report ? t('result.groupOk', { size: formatBytes(g.report.bytes_freed), files: g.report.files_deleted }) : '—'}
            </span>
          </li>
        ))}
      </ul>
      <Caption1 className="wfu-muted">{t('result.logPath', { path: summary.log_path })}</Caption1>
      <div className="wfu-actions">
        <Button onClick={openLog} disabled={opening}>
          {opening ? <Busy label={t('result.opening')} /> : t('result.openLog')}
        </Button>
        <Button appearance="primary" onClick={onHome}>
          {t('result.home')}
        </Button>
      </div>
    </section>
  );
}
