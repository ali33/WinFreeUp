import { describe, expect, it } from 'vitest';
import catalogVi from './catalog.vi.json';
import ui from './vi.json';
import { itemName, outcomeText, reasonText, statusText } from './labels';
import { tw } from './testdata';

describe('nhãn', () => {
  it('trạng thái app nói «gỡ», quyền riêng tư nói «áp dụng»', () => {
    expect(statusText(tw('a', { group: 'bloatware', status: 'applied' }))).toBe('Đã gỡ');
    expect(statusText(tw('a', { group: 'bloatware' }))).toBe('Chưa gỡ');
    expect(statusText(tw('a'))).toBe('Chưa áp dụng');
    expect(statusText(tw('a', { status: 'partial' }))).toBe('Một phần');
  });

  it('lý do không hỗ trợ đọc được', () => {
    expect(reasonText('build_min:26100')).toBe('cần Windows 11 24H2 trở lên');
    expect(reasonText('build_min:22000')).toBe('cần Windows 11');
    expect(reasonText('build_min:19041')).toBe('cần Windows build 19041 trở lên');
    expect(reasonText('build_max:19045')).toBe('chỉ dành cho Windows 10');
    expect(reasonText('edition')).toBe('không áp dụng cho bản Windows này');
    expect(reasonText('la_chua_biet')).toBe('la_chua_biet');
    expect(statusText(tw('r', { status: 'unsupported', reason: 'build_min:26100' } as never))).toBe(
      'Không hỗ trợ trên máy này — cần Windows 11 24H2 trở lên',
    );
  });

  it('tên mục lấy từ danh mục tiếng Việt', () => {
    expect(itemName('app_clipchamp')).toBe('Clipchamp');
  });

  it('kết quả từng mục', () => {
    const o = (status: 'applied' | 'partial' | 'not_applied', errors: string[] = []) => ({ id: 'x', status, errors, store_opened: [] }) as never;
    expect(outcomeText(o('applied'), 'apply')).toBe('✓ Xong');
    expect(outcomeText(o('partial', ['x#0: Access is denied.']), 'apply')).toBe('⚠ Một phần');
    expect(outcomeText(o('not_applied', ['x#0: Access is denied.']), 'apply')).toBe('✗ Lỗi');
    expect(outcomeText(o('not_applied'), 'revert')).toBe('✓ Xong');
  });

  // Lệch có chủ ý (rà Task 11, L7): đã tới trạng thái đích mà còn lỗi phụ (vd dọn ảnh chụp `undo_save … read-only`)
  // ⇒ ⚠ một phần, không phải ✗ — máy đã đổi đúng.
  it('đã tới đích nhưng còn lỗi ⇒ ⚠, chưa tới đích ⇒ ✗', () => {
    const o = (status: 'applied' | 'not_applied', errors: string[]) => ({ id: 'x', status, errors, store_opened: [] }) as never;
    expect(outcomeText(o('applied', ['x: undo_save: read-only']), 'apply')).toBe('⚠ Một phần');
    expect(outcomeText(o('not_applied', ['x: undo_save: read-only']), 'revert')).toBe('⚠ Một phần');
    expect(outcomeText(o('applied', ['x#0: Access is denied.']), 'revert')).toBe('✗ Lỗi');
  });

  it('không có chuỗi rỗng, và khoá UI không trùng khoá danh mục', () => {
    for (const [k, v] of Object.entries({ ...ui, ...catalogVi })) expect(v.trim(), k).not.toBe('');
    for (const k of Object.keys(ui)) expect(k in catalogVi, k).toBe(false);
  });
});
