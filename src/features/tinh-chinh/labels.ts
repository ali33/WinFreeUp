import { tt } from './strings';
import type { TweakLevel, TweakOutcome, TweakView } from './types';

export function itemName(id: string): string {
  return tt(`tweaks.item.${id}.name`);
}

export function itemDesc(id: string): string {
  return tt(`tweaks.item.${id}.desc`);
}

export function levelText(l: TweakLevel): string {
  return tt(`tweaks.preset.${l}`);
}

/** Mã lý do của lõi (`build_min:26100`, `build_max:19045`, `edition`, `missing`) ⇒ câu cho người đọc. */
export function reasonText(reason: string): string {
  const [kind, arg] = reason.split(':');
  if (kind === 'build_min') {
    const build = Number(arg);
    if (build === 22000) return tt('tweaks.reason.win11');
    if (build === 26100) return tt('tweaks.reason.win11_24h2');
    return tt('tweaks.reason.buildMin', { build: arg ?? '?' });
  }
  if (kind === 'build_max') return tt('tweaks.reason.win10Only');
  if (kind === 'edition') return tt('tweaks.reason.edition');
  if (kind === 'missing') return tt('tweaks.reason.missing');
  return reason;
}

export function statusText(t: TweakView): string {
  const app = t.group === 'bloatware';
  switch (t.status) {
    case 'applied':
      return tt(app ? 'tweaks.status.removed' : 'tweaks.status.applied');
    case 'not_applied':
      return tt(app ? 'tweaks.status.notRemoved' : 'tweaks.status.notApplied');
    case 'partial':
      return tt('tweaks.status.partial');
    case 'managed':
      return tt('tweaks.status.managed');
    case 'unsupported':
      return tt('tweaks.status.unsupported', { reason: reasonText(t.reason) });
    case 'not_present':
      return '';
  }
}

/** ✓ khi không lỗi và (áp dụng xong hoặc hoàn tác xong); ⚠ khi còn một phần; ✗ khi có lỗi. */
export function outcomeText(o: TweakOutcome, kind: 'apply' | 'revert'): string {
  if (o.errors.length > 0) return o.status === 'partial' ? tt('tweaks.result.partial') : tt('tweaks.result.failed');
  if (kind === 'apply' && o.status === 'partial') return tt('tweaks.result.partial');
  return tt('tweaks.result.ok');
}
