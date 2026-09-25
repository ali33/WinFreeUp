import { describe, expect, it, vi } from 'vitest';
import { describeErrorEvent, dismissNotice, installGlobalHooks, MAX_NOTICES, messageOf, pushNotice, type Notice } from './errors';

describe('pushNotice', () => {
  it('gộp trùng và đếm số lần', () => {
    let l: Notice[] = [];
    l = pushNotice(l, 'warning', 'A');
    l = pushNotice(l, 'warning', 'A');
    expect(l).toHaveLength(1);
    expect(l[0].count).toBe(2);
  });

  it('tối đa 5 dòng, bỏ cảnh báo cũ trước, giữ lỗi đỏ', () => {
    let l: Notice[] = pushNotice([], 'error', 'ĐỎ');
    for (let i = 0; i < 7; i++) l = pushNotice(l, 'warning', `w${i}`);
    expect(l).toHaveLength(MAX_NOTICES);
    expect(l[0].message).toBe('ĐỎ');
    expect(l.map((n) => n.message)).toEqual(['ĐỎ', 'w3', 'w4', 'w5', 'w6']);
  });

  it('đóng một dòng', () => {
    const l = pushNotice(pushNotice([], 'warning', 'A'), 'error', 'B');
    expect(dismissNotice(l, l[0].id).map((n) => n.message)).toEqual(['B']);
  });
});

describe('thông điệp lỗi', () => {
  it('lấy nguyên văn từ Error, chuỗi hoặc đối tượng', () => {
    expect(messageOf(new Error('hỏng ổ'))).toBe('hỏng ổ');
    expect(messageOf('busy')).toBe('busy');
    expect(messageOf({ code: 5 })).toBe('{"code":5}');
  });

  it('kèm tên file và số dòng', () => {
    expect(describeErrorEvent({ message: 'x is undefined', filename: 'http://localhost/assets/index-ab.js', lineno: 12 })).toBe('x is undefined (index-ab.js:12)');
    expect(describeErrorEvent({ error: new Error('không file') })).toBe('không file');
  });
});

describe('installGlobalHooks', () => {
  it('lỗi ngoài luồng và promise bị bỏ rơi đều lên màn', () => {
    const report = vi.fn();
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    const off = installGlobalHooks(window, report);
    window.dispatchEvent(new ErrorEvent('error', { message: 'vẽ hỏng', filename: 'a.js', lineno: 3 }));
    const rej = new Event('unhandledrejection') as Event & { reason?: unknown };
    rej.reason = new Error('promise hỏng');
    window.dispatchEvent(rej);
    expect(report).toHaveBeenCalledWith('warning', 'vẽ hỏng (a.js:3)');
    expect(report).toHaveBeenCalledWith('warning', 'promise hỏng');
    expect(err).toHaveBeenCalled();
    off();
    window.dispatchEvent(new ErrorEvent('error', { message: 'sau khi gỡ' }));
    expect(report).toHaveBeenCalledTimes(2);
    err.mockRestore();
  });
});
