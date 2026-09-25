import { describe, expect, it, vi } from 'vitest';
import { hasKey, tk } from './i18n';
import km from './vi.json';

// File này và ./vi.json bị xoá ở Task Tích hợp: khi đó chuỗi nằm trong src/i18n/vi.json và
// test «mọi chuỗi đều khác rỗng» của src/i18n/i18n.test.ts phủ luôn các khoá km.*.
describe('chuỗi tab Khám máy', () => {
  it('mọi khoá có tiền tố km. và không rỗng', () => {
    for (const [k, v] of Object.entries(km)) {
      expect(k.startsWith('km.'), k).toBe(true);
      expect((v as string).trim().length, k).toBeGreaterThan(0);
    }
  });

  it('thay tham số; thiếu khoá thì trả chính khoá và cảnh báo console', () => {
    expect(tk('km.disk.rest', { count: 3, size: '1 GB' })).toBe('(+3 mục nhỏ khác, 1 GB)');
    expect(hasKey('km.tab.overview')).toBe(true);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    expect(tk('km.khong.co')).toBe('km.khong.co');
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});
