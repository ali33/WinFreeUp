import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke, listen, unlisten } = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), unlisten: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('@tauri-apps/api/event', () => ({ listen }));

import { khamMayTauriApi as api } from './tauri';

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
  unlisten.mockReset();
  listen.mockResolvedValue(unlisten);
});

describe('khamMayTauriApi', () => {
  it('quét ổ: nghe disk-scan-progress trước khi gọi, luôn gỡ kể cả khi lỗi', async () => {
    let handler: (e: { payload: unknown }) => void = () => {};
    listen.mockImplementation(async (_n: string, h: typeof handler) => {
      handler = h;
      return unlisten;
    });
    invoke.mockImplementation(async () => {
      handler({ payload: { files: 1, bytes: 2, current: 'C:\\a', percent: null } });
      throw 'busy';
    });
    const seen: unknown[] = [];
    await expect(api.diskScan('C:\\', (s) => seen.push(s))).rejects.toBe('busy');
    expect(listen).toHaveBeenCalledWith('disk-scan-progress', expect.any(Function));
    expect(invoke).toHaveBeenCalledWith('disk_scan', { root: 'C:\\' });
    expect(seen).toHaveLength(1);
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('khám nhanh nghe health-finding và trả danh sách cuối', async () => {
    invoke.mockResolvedValue([{ id: 'uptime' }]);
    await expect(api.healthCheck(() => {})).resolves.toEqual([{ id: 'uptime' }]);
    expect(listen).toHaveBeenCalledWith('health-finding', expect.any(Function));
    expect(invoke).toHaveBeenCalledWith('health_check');
  });

  it('theo dõi hiệu năng: nghe tới khi dừng, bắt đầu lại thì gỡ lắng nghe cũ', async () => {
    invoke.mockResolvedValue([]);
    await api.perfStart(() => {}, () => {});
    expect(listen.mock.calls.map((c) => c[0])).toEqual(['perf-tick', 'perf-error']);
    expect(unlisten).not.toHaveBeenCalled();
    await api.perfStart(() => {}, () => {});
    expect(unlisten).toHaveBeenCalledTimes(2);
    await api.perfStop();
    expect(unlisten).toHaveBeenCalledTimes(4);
    expect(invoke).toHaveBeenLastCalledWith('perf_stop');
  });

  it('start → stop → start gọi liền nhau (StrictMode) chạy lần lượt, không sót lắng nghe', async () => {
    invoke.mockResolvedValue([]);
    const order: string[] = [];
    listen.mockImplementation(async (name: string) => {
      order.push(`listen:${name}`);
      await new Promise((r) => setTimeout(r, 5));
      return () => order.push(`unlisten:${name}`);
    });
    invoke.mockImplementation(async (cmd: string) => {
      order.push(cmd);
      return [];
    });
    await Promise.all([api.perfStart(() => {}, () => {}), api.perfStop(), api.perfStart(() => {}, () => {})]);
    expect(order).toEqual([
      'listen:perf-tick',
      'listen:perf-error',
      'perf_start',
      'unlisten:perf-tick',
      'unlisten:perf-error',
      'perf_stop',
      'listen:perf-tick',
      'listen:perf-error',
      'perf_start',
    ]);
    await api.perfStop();
  });

  it('bắt đầu theo dõi hỏng thì không để lại lắng nghe', async () => {
    invoke.mockRejectedValue('PDH open: 0xC0000BB8');
    await expect(api.perfStart(() => {}, () => {})).rejects.toBe('PDH open: 0xC0000BB8');
    expect(unlisten).toHaveBeenCalledTimes(2);
  });

  it('các lệnh đơn giản gửi đúng tên và tham số', async () => {
    invoke.mockResolvedValue(null);
    await api.diskVolumes();
    await api.diskScanCancel();
    await api.treeChildren(7);
    await api.diskDelete(8);
    await api.diskReveal(9);
    await api.healthThrottle();
    await api.startupList();
    await api.startupSet('hkcu_run:OneDrive', false);
    await api.openSettings('power');
    await api.appKill('c:\\x.exe');
    await api.revealPath('C:\\x.exe');
    await api.appIcon('C:\\x.exe');
    expect(invoke.mock.calls).toEqual([
      ['disk_volumes'],
      ['disk_scan_cancel'],
      ['tree_children', { nodeId: 7 }],
      ['disk_delete', { nodeId: 8 }],
      ['disk_reveal', { nodeId: 9 }],
      ['health_throttle'],
      ['startup_list'],
      ['startup_set', { id: 'hkcu_run:OneDrive', enabled: false }],
      ['open_settings', { page: 'power' }],
      ['app_kill', { key: 'c:\\x.exe' }],
      ['reveal_path', { path: 'C:\\x.exe' }],
      ['app_icon', { path: 'C:\\x.exe' }],
    ]);
  });
});
