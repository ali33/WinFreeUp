import type { Api, CleanEvent, CleanSummary, GroupScan, RestorePointStatus } from '../api/types';
import { groupName, noticeText } from '../catalog';
import { messageOf, type Severity } from '../errors/errors';
import { t } from '../i18n';
import type { Action, State } from './machine';
import type { Store } from './store';

export type Notify = (severity: Severity, message: string) => void;

export interface Deps {
  api: Api;
  store: Store<State, Action>;
  notify: Notify;
  session: {
    scanGen: number;
    scanRun: Promise<unknown> | null;
    cleaning: boolean;
    restoring: boolean;
    cancelling: boolean;
    loadingInitial: Promise<void> | null;
  };
}

export function createDeps(api: Api, store: Store<State, Action>, notify: Notify): Deps {
  return {
    api,
    store,
    notify,
    session: { scanGen: 0, scanRun: null, cleaning: false, restoring: false, cancelling: false, loadingInitial: null },
  };
}

/** Lỗi lệnh Tauri là chuỗi; "busy" đổi sang câu dễ hiểu, còn lại giữ nguyên văn. */
export function friendly(e: unknown): string {
  const m = messageOf(e);
  return m === 'busy' ? t('errors.busy') : m;
}

/** StrictMode chạy effect khởi động 2 lần ⇒ appInfo/diskFree bị gọi đôi nếu không chặn; giữ lại
 * lời gọi đang chạy và trả cùng promise đó cho lần gọi chồng, giống các cờ `session` khác. */
export async function loadInitial(d: Deps): Promise<void> {
  if (d.session.loadingInitial) return d.session.loadingInitial;
  const run = (async () => {
    try {
      const info = await d.api.appInfo();
      d.store.dispatch({ type: 'APP_INFO', dryRun: info.dry_run, systemDrive: info.system_drive });
    } catch (e) {
      d.notify('warning', t('errors.appInfoFailed', { message: friendly(e) }));
    }
    await refreshFree(d);
  })();
  d.session.loadingInitial = run;
  try {
    await run;
  } finally {
    if (d.session.loadingInitial === run) d.session.loadingInitial = null;
  }
}

export async function refreshFree(d: Deps): Promise<void> {
  try {
    d.store.dispatch({ type: 'FREE_SPACE', bytes: await d.api.diskFree() });
  } catch (e) {
    d.store.dispatch({ type: 'FREE_SPACE_FAILED' });
    d.notify('warning', t('errors.freeSpaceFailed', { message: friendly(e) }));
  }
}

function reportScanIssues(d: Deps, groups: GroupScan[]): void {
  for (const g of groups) {
    if (g.error && g.error !== 'cancelled') {
      d.notify('warning', t('notice.groupFailed', { name: groupName(g.id), message: g.error }));
    }
    for (const n of g.result?.notices ?? []) d.notify('warning', noticeText(n));
  }
}

export async function startScan(d: Deps): Promise<void> {
  if (d.session.scanRun) return;
  d.store.dispatch({ type: 'SCAN_STARTED' });
  if (d.store.getState().phase !== 'scanning') return;
  const gen = ++d.session.scanGen;
  const run = d.api.scanAll((g) => {
    if (gen === d.session.scanGen) d.store.dispatch({ type: 'SCAN_GROUP_DONE', group: g });
  });
  d.session.scanRun = run;
  try {
    const groups = await run;
    if (gen !== d.session.scanGen) return;
    d.store.dispatch({ type: 'SCAN_FINISHED', groups });
    reportScanIssues(d, groups);
  } catch (e) {
    if (gen !== d.session.scanGen) return;
    d.store.dispatch({ type: 'SCAN_ABORTED' });
    d.notify('error', t('errors.scanFailed', { message: friendly(e) }));
  } finally {
    if (d.session.scanRun === run) d.session.scanRun = null;
  }
}

/** Giữ vòng quay «Đang hủy…» cho tới khi lõi dừng hẳn, để lần Quét kế tiếp không bị «busy». */
export async function cancelScan(d: Deps): Promise<void> {
  if (d.store.getState().phase !== 'scanning' || d.session.cancelling) return;
  d.session.cancelling = true;
  try {
    d.store.dispatch({ type: 'SCAN_CANCEL_REQUESTED' });
    d.session.scanGen++;
    const run = d.session.scanRun;
    try {
      await d.api.cancelScan();
    } catch (e) {
      // KHÔNG có khoá i18n hợp nghĩa cho "hủy quét thất bại" (errors.scanFailed nói về quét,
      // không phải hủy) và vi.json dùng chung, không được thêm khoá mới — giữ nguyên câu gốc.
      d.notify('warning', friendly(e));
    }
    if (run) await run.catch(() => undefined);
    d.store.dispatch({ type: 'SCAN_ABORTED' });
  } finally {
    d.session.cancelling = false;
  }
}

export function toggle(d: Deps, id: string): void {
  d.store.dispatch({ type: 'TOGGLE', id });
}

export async function requestClean(d: Deps): Promise<void> {
  d.store.dispatch({ type: 'REQUEST_CLEAN' });
  await advance(d);
}

export async function acceptConfirm(d: Deps): Promise<void> {
  d.store.dispatch({ type: 'CONFIRM_ACCEPTED' });
  await advance(d);
}

export function cancelConfirm(d: Deps): void {
  d.store.dispatch({ type: 'CONFIRM_CANCELLED' });
}

export async function continueAfterRestoreFailure(d: Deps): Promise<void> {
  d.store.dispatch({ type: 'RESTORE_CONTINUE' });
  await advance(d);
}

export function abortAfterRestoreFailure(d: Deps): void {
  d.store.dispatch({ type: 'RESTORE_ABORT' });
}

async function advance(d: Deps): Promise<void> {
  const s = d.store.getState();
  if (s.phase === 'restorePoint' && s.restore === null && !d.session.restoring) return runRestore(d);
  if (s.phase === 'cleaning' && s.summary === null && !d.session.cleaning) return runClean(d);
}

async function runRestore(d: Deps): Promise<void> {
  d.session.restoring = true;
  try {
    const s = d.store.getState();
    let status: RestorePointStatus;
    try {
      status = await d.api.prepareRestorePoint(s.selected, s.dryRun);
    } catch (e) {
      status = { status: 'failed', message: friendly(e) };
    }
    d.store.dispatch({ type: 'RESTORE_RESULT', status });
    await advance(d);
  } finally {
    d.session.restoring = false;
  }
}

async function runClean(d: Deps): Promise<void> {
  d.session.cleaning = true;
  try {
    const s = d.store.getState();
    try {
      d.store.dispatch({ type: 'FREE_SPACE', bytes: await d.api.diskFree() });
    } catch (e) {
      d.notify('warning', t('errors.freeSpaceFailed', { message: friendly(e) }));
    }
    let summary: CleanSummary;
    try {
      summary = await d.api.clean(s.selected, s.dryRun, (ev: CleanEvent) => d.store.dispatch({ type: 'CLEAN_EVENT', event: ev }));
    } catch (e) {
      d.store.dispatch({ type: 'CLEAN_FAILED' });
      d.notify('error', t('errors.cleanFailed', { message: friendly(e) }));
      return;
    }
    d.store.dispatch({ type: 'CLEAN_DONE', summary });
    for (const g of summary.groups) {
      if (g.error) {
        d.notify('warning', t('notice.groupFailed', { name: groupName(g.id), message: g.error }));
      } else if (g.report && g.report.errors.length > 0) {
        d.notify('warning', t('notice.groupErrors', { name: groupName(g.id), count: g.report.errors.length, first: noticeText(g.report.errors[0]) }));
      }
    }
    if (summary.log_write_failed) d.notify('warning', t('notice.logWriteFailed'));
    try {
      d.store.dispatch({ type: 'FREE_AFTER', bytes: await d.api.diskFree() });
    } catch (e) {
      d.store.dispatch({ type: 'FREE_AFTER_FAILED' });
      d.notify('warning', t('errors.freeSpaceFailed', { message: friendly(e) }));
    }
  } finally {
    d.session.cleaning = false;
  }
}

export async function openLog(d: Deps): Promise<void> {
  try {
    await d.api.openLogFolder();
  } catch (e) {
    d.notify('warning', t('errors.openLogFailed', { message: friendly(e) }));
  }
}

export async function goHome(d: Deps): Promise<void> {
  d.store.dispatch({ type: 'RESET' });
  await refreshFree(d);
}
