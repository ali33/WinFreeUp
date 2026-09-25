import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  ChildrenPage,
  DeleteResult,
  Finding,
  KhamMayApi,
  KillResult,
  PerfTick,
  Sample,
  ScanStatus,
  ScanSummary,
  StartupEntry,
  StartupList,
  VolumeInfo,
} from './types';

/** Gỡ lắng nghe của lần `perfStart` đang chạy. */
let perfUnlisten: UnlistenFn[] = [];
/** perfStart/perfStop xếp hàng lần lượt: StrictMode gọi start → stop → start liền nhau, chạy chồng thì
 *  lắng nghe của lần start đầu có thể bị bỏ sót không gỡ và mỗi mẫu tới hai lần. */
let perfQueue: Promise<unknown> = Promise.resolve();

function serial<T>(run: () => Promise<T>): Promise<T> {
  const next = perfQueue.then(run, run);
  perfQueue = next.catch(() => undefined);
  return next;
}

async function withEvent<E, R>(event: string, onEvent: (e: E) => void, run: () => Promise<R>): Promise<R> {
  const unlisten = await listen<E>(event, (e) => onEvent(e.payload));
  try {
    return await run();
  } finally {
    unlisten();
  }
}

// Tên lệnh/sự kiện khớp phần đăng ký trong src-tauri (Task Tích hợp). Tham số camelCase ⇒ Rust snake_case.
export const khamMayTauriApi: KhamMayApi = {
  diskVolumes: () => invoke<VolumeInfo[]>('disk_volumes'),
  diskScan: (root, onProgress) =>
    withEvent<ScanStatus, ScanSummary>('disk-scan-progress', onProgress, () => invoke<ScanSummary>('disk_scan', { root })),
  diskScanCancel: () => invoke<void>('disk_scan_cancel'),
  treeChildren: (id) => invoke<ChildrenPage>('tree_children', { nodeId: id }),
  diskDelete: (id) => invoke<DeleteResult>('disk_delete', { nodeId: id }),
  diskReveal: (id) => invoke<void>('disk_reveal', { nodeId: id }),
  healthCheck: (onFinding) => withEvent<Finding, Finding[]>('health-finding', onFinding, () => invoke<Finding[]>('health_check')),
  healthThrottle: () => invoke<Finding>('health_throttle'),
  startupList: () => invoke<StartupList>('startup_list'),
  startupSet: (id, enabled) => invoke<StartupEntry>('startup_set', { id, enabled }),
  openSettings: (page) => invoke<void>('open_settings', { page }),
  perfStart: (onTick, onError) =>
    serial(async () => {
      perfUnlisten.forEach((u) => u());
      perfUnlisten = [];
      // Ghi từng hàm gỡ ngay sau mỗi listen: listen thứ hai hỏng thì cái thứ nhất vẫn được gỡ.
      try {
        perfUnlisten.push(await listen<PerfTick>('perf-tick', (e) => onTick(e.payload)));
        perfUnlisten.push(await listen<string>('perf-error', (e) => onError(e.payload)));
        return await invoke<Sample[]>('perf_start');
      } catch (e) {
        perfUnlisten.forEach((u) => u());
        perfUnlisten = [];
        throw e;
      }
    }),
  perfStop: () =>
    serial(async () => {
      perfUnlisten.forEach((u) => u());
      perfUnlisten = [];
      await invoke<void>('perf_stop');
    }),
  appKill: (key) => invoke<KillResult>('app_kill', { key }),
  revealPath: (path) => invoke<void>('reveal_path', { path }),
  appIcon: (path) => invoke<string | null>('app_icon', { path }),
  copyText: (text) => navigator.clipboard.writeText(text),
};
