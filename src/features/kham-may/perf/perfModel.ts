import type { AppRow, Sample } from '../api/types';

/** Spec 5: giữ 5 phút gần nhất. */
export const WINDOW_MS = 5 * 60 * 1000;

/** Thêm mẫu mới, bỏ mẫu trùng giờ và mẫu cũ hơn 5 phút so với mẫu mới nhất. */
export function pushSample(list: Sample[], s: Sample): Sample[] {
  const next = list.filter((x) => x.t_ms !== s.t_ms && x.t_ms >= s.t_ms - WINDOW_MS);
  next.push(s);
  next.sort((a, b) => a.t_ms - b.t_ms);
  return next;
}

export type AppSortKey = 'name' | 'ram' | 'cpu' | 'disk' | 'net';

/** Tổng mạng lên + xuống; `null` khi không theo dõi được mạng theo app. */
export function netTotal(a: { net_up_bps: number | null; net_down_bps: number | null }): number | null {
  if (a.net_up_bps === null || a.net_down_bps === null) return null;
  return a.net_up_bps + a.net_down_bps;
}

function metric(a: AppRow, key: AppSortKey): number {
  switch (key) {
    case 'ram':
      return a.ram;
    case 'cpu':
      return a.cpu;
    case 'disk':
      return a.disk_bps;
    case 'net':
      return netTotal(a) ?? -1;
    default:
      return 0;
  }
}

/** Sắp theo mọi cột (spec 5.2). Mặc định RAM giảm dần. */
export function sortApps(apps: AppRow[], key: AppSortKey, desc: boolean): AppRow[] {
  const dir = desc ? -1 : 1;
  return [...apps].sort((a, b) => {
    const c = key === 'name' ? a.name.localeCompare(b.name, 'vi') : metric(a, key) - metric(b, key);
    return c !== 0 ? c * dir : a.key.localeCompare(b.key);
  });
}

export interface Pt {
  t: number;
  v: number;
}

/**
 * Chuỗi điểm cho một đường, tách thành nhiều đoạn ở chỗ thiếu số liệu hoặc có khoảng dừng lấy mẫu
 * (rời mục rồi quay lại) — không nối thẳng qua chỗ trống, kẻo người xem tưởng có số đo.
 */
export function segments(samples: Sample[], pick: (s: Sample) => number | null): Pt[][] {
  const out: Pt[][] = [];
  let cur: Pt[] = [];
  let prev: Sample | null = null;
  for (const s of samples) {
    const v = pick(s);
    const gap = prev !== null && s.t_ms - prev.t_ms > 3 * Math.max(s.dur_ms, 1000);
    if (v === null || gap) {
      if (cur.length) out.push(cur);
      cur = [];
    }
    if (v !== null) cur.push({ t: s.t_ms, v });
    prev = s;
  }
  if (cur.length) out.push(cur);
  return out;
}

/** Trục dọc tự co cho biểu đồ mạng: bội số đẹp ≥ giá trị lớn nhất, tối thiểu 64 KB/s. */
export function niceMax(values: number[]): number {
  const m = Math.max(64 * 1024, ...values);
  const p = 2 ** Math.ceil(Math.log2(m));
  return p;
}
