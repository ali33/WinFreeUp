import type { RestorePointStatus } from '../../api/types';
import { defaultSelection, needsConfirm, pendingChanges, presetSelection, selectable } from './presets';
import type { ReadResult, RunReport, TweakEvent, TweakLevel } from './types';

/**
 * loading      — lần đọc đầu (chưa có dữ liệu)
 * ready        — danh sách, chọn mục
 * confirm      — hộp xác nhận mục caution / «mọi tài khoản»
 * restorePoint — đang tạo điểm khôi phục (restore = null) hoặc hỏng chờ chọn tiếp/dừng
 * running      — đang áp dụng/hoàn tác (khoá mọi nút)
 * done         — kết quả từng mục + nút khởi động lại Explorer / nhắc khởi động lại
 */
export type Phase = 'loading' | 'ready' | 'confirm' | 'restorePoint' | 'running' | 'done';
export type RunKind = 'apply' | 'revert';

export interface RunState {
  kind: RunKind;
  ids: string[];
  current: string | null;
  finished: string[];
}

export interface TState {
  phase: Phase;
  data: ReadResult | null;
  /** Đang đọc lại sau khi đã có dữ liệu — phủ mờ, không xoá trắng. */
  reloading: boolean;
  loadError: string | null;
  selected: string[];
  /** Người dùng đã tự tích/bỏ — đọc lại không tự đổi lựa chọn nữa. */
  touched: boolean;
  allUsers: boolean;
  restore: RestorePointStatus | null;
  run: RunState | null;
  report: RunReport | null;
}

export const initialTState: TState = {
  phase: 'loading',
  data: null,
  reloading: false,
  loadError: null,
  selected: [],
  touched: false,
  allUsers: false,
  restore: null,
  run: null,
  report: null,
};

export type TAction =
  | { type: 'LOAD_STARTED' }
  | { type: 'LOADED'; data: ReadResult }
  | { type: 'LOAD_FAILED'; message: string }
  | { type: 'TOGGLE'; id: string }
  | { type: 'PRESET'; level: TweakLevel }
  | { type: 'SET_ALL_USERS'; on: boolean }
  | { type: 'REQUEST_APPLY' }
  | { type: 'CONFIRM_ACCEPTED' }
  | { type: 'CONFIRM_CANCELLED' }
  | { type: 'RESTORE_RESULT'; status: RestorePointStatus }
  | { type: 'RESTORE_CONTINUE' }
  | { type: 'RESTORE_ABORT' }
  | { type: 'RUN_STARTED'; kind: RunKind; ids: string[] }
  | { type: 'RUN_EVENT'; event: TweakEvent }
  | { type: 'RUN_DONE'; report: RunReport }
  | { type: 'RUN_FAILED' }
  | { type: 'DISMISS_RESULT' };

/**
 * Mức gốc của lựa chọn khi mở tab: Cơ bản. Lệch có chủ ý so với kế hoạch (quyết định người dùng):
 * lần đọc đầu dùng `defaultSelection` — chỉ mục quyền riêng tư của mức này, không tích sẵn app nào.
 */
export const DEFAULT_LEVEL: TweakLevel = 'basic';

function startRun(s: TState, kind: RunKind, ids: string[]): TState {
  return { ...s, phase: 'running', restore: null, run: { kind, ids, current: null, finished: [] }, report: null };
}

export function reducer(s: TState, a: TAction): TState {
  switch (a.type) {
    case 'LOAD_STARTED':
      return s.data ? { ...s, reloading: true, loadError: null } : { ...s, phase: 'loading', loadError: null };
    case 'LOADED': {
      const tweaks = a.data.tweaks;
      const allowed = new Set(tweaks.filter(selectable).map((t) => t.id));
      const selected = s.touched ? s.selected.filter((id) => allowed.has(id)) : defaultSelection(tweaks);
      const phase = s.phase === 'loading' ? 'ready' : s.phase;
      return { ...s, phase, data: a.data, reloading: false, loadError: null, selected };
    }
    case 'LOAD_FAILED':
      // Lỗi đọc làm mất sạch số liệu ⇒ dọn số cũ (luật báo lỗi: không để người đọc tin số cũ).
      return { ...s, phase: s.phase === 'loading' || s.phase === 'ready' ? 'loading' : s.phase, data: null, reloading: false, loadError: a.message };
    case 'TOGGLE': {
      if (s.phase !== 'ready' || !s.data) return s;
      const t = s.data.tweaks.find((x) => x.id === a.id);
      if (!t || !selectable(t)) return s;
      const selected = s.selected.includes(a.id) ? s.selected.filter((x) => x !== a.id) : [...s.selected, a.id];
      return { ...s, selected, touched: true };
    }
    case 'PRESET':
      if (s.phase !== 'ready' || !s.data) return s;
      return { ...s, selected: presetSelection(s.data.tweaks, a.level), touched: true };
    case 'SET_ALL_USERS':
      if (s.phase !== 'ready') return s;
      return { ...s, allUsers: a.on };
    case 'REQUEST_APPLY': {
      if (s.phase !== 'ready' || !s.data || s.reloading) return s;
      const ids = pendingChanges(s.data.tweaks, s.selected);
      if (ids.length === 0) return s;
      return needsConfirm(s.data.tweaks, ids, s.allUsers).length > 0 ? { ...s, phase: 'confirm' } : { ...s, phase: 'restorePoint', restore: null };
    }
    case 'CONFIRM_ACCEPTED':
      return s.phase === 'confirm' ? { ...s, phase: 'restorePoint', restore: null } : s;
    case 'CONFIRM_CANCELLED':
      return s.phase === 'confirm' ? { ...s, phase: 'ready' } : s;
    case 'RESTORE_RESULT':
      // failed ⇒ đứng lại chờ RESTORE_CONTINUE / RESTORE_ABORT; khác ⇒ bộ điều khiển gửi RUN_STARTED.
      return s.phase === 'restorePoint' && s.restore === null ? { ...s, restore: a.status } : s;
    case 'RESTORE_CONTINUE':
      return s.phase === 'restorePoint' && s.restore?.status === 'failed' ? { ...s, restore: { status: 'skipped' } } : s;
    case 'RESTORE_ABORT':
      return s.phase === 'restorePoint' && s.restore?.status === 'failed' ? { ...s, phase: 'ready', restore: null } : s;
    case 'RUN_STARTED':
      if (a.kind === 'apply' && !(s.phase === 'restorePoint' && s.restore !== null && s.restore.status !== 'failed')) return s;
      // Hoàn tác không chạy khi đang đọc lại (như REQUEST_APPLY) — danh sách đang chờ số thật.
      if (a.kind === 'revert' && (s.phase !== 'ready' || s.reloading)) return s;
      return startRun(s, a.kind, a.ids);
    case 'RUN_EVENT': {
      if (s.phase !== 'running' || !s.run) return s;
      const e = a.event;
      if (e.kind === 'started') return { ...s, run: { ...s.run, current: e.id } };
      return { ...s, run: { ...s.run, current: null, finished: [...s.run.finished, e.outcome.id] } };
    }
    case 'RUN_DONE':
      return s.phase === 'running' ? { ...s, phase: 'done', report: a.report, run: s.run ? { ...s.run, current: null } : null } : s;
    case 'RUN_FAILED':
      return s.phase === 'running' ? { ...s, phase: 'ready', run: null } : s;
    case 'DISMISS_RESULT':
      return s.phase === 'done' ? { ...s, phase: 'ready', run: null, report: null } : s;
  }
}
