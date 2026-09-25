import type { Finding, FindingId, Level } from '../api/types';
import { hasKey, tk } from '../i18n';

/** Đúng thứ tự bảng spec mục 4. */
export const FINDING_ORDER: FindingId[] = [
  'disk_full',
  'disk_health',
  'system_hdd',
  'ram_pressure',
  'startup_apps',
  'uptime',
  'power_saver',
  'cpu_throttle',
  'cpu_hot',
];

export type FindingAction = 'clean' | 'disk' | 'backup' | 'ssd' | 'perf' | 'startup' | 'restart' | 'power' | 'cooling';

/** Cột «Nút» của bảng spec mục 4. */
const ACTIONS: Record<FindingId, FindingAction[]> = {
  disk_full: ['clean', 'disk'],
  disk_health: ['backup'],
  system_hdd: ['ssd'],
  ram_pressure: ['perf'],
  startup_apps: ['startup'],
  uptime: ['restart'],
  power_saver: ['power'],
  cpu_throttle: ['perf'],
  cpu_hot: ['cooling'],
};

/** Chỉ dòng có vấn đề (🔴/🟠) mới có nút xử lý. */
export function actionsFor(f: Finding): FindingAction[] {
  return f.level === 'critical' || f.level === 'warn' ? ACTIONS[f.id] : [];
}

export function levelLabel(level: Level): string {
  return tk(`km.level.${level}`);
}

const decimal = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 1 });

/** Câu giải thích dễ hiểu. Nguồn không đọc được ⇒ «Không đo được: <lý do nguyên văn>». */
export function findingSentence(f: Finding): string {
  if (f.level === 'unknown') return tk('km.overview.unknown', { detail: f.detail ?? '' });
  const params = { value: f.value === null ? '' : decimal.format(f.value), detail: f.detail ?? '' };
  const key = `km.finding.${f.id}.${f.level}`;
  if (hasKey(key)) return tk(key, params);
  // Dòng chỉ có một mức vấn đề (vd system_hdd chỉ 🟠) mà lõi trả mức khác: dùng câu 🟠.
  return tk(`km.finding.${f.id}.warn`, params);
}

export function countProblems(findings: Finding[]): { critical: number; warn: number } {
  return {
    critical: findings.filter((f) => f.level === 'critical').length,
    warn: findings.filter((f) => f.level === 'warn').length,
  };
}
