import { describe, expect, it } from 'vitest';
import { initialState, reducer, type Action, type State } from './machine';
import { confirmKind, dryRunBytes, isConfirmWord, reclaimedBytes, selectedBytes } from './selectors';
import type { GroupScan } from '../api/types';

const GB = 1024 ** 3;

function scan(id: string, risk: GroupScan['risk'], bytes: number, def = risk === 'safe', error: string | null = null): GroupScan {
  return {
    id,
    risk,
    default_selected: def,
    result: error ? null : { total_bytes: bytes, file_count: 1, top_items: [], estimated: false, notices: [] },
    error,
  };
}

function run(s: State, ...actions: Action[]): State {
  return actions.reduce(reducer, s);
}

const SCANS = [
  scan('user_temp', 'safe', 1 * GB),
  scan('system_temp', 'safe', 0),
  scan('browser_cache', 'safe', 0, true, 'disk error'),
  scan('recycle_bin', 'caution', GB / 2),
  scan('windows_old', 'risky', 10 * GB),
];

function preview(): State {
  return run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_FINISHED', groups: SCANS });
}

describe('quét', () => {
  it('bắt đầu quét thì mọi nhóm của catalog đang chờ', () => {
    const s = run(initialState, { type: 'SCAN_STARTED' });
    expect(s.phase).toBe('scanning');
    expect(s.groups).toHaveLength(9);
    expect(s.groups.every((g) => g.status === 'pending')).toBe(true);
  });

  it('nhóm xong nào hiện nhóm đó', () => {
    const s = run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_GROUP_DONE', group: SCANS[0] });
    expect(s.groups.find((g) => g.id === 'user_temp')?.status).toBe('done');
    expect(s.groups.find((g) => g.id === 'system_temp')?.status).toBe('pending');
  });

  it('kết thúc quét: tích sẵn nhóm mặc định có dung lượng, bỏ nhóm 0 byte và nhóm lỗi', () => {
    const s = preview();
    expect(s.phase).toBe('preview');
    expect(s.selected).toEqual(['user_temp']);
    expect(s.groups.find((g) => g.id === 'browser_cache')?.status).toBe('error');
  });

  it('sự kiện đến muộn sau khi hủy bị bỏ qua', () => {
    const aborted = run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_CANCEL_REQUESTED' }, { type: 'SCAN_ABORTED' });
    expect(aborted.phase).toBe('welcome');
    expect(reducer(aborted, { type: 'SCAN_GROUP_DONE', group: SCANS[0] })).toBe(aborted);
    expect(reducer(aborted, { type: 'SCAN_FINISHED', groups: SCANS })).toBe(aborted);
  });
});

describe('chọn nhóm', () => {
  it('tích/bỏ tích giữ thứ tự nhóm và tổng luôn đúng', () => {
    let s = preview();
    expect(selectedBytes(s)).toBe(GB);
    s = reducer(s, { type: 'TOGGLE', id: 'recycle_bin' });
    expect(s.selected).toEqual(['user_temp', 'recycle_bin']);
    expect(selectedBytes(s)).toBe(1.5 * GB);
    s = reducer(s, { type: 'TOGGLE', id: 'user_temp' });
    expect(s.selected).toEqual(['recycle_bin']);
  });

  it('không tích được nhóm lỗi hoặc 0 byte', () => {
    const s = preview();
    expect(reducer(s, { type: 'TOGGLE', id: 'browser_cache' })).toBe(s);
    expect(reducer(s, { type: 'TOGGLE', id: 'system_temp' })).toBe(s);
  });
});

describe('xác nhận theo mức rủi ro', () => {
  it('chỉ nhóm An toàn thì vào dọn ngay, không qua xác nhận', () => {
    const s = reducer(preview(), { type: 'REQUEST_CLEAN' });
    expect(s.phase).toBe('cleaning');
    expect(s.cleaning.map((r) => r.id)).toEqual(['user_temp']);
  });

  it('có Cân nhắc thì hỏi; đồng ý thì tạo điểm khôi phục rồi dọn', () => {
    let s = run(preview(), { type: 'TOGGLE', id: 'recycle_bin' }, { type: 'REQUEST_CLEAN' });
    expect(s.phase).toBe('confirm');
    expect(confirmKind(s.groups, s.selected)).toBe('caution');
    s = reducer(s, { type: 'CONFIRM_ACCEPTED' });
    expect(s.phase).toBe('restorePoint');
    expect(s.restore).toBeNull();
    s = reducer(s, { type: 'RESTORE_RESULT', status: { status: 'created' } });
    expect(s.phase).toBe('cleaning');
  });

  it('có Rủi ro thì confirmKind là risky; quay lại thì về xem trước', () => {
    const s = run(preview(), { type: 'TOGGLE', id: 'windows_old' }, { type: 'REQUEST_CLEAN' });
    expect(confirmKind(s.groups, s.selected)).toBe('risky');
    expect(reducer(s, { type: 'CONFIRM_CANCELLED' }).phase).toBe('preview');
  });

  it('không tạo được điểm khôi phục thì đứng lại cho người dùng chọn', () => {
    const failed = run(
      preview(),
      { type: 'TOGGLE', id: 'recycle_bin' },
      { type: 'REQUEST_CLEAN' },
      { type: 'CONFIRM_ACCEPTED' },
      { type: 'RESTORE_RESULT', status: { status: 'failed', message: 'off' } },
    );
    expect(failed.phase).toBe('restorePoint');
    expect(reducer(failed, { type: 'RESTORE_CONTINUE' }).phase).toBe('cleaning');
    expect(reducer(failed, { type: 'RESTORE_ABORT' }).phase).toBe('preview');
  });

  it('không có gì được chọn thì bấm Dọn không làm gì', () => {
    const s = reducer(preview(), { type: 'TOGGLE', id: 'user_temp' });
    expect(reducer(s, { type: 'REQUEST_CLEAN' })).toBe(s);
  });
});

describe('dọn và kết quả', () => {
  it('sự kiện tiến độ cập nhật từng dòng; xong thì sang kết quả', () => {
    let s = reducer(preview(), { type: 'REQUEST_CLEAN' });
    s = reducer(s, { type: 'CLEAN_EVENT', event: { kind: 'started', id: 'user_temp' } });
    expect(s.cleaning[0].status).toBe('running');
    s = reducer(s, { type: 'CLEAN_EVENT', event: { kind: 'percent', id: 'user_temp', percent: 40 } });
    expect(s.cleaning[0].percent).toBe(40);
    const result = { id: 'user_temp', report: { bytes_freed: GB, files_deleted: 3, skipped_locked: 1, errors: [], dry_run: false }, error: null };
    s = reducer(s, { type: 'CLEAN_EVENT', event: { kind: 'finished', result } });
    expect(s.cleaning[0].status).toBe('done');
    s = reducer(s, { type: 'CLEAN_DONE', summary: { groups: [result], log_path: 'x.log', dry_run: false, log_write_failed: false } });
    expect(s.phase).toBe('result');
    expect(s.freeAfter).toBeNull();
    s = reducer(s, { type: 'FREE_AFTER', bytes: 123 });
    expect(s.freeAfter).toBe(123);
  });

  it('dọn hỏng thì quay về xem trước', () => {
    const s = run(preview(), { type: 'REQUEST_CLEAN' }, { type: 'CLEAN_FAILED' });
    expect(s.phase).toBe('preview');
  });

  it('đo lại "trước" ngay khi dọn thất bại: freeBefore về null, không giữ số cũ', () => {
    let s = run(preview(), { type: 'FREE_SPACE', bytes: 10 * GB }, { type: 'REQUEST_CLEAN' });
    s = reducer(s, { type: 'FREE_BEFORE_FAILED' });
    expect(s.freeBefore).toBeNull();
    expect(s.freeFailed).toBe(true);
  });

  it('freeBefore null lúc CLEAN_DONE thì đánh dấu freeAfterFailed để màn Kết quả không kẹt vòng quay dù đo "sau" thành công', () => {
    let s = run(preview(), { type: 'FREE_SPACE', bytes: 10 * GB }, { type: 'REQUEST_CLEAN' }, { type: 'FREE_BEFORE_FAILED' });
    const result = { id: 'user_temp', report: { bytes_freed: GB, files_deleted: 3, skipped_locked: 1, errors: [], dry_run: false }, error: null };
    s = reducer(s, { type: 'CLEAN_DONE', summary: { groups: [result], log_path: 'x.log', dry_run: false, log_write_failed: false } });
    expect(s.phase).toBe('result');
    expect(s.freeAfterFailed).toBe(true);
    s = reducer(s, { type: 'FREE_AFTER', bytes: 999 });
    expect(s.freeAfter).toBe(999);
    expect(s.freeAfterFailed).toBe(true);
  });

  it('freeBefore còn hợp lệ lúc CLEAN_DONE thì freeAfterFailed vẫn khởi động false như cũ', () => {
    const s = run(preview(), { type: 'FREE_SPACE', bytes: 10 * GB }, { type: 'REQUEST_CLEAN' }, { type: 'CLEAN_DONE', summary: { groups: [], log_path: 'x.log', dry_run: false, log_write_failed: false } });
    expect(s.freeAfterFailed).toBe(false);
  });

  it('về đầu giữ cờ chạy thử và ổ hệ thống', () => {
    const s = run(initialState, { type: 'APP_INFO', dryRun: true, systemDrive: 'D:\\' }, { type: 'SCAN_STARTED' }, { type: 'RESET' });
    expect(s.phase).toBe('welcome');
    expect(s.dryRun).toBe(true);
    expect(s.systemDrive).toBe('D:\\');
  });
});

describe('selectors', () => {
  // Review Focus 1
  it('chấp nhận XOA gõ theo thói quen tiếng Việt', () => {
    for (const ok of ['XOA', 'xoa', 'xóa', 'Xóa', 'XÓA', '  XOA  ', 'xoá']) {
      expect(isConfirmWord(ok), ok).toBe(true);
    }
    for (const bad of ['', 'XO', 'XOAA', 'X O A', 'xóa đi']) {
      expect(isConfirmWord(bad), bad).toBe(false);
    }
  });

  // Review Focus 5
  it('dung lượng lấy lại không bao giờ âm', () => {
    expect(reclaimedBytes(100, 50)).toBe(0);
    expect(reclaimedBytes(100, 300)).toBe(200);
    expect(reclaimedBytes(null, 300)).toBeNull();
  });

  it('chạy thử cộng số byte lẽ ra xóa', () => {
    const r = (b: number) => ({ id: 'a', report: { bytes_freed: b, files_deleted: 1, skipped_locked: 0, errors: [], dry_run: true }, error: null });
    expect(dryRunBytes({ groups: [r(5), r(7), { id: 'x', report: null, error: 'e' }], log_path: '', dry_run: true, log_write_failed: false })).toBe(12);
  });
});
