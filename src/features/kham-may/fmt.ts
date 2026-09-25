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
  const m = e instanceof Error ? e.message : typeof e === 'string' ? e : stringify(e);
  // KHÔNG thêm/đổi nhánh cho mã 'cancelled': Task 12 so sánh nguyên chuỗi này để biết người dùng tự hủy.
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
    case 'unknown_volume':
      return tk('km.err.unknownVolume');
    case 'bad_path':
      return tk('km.err.badPath');
    case 'redirected_path':
      return tk('km.err.redirectedPath');
    case 'no_recycle_bin':
      return tk('km.err.noRecycleBin');
    case 'recycle_disabled':
      return tk('km.err.recycleDisabled');
    case 'onedrive':
      return tk('km.err.onedrive');
    default:
      return protectReason(m) ?? m;
  }
}

const PROTECT_REASONS = [
  'system_dir',
  'user_profile_root',
  'drive_root',
  'root_special',
  'link_ancestor',
  'bad_path',
  'canonical_protected',
] as const;

/** `protected:<lý do>` (lõi, `ProtectRules::protect_reason`) ⇒ câu nói vì sao mục bị khoá; lý do lạ ⇒ câu chung. */
function protectReason(m: string): string | undefined {
  if (!m.startsWith('protected:')) return undefined;
  const reason = m.slice('protected:'.length);
  return (PROTECT_REASONS as readonly string[]).includes(reason) ? tk(`km.protect.${reason}`) : tk('km.err.protected');
}

/** JSON khi được; `undefined`, vòng tham chiếu, BigInt… thì `String(e)` — không bao giờ ném. */
function stringify(e: unknown): string {
  try {
    const s = JSON.stringify(e);
    if (typeof s === 'string') return s;
  } catch {
    // rơi xuống String(e)
  }
  return String(e);
}
