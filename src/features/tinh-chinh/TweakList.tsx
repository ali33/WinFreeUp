import { Badge, Button, Checkbox } from '@fluentui/react-components';
import { itemDesc, itemName, levelText, statusText } from './labels';
import { selectable, visible } from './presets';
import { tt } from './strings';
import type { TweakGroup, TweakView } from './types';

interface ListProps {
  tweaks: TweakView[];
  selected: string[];
  /** Đang chạy/đọc lại ⇒ khoá mọi ô và nút. */
  locked: boolean;
  onToggle: (id: string) => void;
  onReinstall: (id: string) => void;
}

function statusColor(t: TweakView): 'success' | 'warning' | 'informative' | 'subtle' {
  if (t.status === 'applied') return 'success';
  if (t.status === 'partial') return 'warning';
  if (t.status === 'managed' || t.status === 'unsupported') return 'subtle';
  return 'informative';
}

function Row({ t, checked, locked, onToggle, onReinstall }: { t: TweakView; checked: boolean; locked: boolean; onToggle: () => void; onReinstall: () => void }) {
  const canPick = selectable(t);
  return (
    <li className={canPick ? 'tc-row' : 'tc-row tc-row-off'} data-testid={`tweak-${t.id}`}>
      <Checkbox checked={checked} disabled={!canPick || locked} onChange={onToggle} aria-label={itemName(t.id)} />
      <div className="tc-row-main">
        <div className="tc-row-title">
          <span>{itemName(t.id)}</span>
          {t.risk === 'caution' && (
            <Badge appearance="tint" color="warning" size="small">
              ⚠ {tt('tweaks.caution')}
            </Badge>
          )}
        </div>
        <div className="wfu-muted">{itemDesc(t.id)}</div>
        {t.errors.map((e) => (
          <div key={e} className="tc-row-error">
            {tt('tweaks.readError', { message: e })}
          </div>
        ))}
      </div>
      <span className="tc-level">{levelText(t.level)}</span>
      <Badge appearance="outline" color={statusColor(t)} className="tc-status">
        {statusText(t)}
      </Badge>
      {t.group === 'bloatware' && t.status === 'applied' && t.has_undo ? (
        <Button size="small" disabled={locked} onClick={onReinstall}>
          {tt('tweaks.reinstall')}
        </Button>
      ) : (
        <span />
      )}
    </li>
  );
}

function Section({ group, title, ...p }: ListProps & { group: TweakGroup; title: string }) {
  const rows = p.tweaks.filter((t) => t.group === group && visible(t));
  if (rows.length === 0) return null;
  const chosen = new Set(p.selected);
  return (
    <section className="tc-section">
      <h3 className="tc-section-title">{title}</h3>
      <ul className="tc-rows">
        {rows.map((t) => (
          <Row key={t.id} t={t} checked={chosen.has(t.id)} locked={p.locked} onToggle={() => p.onToggle(t.id)} onReinstall={() => p.onReinstall(t.id)} />
        ))}
      </ul>
    </section>
  );
}

export function TweakList(p: ListProps) {
  const apps = p.tweaks.filter((t) => t.group === 'bloatware' && visible(t));
  const removed = apps.filter((t) => t.status === 'applied').length;
  const present = apps.filter((t) => t.status === 'not_applied' || t.status === 'partial').length;
  return (
    <>
      <Section {...p} group="bloatware" title={tt('tweaks.group.bloatware', { present, removed })} />
      <Section {...p} group="privacy" title={tt('tweaks.group.privacy')} />
    </>
  );
}
