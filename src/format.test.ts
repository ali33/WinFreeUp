import { describe, expect, it } from 'vitest';
import { formatBytes } from './format';

describe('formatBytes', () => {
  it('GB có một chữ số thập phân, dấu phẩy', () => {
    expect(formatBytes(1.5 * 1024 ** 3)).toBe('1,5 GB');
    expect(formatBytes(1024 ** 3)).toBe('1 GB');
  });
  it('MB là số nguyên', () => {
    expect(formatBytes(5 * 1024 ** 2)).toBe('5 MB');
  });
  it('rất nhỏ, bằng 0 hoặc âm', () => {
    expect(formatBytes(1000)).toBe('< 1 MB');
    expect(formatBytes(0)).toBe('0 MB');
    expect(formatBytes(-5)).toBe('0 MB');
  });
});
