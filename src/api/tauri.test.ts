import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke, listen, unlisten } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), unlisten: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

import { tauriApi } from './tauri';

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  unlisten.mockReset();
});

describe('tauriApi', () => {
  it('clean gửi tham số camelCase, chuyển tiếp sự kiện và luôn gỡ lắng nghe kể cả khi lỗi', async () => {
    let handler: (e: { payload: unknown }) => void = () => {};
    listen.mockImplementation(async (_name: string, h: typeof handler) => {
      handler = h;
      return unlisten;
    });
    invoke.mockImplementation(async () => {
      handler({ payload: { kind: 'started', id: 'user_temp' } });
      throw 'busy';
    });
    const events: unknown[] = [];
    await expect(tauriApi.clean(['user_temp'], true, (e) => events.push(e))).rejects.toBe('busy');
    expect(listen).toHaveBeenCalledWith('clean-progress', expect.any(Function));
    expect(invoke).toHaveBeenCalledWith('clean', { ids: ['user_temp'], dryRun: true });
    expect(events).toEqual([{ kind: 'started', id: 'user_temp' }]);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('scanAll lắng nghe scan-progress và trả kết quả cuối', async () => {
    listen.mockResolvedValue(unlisten);
    invoke.mockResolvedValue([{ id: 'a' }]);
    await expect(tauriApi.scanAll(() => {})).resolves.toEqual([{ id: 'a' }]);
    expect(listen).toHaveBeenCalledWith('scan-progress', expect.any(Function));
    expect(invoke).toHaveBeenCalledWith('scan_all');
    expect(unlisten).toHaveBeenCalled();
  });

  it('các lệnh đơn giản', async () => {
    invoke.mockResolvedValue(0);
    await tauriApi.diskFree();
    await tauriApi.prepareRestorePoint(['wu_download'], false);
    await tauriApi.cancelScan();
    await tauriApi.openLogFolder();
    await tauriApi.appInfo();
    expect(invoke.mock.calls).toEqual([
      ['disk_free', { drive: null }],
      ['prepare_restore_point', { ids: ['wu_download'], dryRun: false }],
      ['cancel_scan'],
      ['open_log_folder'],
      ['app_info'],
    ]);
  });
});
