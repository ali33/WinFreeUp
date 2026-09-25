import { describe, expect, it, vi } from 'vitest';
import type { Api, CleanSummary, GroupScan } from '../api/types';
import { initialState, reducer } from './machine';
import { createStore } from './store';
import * as c from './controller';

const GB = 1024 ** 3;

function scan(id: string, risk: GroupScan['risk'], bytes: number, extra: Partial<GroupScan> = {}): GroupScan {
  return {
    id,
    risk,
    default_selected: risk === 'safe',
    result: { total_bytes: bytes, file_count: 1, top_items: [], estimated: false, notices: [] },
    error: null,
    ...extra,
  };
}

const SCANS: GroupScan[] = [
  scan('user_temp', 'safe', GB),
  scan('browser_cache', 'safe', GB, { result: { total_bytes: GB, file_count: 1, top_items: [], estimated: false, notices: ['browser_running:chrome'] } }),
  scan('delivery_opt', 'safe', 0, { result: null, error: 'Access is denied. (os error 5)' }),
  scan('recycle_bin', 'caution', GB),
];

function summary(ids: string[], extra: Partial<CleanSummary> = {}): CleanSummary {
  return {
    groups: ids.map((id) => ({ id, report: { bytes_freed: GB, files_deleted: 2, skipped_locked: 0, errors: [], dry_run: false }, error: null })),
    log_path: 'C:\\logs\\x.log',
    dry_run: false,
    log_write_failed: false,
    ...extra,
  };
}

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

function setup(over: Partial<Api> = {}) {
  const api: Api = {
    appInfo: vi.fn(async () => ({ version: '0.1.0', dry_run: false, system_drive: 'C:\\' })),
    diskFree: vi.fn(async () => 50 * GB),
    scanAll: vi.fn(async (onGroup: (g: GroupScan) => void) => {
      SCANS.forEach(onGroup);
      return SCANS;
    }),
    cancelScan: vi.fn(async () => {}),
    prepareRestorePoint: vi.fn(async () => ({ status: 'created' as const })),
    clean: vi.fn(async (ids: string[]) => summary(ids)),
    openLogFolder: vi.fn(async () => {}),
    ...over,
  };
  const store = createStore(reducer, initialState);
  const notify = vi.fn();
  const d = c.createDeps(api, store, notify);
  return { api, store, notify, d };
}

describe('khởi động', () => {
  it('đọc thông tin ứng dụng và dung lượng trống', async () => {
    const { d, store } = setup();
    await c.loadInitial(d);
    expect(store.getState().freeBefore).toBe(50 * GB);
    expect(store.getState().systemDrive).toBe('C:\\');
  });

  it('không đọc được dung lượng thì báo hổ phách, không treo vòng quay', async () => {
    const { d, store, notify } = setup({ diskFree: vi.fn(async () => Promise.reject('The device is not ready.')) });
    await c.loadInitial(d);
    expect(store.getState().freeFailed).toBe(true);
    expect(notify).toHaveBeenCalledWith('warning', 'Không đọc được dung lượng ổ đĩa: The device is not ready.');
  });
});

describe('quét', () => {
  it('xong thì sang xem trước; nhóm lỗi và trình duyệt đang mở lên băng hổ phách', async () => {
    const { d, store, notify } = setup();
    await c.startScan(d);
    expect(store.getState().phase).toBe('preview');
    expect(notify).toHaveBeenCalledWith('warning', 'Bản cập nhật chia sẻ: Access is denied. (os error 5)');
    expect(notify).toHaveBeenCalledWith('warning', expect.stringContaining('Chrome đang mở'));
  });

  it('quét hỏng hẳn thì băng đỏ và về màn Chào', async () => {
    const { d, store, notify } = setup({ scanAll: vi.fn(async () => Promise.reject('core crashed')) });
    await c.startScan(d);
    expect(store.getState().phase).toBe('welcome');
    expect(notify).toHaveBeenCalledWith('error', 'Không quét được: core crashed');
  });

  it('hủy: chờ lõi dừng hẳn rồi về Chào; kết quả và sự kiện đến muộn bị bỏ qua', async () => {
    const run = deferred<GroupScan[]>();
    let onGroup: (g: GroupScan) => void = () => {};
    const { d, store, api } = setup({
      scanAll: vi.fn((cb: (g: GroupScan) => void) => {
        onGroup = cb;
        return run.promise;
      }),
    });
    const scanning = c.startScan(d);
    expect(store.getState().phase).toBe('scanning');
    const cancelling = c.cancelScan(d);
    expect(store.getState().cancelling).toBe(true);
    onGroup(SCANS[0]);
    run.resolve(SCANS);
    await cancelling;
    await scanning;
    expect(api.cancelScan).toHaveBeenCalled();
    expect(store.getState().phase).toBe('welcome');
    expect(store.getState().groups).toEqual([]);
  });
});

describe('dọn', () => {
  it('chỉ nhóm An toàn: không hỏi, không tạo điểm khôi phục, đo dung lượng trước và sau', async () => {
    const free = [50 * GB, 50 * GB, 52 * GB];
    const { d, store, api } = setup({ diskFree: vi.fn(async () => free.shift() ?? 0) });
    await c.loadInitial(d);
    await c.startScan(d);
    await c.requestClean(d);
    expect(api.prepareRestorePoint).not.toHaveBeenCalled();
    expect(api.clean).toHaveBeenCalledWith(['user_temp', 'browser_cache'], false, expect.any(Function));
    expect(store.getState().phase).toBe('result');
    expect(store.getState().freeBefore).toBe(50 * GB);
    expect(store.getState().freeAfter).toBe(52 * GB);
  });

  it('có nhóm Cân nhắc: hỏi, đồng ý thì tạo điểm khôi phục rồi dọn', async () => {
    const { d, store, api } = setup();
    await c.startScan(d);
    c.toggle(d, 'recycle_bin');
    await c.requestClean(d);
    expect(store.getState().phase).toBe('confirm');
    expect(api.clean).not.toHaveBeenCalled();
    await c.acceptConfirm(d);
    expect(api.prepareRestorePoint).toHaveBeenCalledWith(['user_temp', 'browser_cache', 'recycle_bin'], false);
    expect(api.clean).toHaveBeenCalledTimes(1);
    expect(store.getState().phase).toBe('result');
  });

  it('không tạo được điểm khôi phục: chờ người dùng; dừng thì không dọn', async () => {
    const { d, store, api } = setup({ prepareRestorePoint: vi.fn(async () => ({ status: 'failed' as const, message: 'System Protection is off' })) });
    await c.startScan(d);
    c.toggle(d, 'recycle_bin');
    await c.requestClean(d);
    await c.acceptConfirm(d);
    expect(store.getState().phase).toBe('restorePoint');
    expect(api.clean).not.toHaveBeenCalled();
    c.abortAfterRestoreFailure(d);
    expect(store.getState().phase).toBe('preview');
    expect(api.clean).not.toHaveBeenCalled();
  });

  it('không tạo được điểm khôi phục: chọn dọn tiếp thì dọn', async () => {
    const { d, store, api } = setup({ prepareRestorePoint: vi.fn(async () => ({ status: 'failed' as const, message: 'x' })) });
    await c.startScan(d);
    c.toggle(d, 'recycle_bin');
    await c.requestClean(d);
    await c.acceptConfirm(d);
    await c.continueAfterRestoreFailure(d);
    expect(api.clean).toHaveBeenCalledTimes(1);
    expect(store.getState().phase).toBe('result');
  });

  it('lõi báo bận thì băng đỏ câu dễ hiểu và quay về xem trước', async () => {
    const { d, store, notify } = setup({ clean: vi.fn(async () => Promise.reject('busy')) });
    await c.startScan(d);
    await c.requestClean(d);
    expect(store.getState().phase).toBe('preview');
    expect(notify).toHaveBeenCalledWith('error', 'Không dọn được: WinFreeUp đang bận với thao tác trước, hãy đợi xong rồi thử lại.');
  });

  it('lỗi từng nhóm, lỗi từng file và nhật ký hỏng đều lên băng hổ phách', async () => {
    const s: CleanSummary = {
      groups: [
        { id: 'user_temp', report: { bytes_freed: 1, files_deleted: 1, skipped_locked: 0, errors: ['C:\\x: denied'], dry_run: false }, error: null },
        { id: 'browser_cache', report: null, error: 'panic: boom' },
      ],
      log_path: 'x',
      dry_run: false,
      log_write_failed: true,
    };
    const { d, notify } = setup({ clean: vi.fn(async () => s) });
    await c.startScan(d);
    await c.requestClean(d);
    expect(notify).toHaveBeenCalledWith('warning', 'File tạm của bạn: 1 lỗi, ví dụ: C:\\x: denied');
    expect(notify).toHaveBeenCalledWith('warning', 'Bộ nhớ đệm trình duyệt: panic: boom');
    expect(notify).toHaveBeenCalledWith('warning', 'Không ghi được đầy đủ nhật ký lượt dọn này.');
  });

  it('chế độ chạy thử truyền dryRun=true', async () => {
    const { d, api } = setup({ appInfo: vi.fn(async () => ({ version: '0.1.0', dry_run: true, system_drive: 'C:\\' })) });
    await c.loadInitial(d);
    await c.startScan(d);
    await c.requestClean(d);
    expect(api.clean).toHaveBeenCalledWith(expect.any(Array), true, expect.any(Function));
  });

  it('mở nhật ký hỏng thì báo hổ phách', async () => {
    const { d, notify } = setup({ openLogFolder: vi.fn(async () => Promise.reject('explorer.exe not found')) });
    await c.openLog(d);
    expect(notify).toHaveBeenCalledWith('warning', 'Không mở được thư mục nhật ký: explorer.exe not found');
  });
});
