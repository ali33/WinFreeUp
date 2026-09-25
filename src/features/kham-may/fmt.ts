import { formatBytes } from '../../format';
import { tk } from './i18n';

export { formatBytes };

const one = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 1 });
const int = new Intl.NumberFormat('vi-VN', { maximumFractionDigits: 0 });

/** Số nguyên có dấu chấm ngăn cách hàng nghìn kiểu Việt Nam: 1234567 ⇒ "1.234.567". */
export function formatCount(n: number): string {
  return int.format(n);
}

/** Byte/giây ⇒ MB/s một chữ số thập phân ("0" khi không có hoạt động). */
export function formatMBps(bps: number): string {
  return one.format(Math.max(0, bps) / 1024 ** 2);
}

/** Byte/giây ⇒ KB/s số nguyên. */
export function formatKBps(bps: number): string {
  return int.format(Math.max(0, bps) / 1024);
}

/** Tốc độ mạng tự chọn đơn vị cho nhãn biểu đồ. */
export function formatRate(bps: number): string {
  if (bps >= 1024 ** 2) return `${one.format(bps / 1024 ** 2)} MB/s`;
  return `${int.format(Math.max(0, bps) / 1024)} KB/s`;
}

/** ms ⇒ giây một chữ số thập phân kiểu Việt Nam: 4200 ⇒ "4,2". */
export function formatSeconds(ms: number): string {
  return one.format(ms / 1000);
}

export function formatPct(v: number): string {
  return `${int.format(v)}%`;
}

/** Giây Unix ⇒ "dd/MM/yyyy" theo giờ địa phương; 0 ⇒ chuỗi rỗng. */
export function formatDate(unixSeconds: number): string {
  if (!unixSeconds) return '';
  const d = new Date(unixSeconds * 1000);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getDate())}/${p(d.getMonth() + 1)}/${d.getFullYear()}`;
}

/** ms Unix ⇒ "HH:mm:ss" theo giờ địa phương. */
export function formatClock(ms: number): string {
  const d = new Date(ms);
  const p = (n: number) => String(n).padStart(2, '0');
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}

/**
 * Lỗi lệnh Tauri là chuỗi: mã ngắn của lõi đổi sang câu dễ hiểu, còn lại giữ nguyên văn để người dùng
 * chụp màn hình gửi lại được.
 */
export function friendly(e: unknown): string {
  const m = e instanceof Error ? e.message : typeof e === 'string' ? e : JSON.stringify(e);
  switch (m) {
    case 'busy':
      return tk('km.err.busy');
    case 'protected':
      return tk('km.err.protected');
    case 'essential':
      return tk('km.err.essential');
    case 'no_tree':
    case 'unknown_node':
      return tk('km.err.staleTree');
    case 'unknown_app':
      return tk('km.err.unknownApp');
    case 'aborted':
      return tk('km.err.aborted');
    default:
      return m;
  }
}
