import km from './vi.json';

// Chuỗi của tab Khám máy tạm nằm ở `./vi.json` để thi công song song với v0.1 mà không chạm `src/i18n/vi.json`.
// Task Tích hợp trộn các khoá này vào `src/i18n/vi.json` rồi thay cả file này bằng: export { t as tk, hasKey } from '../../i18n';
const dict: Record<string, string> = km;

export type Params = Record<string, string | number>;

export function hasKey(key: string): boolean {
  return Object.prototype.hasOwnProperty.call(dict, key);
}

export function tk(key: string, params?: Params): string {
  if (!hasKey(key)) {
    console.warn(`[i18n] thiếu khoá: ${key}`);
    return key;
  }
  return dict[key].replace(/\{(\w+)\}/g, (whole, name: string) => (params && name in params ? String(params[name]) : whole));
}
