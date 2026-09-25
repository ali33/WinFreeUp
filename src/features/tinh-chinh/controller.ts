import type { RestorePointStatus } from '../../api/types';
import { messageOf, type Severity } from '../../errors/errors';
import type { Store } from '../../state/store';
import { pendingChanges, revertable } from './presets';
import type { RunKind, TAction, TState } from './reducer';
import { tt } from './strings';
import type { TweakApi, TweakEvent, TweakLevel } from './types';

export type Notify = (severity: Severity, message: string) => void;

export interface TDeps {
  api: TweakApi;
  store: Store<TState, TAction>;
  notify: Notify;
}

export function createTDeps(api: TweakApi, store: Store<TState, TAction>, notify: Notify): TDeps {
  return { api, store, notify };
}

/** Lỗi của lệnh Tauri là chuỗi; hai mã riêng được dịch, còn lại giữ nguyên văn. */
export function friendlyT(e: unknown): string {
  const m = messageOf(e);
  if (m === 'busy') return tt('tweaks.errors.busy');
  if (m === 'dry_run') return tt('tweaks.errors.dryRun');
  return m;
}

export function noticeText(code: string): string {
  const i = code.indexOf(':');
  const [kind, detail] = i < 0 ? [code, ''] : [code.slice(0, i), code.slice(i + 1)];
  return kind === 'undo_corrupt' ? tt('tweaks.notice.undoCorrupt', { detail }) : code;
}

export async function load(d: TDeps): Promise<void> {
  d.store.dispatch({ type: 'LOAD_STARTED' });
  try {
    const data = await d.api.read();
    d.store.dispatch({ type: 'LOADED', data });
    data.notices.forEach((n) => d.notify('warning', noticeText(n)));
  } catch (e) {
    const message = friendlyT(e);
    d.store.dispatch({ type: 'LOAD_FAILED', message });
    d.notify('error', tt('tweaks.errors.loadFailed', { message }));
  }
}

export function toggle(d: TDeps, id: string): void {
  d.store.dispatch({ type: 'TOGGLE', id });
}

export function preset(d: TDeps, level: TweakLevel): void {
  d.store.dispatch({ type: 'PRESET', level });
}

export function setAllUsers(d: TDeps, on: boolean): void {
  d.store.dispatch({ type: 'SET_ALL_USERS', on });
}

export function dismissResult(d: TDeps): void {
  d.store.dispatch({ type: 'DISMISS_RESULT' });
}

async function execute(d: TDeps, kind: RunKind, ids: string[]): Promise<void> {
  const before = d.store.getState();
  d.store.dispatch({ type: 'RUN_STARTED', kind, ids });
  // Reducer từ chối (đang chạy lượt khác, hoặc chưa qua điểm khôi phục) ⇒ trạng thái giữ nguyên tham chiếu ⇒ không gọi lệnh.
  if (d.store.getState() === before) return;
  const allUsers = d.store.getState().allUsers;
  const onEvent = (event: TweakEvent) => d.store.dispatch({ type: 'RUN_EVENT', event });
  let notices: string[] = [];
  try {
    const report = kind === 'apply' ? await d.api.apply(ids, allUsers, onEvent) : await d.api.revert(ids, onEvent);
    d.store.dispatch({ type: 'RUN_DONE', report });
    notices = report.notices ?? [];
  } catch (e) {
    d.store.dispatch({ type: 'RUN_FAILED' });
    d.notify('error', tt(kind === 'apply' ? 'tweaks.errors.applyFailed' : 'tweaks.errors.revertFailed', { message: friendlyT(e) }));
  }
  // Ngoài try: lỗi khi báo notices không được đổi thành «Áp dụng không xong» sau khi lệnh đã xong.
  notices.forEach((n) => d.notify('warning', noticeText(n)));
  await load(d);
}

async function createRestorePoint(d: TDeps): Promise<void> {
  let status: RestorePointStatus;
  try {
    status = await d.api.prepareRestorePoint();
  } catch (e) {
    status = { status: 'failed', message: friendlyT(e) };
  }
  d.store.dispatch({ type: 'RESTORE_RESULT', status });
  if (status.status !== 'failed') await runApply(d);
}

async function runApply(d: TDeps): Promise<void> {
  const s = d.store.getState();
  if (!s.data) return;
  await execute(d, 'apply', pendingChanges(s.data.tweaks, s.selected));
}

export async function requestApply(d: TDeps): Promise<void> {
  const before = d.store.getState();
  d.store.dispatch({ type: 'REQUEST_APPLY' });
  // Reducer từ chối (vd bấm đúp khi đang tạo điểm khôi phục) ⇒ không tạo lần hai.
  if (d.store.getState() === before) return;
  if (d.store.getState().phase === 'restorePoint') await createRestorePoint(d);
}

export async function acceptConfirm(d: TDeps): Promise<void> {
  const before = d.store.getState();
  d.store.dispatch({ type: 'CONFIRM_ACCEPTED' });
  if (d.store.getState() === before) return;
  if (d.store.getState().phase === 'restorePoint') await createRestorePoint(d);
}

export function cancelConfirm(d: TDeps): void {
  d.store.dispatch({ type: 'CONFIRM_CANCELLED' });
}

export async function continueAfterRestoreFailure(d: TDeps): Promise<void> {
  d.store.dispatch({ type: 'RESTORE_CONTINUE' });
  await runApply(d);
}

export function abortAfterRestoreFailure(d: TDeps): void {
  d.store.dispatch({ type: 'RESTORE_ABORT' });
}

export async function revertSelected(d: TDeps): Promise<void> {
  const s = d.store.getState();
  if (!s.data || s.phase !== 'ready') return;
  const ids = revertable(s.data.tweaks, s.selected);
  if (ids.length > 0) await execute(d, 'revert', ids);
}

/** Nút «Cài lại từ Store» của một app đã gỡ = hoàn tác riêng mục đó. */
export async function reinstall(d: TDeps, id: string): Promise<void> {
  await execute(d, 'revert', [id]);
}

export async function restartExplorer(d: TDeps): Promise<void> {
  try {
    await d.api.restartExplorer();
  } catch (e) {
    d.notify('error', tt('tweaks.errors.explorerFailed', { message: friendlyT(e) }));
  }
}
