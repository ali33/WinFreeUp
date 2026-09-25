import { describe, expect, it, vi } from 'vitest';
import { createStore } from '../../state/store';
import * as c from './controller';
import { initialTState, reducer } from './reducer';
import { readResult, report } from './testdata';
import type { RunReport, TweakApi } from './types';

function fakeApi(over: Partial<TweakApi> = {}): TweakApi {
  return {
    read: vi.fn(async () => readResult()),
    prepareRestorePoint: vi.fn(async () => ({ status: 'created' as const })),
    apply: vi.fn(async (ids: string[], _all: boolean, onEvent) => {
      ids.forEach((id, index) => {
        onEvent({ kind: 'started', id, index, total: ids.length });
        onEvent({ kind: 'finished', outcome: { id, status: 'applied', errors: [], store_opened: [] } });
      });
      return report({ outcomes: ids.map((id) => ({ id, status: 'applied' as const, errors: [], store_opened: [] })) });
    }),
    revert: vi.fn(async () => report()),
    restartExplorer: vi.fn(async () => {}),
    ...over,
  };
}

function setup(api = fakeApi()) {
  const store = createStore(reducer, initialTState);
  const notify = vi.fn();
  return { d: c.createTDeps(api, store, notify), store, notify, api };
}

describe('bộ điều khiển Tinh chỉnh', () => {
  it('áp dụng: điểm khôi phục ⇒ chỉ gửi mục chưa ở trạng thái đích ⇒ đọc lại', async () => {
    const { d, store, api } = setup();
    await c.load(d);
    await c.requestApply(d);
    expect(api.prepareRestorePoint).toHaveBeenCalledTimes(1);
    expect(api.apply).toHaveBeenCalledWith(['ads_id'], false, expect.any(Function));
    expect(store.getState().phase).toBe('done');
    expect(store.getState().run?.finished).toEqual(['ads_id']);
    expect(api.read).toHaveBeenCalledTimes(2);
  });

  it('điểm khôi phục hỏng ⇒ không áp dụng cho tới khi người dùng chọn tiếp', async () => {
    const { d, store, api } = setup(fakeApi({ prepareRestorePoint: vi.fn(async () => ({ status: 'failed' as const, message: 'System Protection is off' })) }));
    await c.load(d);
    await c.requestApply(d);
    expect(api.apply).not.toHaveBeenCalled();
    expect(store.getState().restore).toEqual({ status: 'failed', message: 'System Protection is off' });
    await c.continueAfterRestoreFailure(d);
    expect(api.apply).toHaveBeenCalledTimes(1);
  });

  it('lệnh điểm khôi phục ném lỗi ⇒ coi như hỏng, giữ nguyên văn', async () => {
    const { d, store } = setup(fakeApi({ prepareRestorePoint: vi.fn(async () => Promise.reject('rpc down')) }));
    await c.load(d);
    await c.requestApply(d);
    expect(store.getState().restore).toEqual({ status: 'failed', message: 'rpc down' });
  });

  it('đọc hỏng ⇒ băng đỏ nguyên văn', async () => {
    const { d, notify } = setup(fakeApi({ read: vi.fn(async () => Promise.reject(new Error('Access is denied.'))) }));
    await c.load(d);
    expect(notify).toHaveBeenCalledWith('error', 'Không đọc được trạng thái máy: Access is denied.');
  });

  it('file hoàn tác hỏng ⇒ băng hổ phách', async () => {
    const { d, notify } = setup(fakeApi({ read: vi.fn(async () => readResult(undefined, { notices: ['undo_corrupt:x.json: EOF'] })) }));
    await c.load(d);
    expect(notify).toHaveBeenCalledWith('warning', expect.stringContaining('dùng giá trị mặc định của Windows'));
    expect(notify).toHaveBeenCalledWith('warning', expect.stringContaining('x.json: EOF'));
  });

  it('áp dụng ném lỗi ⇒ băng đỏ, về danh sách, vẫn đọc lại trạng thái thật', async () => {
    const { d, store, notify, api } = setup(fakeApi({ apply: vi.fn(async () => Promise.reject('busy')) }));
    await c.load(d);
    await c.requestApply(d);
    expect(notify).toHaveBeenCalledWith('error', expect.stringContaining('Đang có thao tác khác'));
    expect(store.getState().phase).toBe('ready');
    expect(api.read).toHaveBeenCalledTimes(2);
  });

  it('hoàn tác chỉ gửi mục có gì để trả; «Cài lại từ Store» hoàn tác đúng một app', async () => {
    const { d, api } = setup();
    await c.load(d);
    // Lần đầu không tích sẵn app (quyết định người dùng) ⇒ tự tích app đã gỡ trước khi hoàn tác.
    c.toggle(d, 'app_clipchamp');
    await c.revertSelected(d);
    expect(api.revert).toHaveBeenCalledWith(['app_clipchamp'], expect.any(Function));
    c.dismissResult(d);
    await c.reinstall(d, 'app_clipchamp');
    expect(api.revert).toHaveBeenLastCalledWith(['app_clipchamp'], expect.any(Function));
  });

  it('không chạy chồng: đang chạy thì bấm tiếp không gọi lệnh', async () => {
    let release: () => void = () => {};
    const api = fakeApi({ revert: vi.fn(() => new Promise<RunReport>((r) => (release = () => r(report())))) });
    const { d } = setup(api);
    await c.load(d);
    const first = c.reinstall(d, 'app_clipchamp');
    await c.reinstall(d, 'app_clipchamp');
    expect(api.revert).toHaveBeenCalledTimes(1);
    release();
    await first;
  });

  it('khởi động lại Explorer hỏng ⇒ băng đỏ nguyên văn', async () => {
    const { d, notify } = setup(fakeApi({ restartExplorer: vi.fn(async () => Promise.reject('taskkill: Access is denied.')) }));
    await c.restartExplorer(d);
    expect(notify).toHaveBeenCalledWith('error', 'Không khởi động lại được Explorer: taskkill: Access is denied.');
  });

  // Lệch có chủ ý (rà Task 10): ba test dưới.
  it('bấm đúp «Áp dụng» chỉ tạo điểm khôi phục một lần', async () => {
    const { d, api } = setup();
    await c.load(d);
    const first = c.requestApply(d);
    await c.requestApply(d);
    await first;
    expect(api.prepareRestorePoint).toHaveBeenCalledTimes(1);
    expect(api.apply).toHaveBeenCalledTimes(1);
  });

  it('bấm đúp «Đồng ý» ở hộp xác nhận chỉ tạo điểm khôi phục một lần', async () => {
    const { d, store, api } = setup();
    await c.load(d);
    c.preset(d, 'recommended');
    await c.requestApply(d);
    expect(store.getState().phase).toBe('confirm');
    const first = c.acceptConfirm(d);
    await c.acceptConfirm(d);
    await first;
    expect(api.prepareRestorePoint).toHaveBeenCalledTimes(1);
    expect(api.apply).toHaveBeenCalledTimes(1);
  });

  it('báo cáo thiếu notices ⇒ không báo nhầm «Áp dụng không xong»', async () => {
    const bare = { outcomes: [], restart: 'none' } as unknown as RunReport;
    const { d, store, notify } = setup(fakeApi({ apply: vi.fn(async () => bare) }));
    await c.load(d);
    await c.requestApply(d);
    expect(store.getState().phase).toBe('done');
    expect(notify).not.toHaveBeenCalledWith('error', expect.anything());
  });
});
