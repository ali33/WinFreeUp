// Chuỗi của tab Tinh chỉnh. Trước Task Tích hợp: đọc hai file JSON riêng của tính năng
// (không đụng src/i18n/vi.json đang do v0.1 sở hữu). Task Tích hợp thay cả file bằng một dòng re-export `t`.
import ui from './vi.json';
import items from './catalog.vi.json';

const dict: Record<string, string> = { ...ui, ...items };

export type Params = Record<string, string | number>;

export function hasKey(key: string): boolean {
  return Object.prototype.hasOwnProperty.call(dict, key);
}

export function tt(key: string, params?: Params): string {
  if (!hasKey(key)) {
    console.warn(`[i18n] thiếu khoá: ${key}`);
    return key;
  }
  return dict[key].replace(/\{(\w+)\}/g, (whole, name: string) => (params && name in params ? String(params[name]) : whole));
}
