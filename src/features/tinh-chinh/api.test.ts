import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke, listen, unlisten } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), unlisten: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

import { tauriTweakApi } from './api';

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  unlisten.mockReset();
});

describe('tauriTweakApi', () => {
  it('apply gửi allUsers, chuyển tiếp tweak-progress và luôn gỡ lắng nghe kể cả khi lỗi', async () => {
    let handler: (e: { payload: unknown }) => void = () => {};
    listen.mockImplementation(async (_n: string, h: typeof handler) => {
      handler = h;
      return unlisten;
    });
    invoke.mockImplementation(async () => {
      handler({ payload: { kind: 'started', id: 'ads_id', index: 0, total: 1 } });
      throw 'busy';
    });
    const events: unknown[] = [];
    await expect(tauriTweakApi.apply(['ads_id'], true, (e) => events.push(e))).rejects.toBe('busy');
    expect(listen).toHaveBeenCalledWith('tweak-progress', expect.any(Function));
    expect(invoke).toHaveBeenCalledWith('tweaks_apply', { ids: ['ads_id'], allUsers: true });
    expect(events).toEqual([{ kind: 'started', id: 'ads_id', index: 0, total: 1 }]);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('revert và các lệnh đơn giản', async () => {
    listen.mockResolvedValue(unlisten);
    invoke.mockResolvedValue({ outcomes: [], restart: 'none', notices: [] });
    await tauriTweakApi.revert(['app_clipchamp'], () => {});
    await tauriTweakApi.read();
    await tauriTweakApi.prepareRestorePoint();
    await tauriTweakApi.restartExplorer();
    expect(invoke.mock.calls).toEqual([
      ['tweaks_revert', { ids: ['app_clipchamp'] }],
      ['tweaks_read'],
      ['tweaks_prepare_restore_point'],
      ['tweaks_restart_explorer'],
    ]);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
