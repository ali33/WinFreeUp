import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { RestorePointStatus } from '../../api/types';
import type { ReadResult, RunReport, TweakApi, TweakEvent } from './types';

// Lệnh do Task Tích hợp đăng ký trong src-tauri. Tauri đổi camelCase ⇒ snake_case (allUsers → all_users).
async function withProgress(run: () => Promise<RunReport>, onEvent: (e: TweakEvent) => void): Promise<RunReport> {
  const unlisten = await listen<TweakEvent>('tweak-progress', (e) => onEvent(e.payload));
  try {
    return await run();
  } finally {
    unlisten();
  }
}

export const tauriTweakApi: TweakApi = {
  read: () => invoke<ReadResult>('tweaks_read'),
  prepareRestorePoint: () => invoke<RestorePointStatus>('tweaks_prepare_restore_point'),
  apply: (ids, allUsers, onEvent) => withProgress(() => invoke<RunReport>('tweaks_apply', { ids, allUsers }), onEvent),
  revert: (ids, onEvent) => withProgress(() => invoke<RunReport>('tweaks_revert', { ids }), onEvent),
  restartExplorer: () => invoke<void>('tweaks_restart_explorer'),
};
