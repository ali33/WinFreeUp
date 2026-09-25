import { describe, expect, it } from 'vitest';
import { formatClock, formatCount, formatDate, formatKBps, formatMBps, formatRate, formatSeconds, friendly } from './fmt';

describe('định dạng', () => {
  it('số và tốc độ theo kiểu Việt Nam', () => {
    expect(formatCount(1234567)).toBe('1.234.567');
    expect(formatMBps(1.5 * 1024 ** 2)).toBe('1,5');
    expect(formatMBps(-3)).toBe('0');
    expect(formatKBps(2048)).toBe('2');
    expect(formatRate(3 * 1024 ** 2)).toBe('3 MB/s');
    expect(formatRate(512 * 1024)).toBe('512 KB/s');
    expect(formatSeconds(4200)).toBe('4,2');
  });

  it('ngày và giờ theo giờ địa phương', () => {
    const local = new Date(2026, 8, 25, 7, 3, 9);
    expect(formatDate(local.getTime() / 1000)).toBe('25/09/2026');
    expect(formatClock(local.getTime())).toBe('07:03:09');
    expect(formatDate(0)).toBe('');
  });

  it('mã lỗi của lõi thành câu dễ hiểu, lỗi khác giữ nguyên văn', () => {
    expect(friendly('busy')).toContain('đang bận');
    expect(friendly('protected')).toContain('không xóa được');
    expect(friendly('essential')).toContain('thiết yếu');
    expect(friendly('unknown_node')).toContain('quét lại');
    expect(friendly(new Error('Access is denied. (os error 5)'))).toBe('Access is denied. (os error 5)');
  });
});
