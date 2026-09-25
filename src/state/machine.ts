import type { CleanEvent, CleanSummary, GroupClean, GroupScan, RestorePointStatus, RiskLevel, ScanResult } from '../api/types';
import { GROUP_IDS } from '../catalog';
import { confirmKind, selectable } from './selectors';

export type Phase = 'welcome' | 'scanning' | 'preview' | 'confirm' | 'restorePoint' | 'cleaning' | 'result';

export interface GroupState {
  id: string;
  status: 'pending' | 'done' | 'error';
  risk: RiskLevel | null;
  defaultSelected: boolean;
  result: ScanResult | null;
  error: string | null;
}

export interface CleanRow {
  id: string;
  status: 'waiting' | 'running' | 'done';
  percent: number | null;
  result: GroupClean | null;
}

export interface State {
  phase: Phase;
  dryRun: boolean;
  systemDrive: string;
  freeBefore: number | null;
  freeFailed: boolean;
  freeAfter: number | null;
  freeAfterFailed: boolean;
  groups: GroupState[];
  selected: string[];
  cancelling: boolean;
  restore: RestorePointStatus | null;
  cleaning: CleanRow[];
  summary: CleanSummary | null;
}

export const initialState: State = {
  phase: 'welcome',
  dryRun: false,
  systemDrive: 'C:\\',
  freeBefore: null,
  freeFailed: false,
  freeAfter: null,
  freeAfterFailed: false,
  groups: [],
  selected: [],
  cancelling: false,
  restore: null,
  cleaning: [],
  summary: null,
};

export type Action =
  | { type: 'APP_INFO'; dryRun: boolean; systemDrive: string }
  | { type: 'FREE_SPACE'; bytes: number }
  | { type: 'FREE_SPACE_FAILED' }
  | { type: 'FREE_BEFORE_FAILED' }
  | { type: 'SCAN_STARTED' }
  | { type: 'SCAN_GROUP_DONE'; group: GroupScan }
  | { type: 'SCAN_FINISHED'; groups: GroupScan[] }
  | { type: 'SCAN_CANCEL_REQUESTED' }
  | { type: 'SCAN_ABORTED' }
  | { type: 'TOGGLE'; id: string }
  | { type: 'REQUEST_CLEAN' }
  | { type: 'CONFIRM_ACCEPTED' }
  | { type: 'CONFIRM_CANCELLED' }
  | { type: 'RESTORE_RESULT'; status: RestorePointStatus }
  | { type: 'RESTORE_CONTINUE' }
  | { type: 'RESTORE_ABORT' }
  | { type: 'CLEAN_EVENT'; event: CleanEvent }
  | { type: 'CLEAN_DONE'; summary: CleanSummary }
  | { type: 'CLEAN_FAILED' }
  | { type: 'FREE_AFTER'; bytes: number }
  | { type: 'FREE_AFTER_FAILED' }
  | { type: 'RESET' };

function fromScan(g: GroupScan): GroupState {
  return {
    id: g.id,
    status: g.error ? 'error' : 'done',
    risk: g.risk,
    defaultSelected: g.default_selected,
    result: g.error ? null : g.result,
    error: g.error,
  };
}

function pendingGroups(): GroupState[] {
  return GROUP_IDS.map((id) => ({ id, status: 'pending', risk: null, defaultSelected: false, result: null, error: null }));
}

function mergeGroup(groups: GroupState[], g: GroupScan): GroupState[] {
  return groups.some((x) => x.id === g.id)
    ? groups.map((x) => (x.id === g.id ? fromScan(g) : x))
    : [...groups, fromScan(g)];
}

function startCleaning(s: State): State {
  return {
    ...s,
    phase: 'cleaning',
    cleaning: s.selected.map((id) => ({ id, status: 'waiting', percent: null, result: null })),
    summary: null,
    freeAfter: null,
    freeAfterFailed: false,
  };
}

function applyCleanEvent(rows: CleanRow[], e: CleanEvent): CleanRow[] {
  switch (e.kind) {
    case 'started':
      return rows.map((r) => (r.id === e.id ? { ...r, status: 'running' } : r));
    case 'percent':
      return rows.map((r) => (r.id === e.id ? { ...r, percent: e.percent } : r));
    case 'finished':
      return rows.map((r) => (r.id === e.result.id ? { ...r, status: 'done', result: e.result } : r));
  }
}

export function reducer(s: State, a: Action): State {
  switch (a.type) {
    case 'APP_INFO':
      return { ...s, dryRun: a.dryRun, systemDrive: a.systemDrive };
    case 'FREE_SPACE':
      return { ...s, freeBefore: a.bytes, freeFailed: false };
    case 'FREE_SPACE_FAILED':
      return { ...s, freeFailed: true };
    // Đo lại "trước" ngay khi bắt đầu dọn (runClean) mà lỗi: freeBefore cũ có thể đã lệch xa
    // thực tế (đo từ màn Chào), không được giữ lại kẻo «Đã lấy lại» ở màn Kết quả tính ra số
    // sai. Đặt về null để buộc hiện «Không đo được…».
    case 'FREE_BEFORE_FAILED':
      return { ...s, freeBefore: null, freeFailed: true };
    case 'SCAN_STARTED':
      if (s.phase !== 'welcome' && s.phase !== 'preview') return s;
      return { ...s, phase: 'scanning', groups: pendingGroups(), selected: [], cancelling: false };
    case 'SCAN_GROUP_DONE':
      if (s.phase !== 'scanning') return s;
      return { ...s, groups: mergeGroup(s.groups, a.group) };
    case 'SCAN_FINISHED': {
      if (s.phase !== 'scanning') return s;
      const groups = a.groups.reduce(mergeGroup, s.groups);
      const selected = groups.filter((g) => g.defaultSelected && selectable(g)).map((g) => g.id);
      return { ...s, phase: 'preview', groups, selected, cancelling: false };
    }
    case 'SCAN_CANCEL_REQUESTED':
      if (s.phase !== 'scanning') return s;
      return { ...s, cancelling: true };
    case 'SCAN_ABORTED':
      if (s.phase !== 'scanning') return s;
      return { ...s, phase: 'welcome', groups: [], selected: [], cancelling: false };
    case 'TOGGLE': {
      if (s.phase !== 'preview') return s;
      const g = s.groups.find((x) => x.id === a.id);
      if (!g || !selectable(g)) return s;
      const on = !s.selected.includes(a.id);
      const selected = s.groups
        .filter((x) => (x.id === a.id ? on : s.selected.includes(x.id)))
        .map((x) => x.id);
      return { ...s, selected };
    }
    case 'REQUEST_CLEAN':
      if (s.phase !== 'preview' || s.selected.length === 0) return s;
      return confirmKind(s.groups, s.selected) === 'none'
        ? startCleaning({ ...s, restore: null })
        : { ...s, phase: 'confirm' };
    case 'CONFIRM_ACCEPTED':
      if (s.phase !== 'confirm') return s;
      return { ...s, phase: 'restorePoint', restore: null };
    case 'CONFIRM_CANCELLED':
      if (s.phase !== 'confirm') return s;
      return { ...s, phase: 'preview' };
    case 'RESTORE_RESULT':
      if (s.phase !== 'restorePoint' || s.restore !== null) return s;
      return a.status.status === 'failed' ? { ...s, restore: a.status } : startCleaning({ ...s, restore: a.status });
    case 'RESTORE_CONTINUE':
      if (s.phase !== 'restorePoint' || s.restore?.status !== 'failed') return s;
      return startCleaning(s);
    case 'RESTORE_ABORT':
      if (s.phase !== 'restorePoint' || s.restore?.status !== 'failed') return s;
      return { ...s, phase: 'preview', restore: null };
    case 'CLEAN_EVENT':
      if (s.phase !== 'cleaning') return s;
      return { ...s, cleaning: applyCleanEvent(s.cleaning, a.event) };
    case 'CLEAN_DONE':
      if (s.phase !== 'cleaning') return s;
      // freeBefore đã null (do FREE_BEFORE_FAILED) thì «Đã lấy lại» không tính được dù đo "sau"
      // có thành công hay không ⇒ đánh dấu freeAfterFailed ngay từ đây để màn Kết quả hiện
      // «Không đo được…» thay vì kẹt mãi ở vòng quay (FREE_AFTER không tự xoá cờ này).
      return { ...s, phase: 'result', summary: a.summary, freeAfter: null, freeAfterFailed: s.freeBefore === null };
    case 'CLEAN_FAILED':
      if (s.phase !== 'cleaning') return s;
      return { ...s, phase: 'preview', cleaning: [] };
    case 'FREE_AFTER':
      if (s.phase !== 'result') return s;
      return { ...s, freeAfter: a.bytes };
    case 'FREE_AFTER_FAILED':
      if (s.phase !== 'result') return s;
      return { ...s, freeAfterFailed: true };
    case 'RESET':
      return { ...initialState, dryRun: s.dryRun, systemDrive: s.systemDrive };
  }
}
