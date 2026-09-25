import type { CleanSummary } from '../api/types';
import type { GroupState, State } from './machine';

export type ConfirmKind = 'none' | 'caution' | 'risky';

export function selectable(g: GroupState): boolean {
  return g.status === 'done' && (g.result?.total_bytes ?? 0) > 0;
}

export function selectedBytes(s: State): number {
  return s.groups
    .filter((g) => s.selected.includes(g.id) && g.result)
    .reduce((sum, g) => sum + (g.result?.total_bytes ?? 0), 0);
}

export function confirmKind(groups: GroupState[], selected: string[]): ConfirmKind {
  const risks = groups.filter((g) => selected.includes(g.id)).map((g) => g.risk);
  if (risks.includes('risky')) return 'risky';
  if (risks.includes('caution')) return 'caution';
  return 'none';
}

/** "XOA" sau khi bỏ dấu tiếng Việt, khoảng trắng hai đầu và phân biệt hoa/thường. */
export function isConfirmWord(input: string): boolean {
  const plain = input
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[đĐ]/g, 'd')
    .trim()
    .toUpperCase();
  return plain === 'XOA';
}

export function reclaimedBytes(before: number | null, after: number | null): number | null {
  if (before === null || after === null) return null;
  return Math.max(0, after - before);
}

export function dryRunBytes(summary: CleanSummary): number {
  return summary.groups.reduce((sum, g) => sum + (g.report?.bytes_freed ?? 0), 0);
}
