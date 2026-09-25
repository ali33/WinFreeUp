import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { Api, AppInfo, CleanEvent, CleanSummary, GroupScan, RestorePointStatus } from './types';

// Tauri đổi tham số camelCase ở JS sang snake_case ở Rust (dryRun → dry_run).
export const tauriApi: Api = {
  appInfo: () => invoke<AppInfo>('app_info'),
  diskFree: () => invoke<number>('disk_free', { drive: null }),
  async scanAll(onGroup) {
    const unlisten = await listen<GroupScan>('scan-progress', (e) => onGroup(e.payload));
    try {
      return await invoke<GroupScan[]>('scan_all');
    } finally {
      unlisten();
    }
  },
  cancelScan: () => invoke<void>('cancel_scan'),
  prepareRestorePoint: (ids, dryRun) => invoke<RestorePointStatus>('prepare_restore_point', { ids, dryRun }),
  async clean(ids, dryRun, onEvent) {
    const unlisten = await listen<CleanEvent>('clean-progress', (e) => onEvent(e.payload));
    try {
      return await invoke<CleanSummary>('clean', { ids, dryRun });
    } finally {
      unlisten();
    }
  },
  openLogFolder: () => invoke<void>('open_log_folder'),
};
