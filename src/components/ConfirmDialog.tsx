import { useState } from 'react';
import {
  Body1,
  Button,
  Dialog,
  DialogActions,
  DialogBody,
  DialogContent,
  DialogSurface,
  DialogTitle,
  Field,
  Input,
} from '@fluentui/react-components';
import { groupDesc, groupName } from '../catalog';
import { t } from '../i18n';
import type { State } from '../state/machine';
import { confirmKind, isConfirmWord } from '../state/selectors';

export function ConfirmDialog({ state, onAccept, onCancel }: { state: State; onAccept: () => void; onCancel: () => void }) {
  const [word, setWord] = useState('');
  const kind = confirmKind(state.groups, state.selected);
  const listed = state.groups.filter((g) => state.selected.includes(g.id) && g.risk !== 'safe');
  const allowed = kind !== 'risky' || isConfirmWord(word);
  return (
    <Dialog open modalType="alert">
      <DialogSurface>
        <DialogBody>
          <DialogTitle>{t('confirm.title')}</DialogTitle>
          <DialogContent>
            <Body1>{t('confirm.cautionIntro')}</Body1>
            <ul>
              {listed.map((g) => (
                <li key={g.id}>
                  <strong>{groupName(g.id)}</strong>
                  {' — '}
                  {g.risk ? t(`risk.${g.risk}`) : ''}: {groupDesc(g.id)}
                </li>
              ))}
            </ul>
            {kind === 'risky' && (
              <>
                <Body1>{t('confirm.riskyIntro')}</Body1>
                <Field label={t('confirm.riskyLabel')}>
                  <Input value={word} onChange={(_, data) => setWord(data.value)} autoFocus />
                </Field>
              </>
            )}
          </DialogContent>
          <DialogActions>
            <Button appearance="secondary" onClick={onCancel}>
              {t('confirm.cancel')}
            </Button>
            <Button appearance="primary" disabled={!allowed} onClick={onAccept}>
              {t('confirm.ok')}
            </Button>
          </DialogActions>
        </DialogBody>
      </DialogSurface>
    </Dialog>
  );
}
