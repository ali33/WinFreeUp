import { describe, expect, it } from 'vitest';
import { itemName } from './labels';
import { initialTState } from './reducer';
import { spokenLabel } from './RunPanel';
import { tt } from './strings';

// Rà lần 3 (LOW): spokenLabel tách từ toán tử ba ngôi lồng trong TinhChinhView, đặt cạnh runLabel.
describe('spokenLabel', () => {
  it('đang tạo điểm khôi phục ⇒ nhãn tạo điểm khôi phục', () => {
    expect(spokenLabel({ ...initialTState, phase: 'restorePoint', restore: null })).toBe(tt('tweaks.restore.creating'));
  });

  it('điểm khôi phục đã có kết quả (không còn null) ⇒ không phải nhãn tạo điểm khôi phục nữa', () => {
    expect(spokenLabel({ ...initialTState, phase: 'restorePoint', restore: { status: 'created' } })).toBe('');
  });

  it('đang chạy, có mục hiện tại ⇒ tên mục', () => {
    expect(spokenLabel({ ...initialTState, phase: 'running', run: { kind: 'apply', ids: ['ads_id'], current: 'ads_id', finished: [] } })).toBe(
      itemName('ads_id'),
    );
  });

  it('đang chạy, chưa có mục hiện tại (giữa hai sự kiện) ⇒ chuỗi rỗng', () => {
    expect(spokenLabel({ ...initialTState, phase: 'running', run: { kind: 'apply', ids: ['ads_id'], current: null, finished: [] } })).toBe('');
  });

  it('đang đọc lại ⇒ nhãn đọc lại', () => {
    expect(spokenLabel({ ...initialTState, phase: 'ready', reloading: true })).toBe(tt('tweaks.reloading'));
  });

  it('không thuộc trường hợp nào ⇒ chuỗi rỗng', () => {
    expect(spokenLabel({ ...initialTState, phase: 'ready' })).toBe('');
  });
});
