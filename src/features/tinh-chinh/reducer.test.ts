import { describe, expect, it } from 'vitest';
import { initialTState, reducer, type TAction, type TState } from './reducer';
import { readResult, report, sample, tw } from './testdata';
import type { TweakView } from './types';

const run = (actions: TAction[], from: TState = initialTState) => actions.reduce(reducer, from);
const loaded = () => run([{ type: 'LOAD_STARTED' }, { type: 'LOADED', data: readResult() }]);

describe('đọc trạng thái', () => {
  // Lệch có chủ ý (quyết định người dùng): lần đầu chỉ tích mục quyền riêng tư của mức Cơ bản.
  it('lần đầu tích sẵn mục quyền riêng tư của mức Cơ bản', () => {
    const s = loaded();
    expect(s.phase).toBe('ready');
    expect(s.selected).toEqual(['ads_id']);
  });

  it('lần đầu không tích sẵn mục gỡ app nào', () => {
    const s = loaded();
    const bloat = new Set(sample().filter((t) => t.group === 'bloatware').map((t) => t.id));
    expect(s.selected.filter((id) => bloat.has(id))).toEqual([]);
  });

  it('đọc lại giữ lựa chọn người dùng, bỏ mục không còn chọn được, phủ mờ chứ không xoá dữ liệu', () => {
    let s = run([{ type: 'TOGGLE', id: 'svc_diagtrack' }], loaded());
    s = reducer(s, { type: 'LOAD_STARTED' });
    expect(s.reloading).toBe(true);
    expect(s.data).not.toBeNull();
    const again = sample().map((t) => (t.id === 'svc_diagtrack' ? { ...t, status: 'managed' as const } : t));
    s = reducer(s, { type: 'LOADED', data: readResult(again) });
    expect(s.reloading).toBe(false);
    expect(s.selected).toEqual(['ads_id']);
  });

  it('đọc hỏng ⇒ dọn số cũ và giữ thông điệp', () => {
    const s = reducer(loaded(), { type: 'LOAD_FAILED', message: 'Access is denied.' });
    expect(s.data).toBeNull();
    expect(s.loadError).toBe('Access is denied.');
  });
});

describe('chọn mục', () => {
  it('không tích được mục không hỗ trợ', () => {
    const s = loaded();
    expect(reducer(s, { type: 'TOGGLE', id: 'recall_off' })).toBe(s);
  });

  it('bấm mức sẵn thay toàn bộ lựa chọn', () => {
    const s = run([{ type: 'TOGGLE', id: 'ads_id' }, { type: 'PRESET', level: 'recommended' }], loaded());
    expect(s.selected).toEqual(['ads_id', 'app_clipchamp', 'svc_diagtrack', 'app_weather']);
  });

  it('bấm mức Cơ bản vẫn tích cả mục gỡ app', () => {
    const s = run([{ type: 'PRESET', level: 'basic' }], loaded());
    expect(s.selected).toEqual(['ads_id', 'app_clipchamp']);
  });
});

describe('luồng áp dụng', () => {
  it('chỉ mục an toàn ⇒ thẳng tới điểm khôi phục, rồi chạy', () => {
    let s = reducer(loaded(), { type: 'REQUEST_APPLY' });
    expect(s.phase).toBe('restorePoint');
    expect(s.restore).toBeNull();
    s = run([{ type: 'RESTORE_RESULT', status: { status: 'created' } }, { type: 'RUN_STARTED', kind: 'apply', ids: ['ads_id'] }], s);
    expect(s.phase).toBe('running');
    s = run(
      [
        { type: 'RUN_EVENT', event: { kind: 'started', id: 'ads_id', index: 0, total: 1 } },
        { type: 'RUN_EVENT', event: { kind: 'finished', outcome: { id: 'ads_id', status: 'applied', errors: [], store_opened: [] } } },
        { type: 'RUN_DONE', report: report({ restart: 'explorer' }) },
      ],
      s,
    );
    expect(s.phase).toBe('done');
    expect(s.run?.finished).toEqual(['ads_id']);
    expect(s.report?.restart).toBe('explorer');
    expect(reducer(s, { type: 'DISMISS_RESULT' }).phase).toBe('ready');
  });

  it('có mục caution ⇒ hỏi trước; huỷ thì quay lại', () => {
    const s = run([{ type: 'PRESET', level: 'recommended' }, { type: 'REQUEST_APPLY' }], loaded());
    expect(s.phase).toBe('confirm');
    expect(reducer(s, { type: 'CONFIRM_CANCELLED' }).phase).toBe('ready');
    expect(reducer(s, { type: 'CONFIRM_ACCEPTED' }).phase).toBe('restorePoint');
  });

  it('bật «mọi tài khoản» ⇒ hỏi trước cả khi toàn mục an toàn', () => {
    const s = run([{ type: 'SET_ALL_USERS', on: true }, { type: 'TOGGLE', id: 'app_weather' }, { type: 'REQUEST_APPLY' }], loaded());
    expect(s.phase).toBe('confirm');
  });

  // Lệch có chủ ý (rà Task 11, L3): chạy bằng tài khoản admin khác ⇒ luôn hỏi lại trước khi áp dụng.
  it('chạy bằng tài khoản admin khác ⇒ luôn hỏi trước', () => {
    const data = readResult(undefined, { system: { build: 26200, edition: 'Pro', managed: false, other_user: true } });
    const s = run([{ type: 'LOAD_STARTED' }, { type: 'LOADED', data }, { type: 'REQUEST_APPLY' }]);
    expect(s.phase).toBe('confirm');
  });

  it('không tạo được điểm khôi phục ⇒ chờ người dùng chọn tiếp hay dừng', () => {
    let s = run([{ type: 'REQUEST_APPLY' }, { type: 'RESTORE_RESULT', status: { status: 'failed', message: 'System Protection is off' } }], loaded());
    expect(s.phase).toBe('restorePoint');
    expect(reducer(s, { type: 'RUN_STARTED', kind: 'apply', ids: ['ads_id'] })).toBe(s);
    expect(reducer(s, { type: 'RESTORE_ABORT' }).phase).toBe('ready');
    s = run([{ type: 'RESTORE_CONTINUE' }, { type: 'RUN_STARTED', kind: 'apply', ids: ['ads_id'] }], s);
    expect(s.phase).toBe('running');
  });

  it('không có gì để đổi ⇒ nút Áp dụng không làm gì', () => {
    const s = run([{ type: 'TOGGLE', id: 'ads_id' }], loaded()); // bỏ mục duy nhất đang tích
    expect(reducer(s, { type: 'REQUEST_APPLY' })).toBe(s);
  });

  it('đang chạy thì mọi thao tác chọn bị bỏ qua', () => {
    const s = run([{ type: 'RUN_STARTED', kind: 'revert', ids: ['app_clipchamp'] }], loaded());
    expect(s.phase).toBe('running');
    expect(reducer(s, { type: 'TOGGLE', id: 'ads_id' })).toBe(s);
    expect(reducer(s, { type: 'PRESET', level: 'basic' })).toBe(s);
    expect(reducer(s, { type: 'REQUEST_APPLY' })).toBe(s);
    expect(reducer(s, { type: 'RUN_FAILED' }).phase).toBe('ready');
  });

  // Lệch có chủ ý (rà Task 10): hoàn tác cũng bị chặn khi đang đọc lại, như REQUEST_APPLY.
  it('đang đọc lại thì không bắt đầu hoàn tác', () => {
    const s = reducer(loaded(), { type: 'LOAD_STARTED' });
    expect(s.reloading).toBe(true);
    expect(reducer(s, { type: 'RUN_STARTED', kind: 'revert', ids: ['app_clipchamp'] })).toBe(s);
  });
});

// Lệch có chủ ý (yêu cầu sau rà Task 11): mục chỉ hoàn tác tích được, và đọc lại không bỏ nó khỏi lựa chọn.
describe('mục chỉ hoàn tác', () => {
  const data = readResult([...sample(), tw('old_tweak', { status: 'unsupported', reason: 'build_max:19045', has_undo: true } as Partial<TweakView>)]);
  it('tích được và giữ qua lần đọc lại', () => {
    let s = run([{ type: 'LOAD_STARTED' }, { type: 'LOADED', data }]);
    expect(s.selected).not.toContain('old_tweak');
    s = reducer(s, { type: 'TOGGLE', id: 'old_tweak' });
    expect(s.selected).toContain('old_tweak');
    s = run([{ type: 'LOAD_STARTED' }, { type: 'LOADED', data }], s);
    expect(s.selected).toContain('old_tweak');
  });
});
