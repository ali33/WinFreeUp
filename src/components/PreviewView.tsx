import { useState } from 'react';
import { Badge, Body1, Button, Checkbox, Title3 } from '@fluentui/react-components';
import type { RiskLevel } from '../api/types';
import { groupDesc, groupName } from '../catalog';
import { formatBytes } from '../format';
import { t } from '../i18n';
import type { GroupState, State } from '../state/machine';
import { selectable, selectedBytes } from '../state/selectors';

const RISK_COLOR: Record<RiskLevel, 'success' | 'warning' | 'danger'> = {
  safe: 'success',
  caution: 'warning',
  risky: 'danger',
};

function GroupRow({ g, checked, onToggle }: { g: GroupState; checked: boolean; onToggle: (id: string) => void }) {
  const [open, setOpen] = useState(false);
  const bytes = g.result?.total_bytes ?? 0;
  const size = g.result?.estimated ? t('preview.estimated', { size: formatBytes(bytes) }) : formatBytes(bytes);
  return (
    <li className="wfu-group">
      <Checkbox checked={checked} disabled={!selectable(g)} onChange={() => onToggle(g.id)} aria-label={groupName(g.id)} />
      <div className="wfu-group-main">
        <span className="wfu-group-title">
          {groupName(g.id)}
          {g.risk && (
            <Badge appearance="tint" color={RISK_COLOR[g.risk]}>
              {t(`risk.${g.risk}`)}
            </Badge>
          )}
        </span>
        <span className="wfu-group-desc">{groupDesc(g.id)}</span>
        {g.status === 'error' && <span className="wfu-muted">{t('preview.groupError', { message: g.error ?? '' })}</span>}
        {g.result && g.result.top_items.length > 0 && (
          <Button appearance="transparent" size="small" onClick={() => setOpen(!open)}>
            {open ? t('preview.hideItems') : t('preview.showItems')}
          </Button>
        )}
      </div>
      <span className="wfu-group-size">{g.status === 'error' ? '—' : size}</span>
      {open && g.result && (
        <ul className="wfu-items">
          {g.result.top_items.map((it) => (
            <li key={it.path} className="wfu-item">
              <span className="wfu-item-path" title={it.path}>
                {it.path}
              </span>
              <span>{formatBytes(it.bytes)}</span>
            </li>
          ))}
        </ul>
      )}
    </li>
  );
}

export function PreviewView({
  state,
  onToggle,
  onClean,
  onRescan,
}: {
  state: State;
  onToggle: (id: string) => void;
  onClean: () => void;
  onRescan: () => void;
}) {
  const anything = state.groups.some(selectable);
  return (
    <section className="wfu-hero">
      <Title3>{t('preview.title')}</Title3>
      {!anything && <Body1>{t('preview.nothing')}</Body1>}
      <ul className="wfu-groups">
        {state.groups.map((g) => (
          <GroupRow key={g.id} g={g} checked={state.selected.includes(g.id)} onToggle={onToggle} />
        ))}
      </ul>
      <div className="wfu-actions">
        <Button appearance="primary" disabled={state.selected.length === 0} onClick={onClean}>
          {t('preview.clean', { size: formatBytes(selectedBytes(state)) })}
        </Button>
        <Button onClick={onRescan}>{t('preview.rescan')}</Button>
      </div>
    </section>
  );
}
