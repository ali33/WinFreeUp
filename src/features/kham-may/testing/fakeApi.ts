import { vi } from 'vitest';
import type { AppRow, ChildrenPage, Finding, KhamMayApi, NodeView, PerfTick, Sample } from '../api/types';

export const GB = 1024 ** 3;

export function node(id: number, name: string, bytes: number, extra: Partial<NodeView> = {}): NodeView {
  return {
    id,
    name,
    path: `C:\\${name}`,
    bytes,
    files: 1,
    modified: 1_790_000_000,
    is_dir: true,
    is_link: false,
    unreadable: false,
    has_children: true,
    protected: false,
    ...extra,
  };
}

/** Cây giả: C:\ (0) ─ Users (1) ─ an (2) ─ Downloads (3) ─ film.mkv (4); Windows (5, 🔒); hiberfil.sys (6, 🔒). */
export const ROOT = node(0, 'C:\\', 60 * GB, { path: 'C:\\', protected: true, files: 9 });
export const PAGES: Record<number, ChildrenPage> = {
  0: {
    parent: ROOT,
    items: [
      node(1, 'Users', 30 * GB, { protected: true, files: 5 }),
      node(5, 'Windows', 20 * GB, { protected: true, files: 3 }),
      node(6, 'hiberfil.sys', 8 * GB, { is_dir: false, has_children: false, protected: true }),
    ],
    rest: { count: 3, bytes: 2 * GB },
  },
  1: { parent: node(1, 'Users', 30 * GB, { protected: true }), items: [node(2, 'an', 30 * GB, { path: 'C:\\Users\\an', protected: true })], rest: null },
  2: {
    parent: node(2, 'an', 30 * GB, { path: 'C:\\Users\\an', protected: true }),
    items: [node(3, 'Downloads', 30 * GB, { path: 'C:\\Users\\an\\Downloads', files: 4 })],
    rest: null,
  },
  3: {
    parent: node(3, 'Downloads', 30 * GB, { path: 'C:\\Users\\an\\Downloads' }),
    items: [node(4, 'film.mkv', 30 * GB, { path: 'C:\\Users\\an\\Downloads\\film.mkv', is_dir: false, has_children: false })],
    rest: null,
  },
};

export const FINDINGS: Finding[] = [
  { id: 'disk_full', level: 'warn', value: 9.5, detail: null },
  { id: 'disk_health', level: 'ok', value: null, detail: null },
  { id: 'system_hdd', level: 'ok', value: null, detail: null },
  { id: 'ram_pressure', level: 'ok', value: 60, detail: null },
  { id: 'startup_apps', level: 'warn', value: 12, detail: null },
  { id: 'uptime', level: 'ok', value: 1.2, detail: null },
  { id: 'power_saver', level: 'ok', value: null, detail: null },
  { id: 'cpu_hot', level: 'unknown', value: null, detail: 'HRESULT Call failed with: 0x8004100C' },
];

export function sample(t_ms: number, cpu = 20): Sample {
  return {
    t_ms,
    dur_ms: 1000,
    cpu,
    ram_pct: 50,
    ram_used: 8 * GB,
    ram_total: 16 * GB,
    disk_active: 10,
    net_up_bps: 1024,
    net_down_bps: 4096,
    cpu_perf: 100,
  };
}

export function app(key: string, name: string, ram: number, extra: Partial<AppRow> = {}): AppRow {
  return {
    key,
    name,
    path: `C:\\Apps\\${key}`,
    ram,
    cpu: 1,
    disk_bps: 0,
    net_up_bps: 0,
    net_down_bps: 0,
    essential: false,
    procs: [{ pid: 100, create_time: 1, name: `${key}`, ram, cpu: 1, disk_bps: 0, net_up_bps: 0, net_down_bps: 0, essential: false }],
    ...extra,
  };
}

export function tick(t_ms: number, extra: Partial<PerfTick> = {}): PerfTick {
  return {
    sample: sample(t_ms),
    apps: [app('chrome.exe', 'Google Chrome', 2 * GB), app('svchost.exe', 'Service Host', GB, { essential: true, path: 'C:\\Windows\\System32\\svchost.exe' })],
    stutters: [],
    throttled: false,
    perf_error: null,
    disk_error: null,
    temp: { state: 'ok', celsius: 55 },
    net_app_error: null,
    interval_ms: 1000,
    ...extra,
  };
}

/** Bộ nối giả: mọi hàm là `vi.fn` trả dữ liệu mẫu ở trên; ghi đè từng hàm qua `over`. */
export function fakeApi(over: Partial<KhamMayApi> = {}): KhamMayApi {
  return {
    diskVolumes: vi.fn(async () => [
      { root: 'C:\\', label: 'Windows', fs: 'NTFS', total: 237 * GB, free: 20 * GB, kind: 'fixed' as const },
      { root: 'E:\\', label: 'USB', fs: 'FAT32', total: 32 * GB, free: 30 * GB, kind: 'removable' as const },
    ]),
    diskScan: vi.fn(async (_root: string, onProgress) => {
      onProgress({ files: 100, bytes: GB, current: 'C:\\Users', percent: 50 });
      return { root: ROOT, plan: { mode: 'mft' as const }, elapsed_ms: 4200 };
    }),
    diskScanCancel: vi.fn(async () => {}),
    treeChildren: vi.fn(async (id: number) => {
      const p = PAGES[id];
      if (!p) throw 'unknown_node';
      return p;
    }),
    diskDelete: vi.fn(async () => ({ removed: { bytes: 30 * GB, files: 1 }, parent: node(3, 'Downloads', 0), log_error: null })),
    diskReveal: vi.fn(async () => {}),
    healthCheck: vi.fn(async (onFinding) => {
      FINDINGS.forEach(onFinding);
      return FINDINGS;
    }),
    healthThrottle: vi.fn(async () => ({ id: 'cpu_throttle' as const, level: 'ok' as const, value: null, detail: null })),
    startupList: vi.fn(async () => ({
      entries: [
        { id: 'hkcu_run:OneDrive', source: 'hkcu_run' as const, name: 'OneDrive', command: 'C:\\OneDrive.exe /background', enabled: true },
        { id: 'user_folder:Zalo.lnk', source: 'user_folder' as const, name: 'Zalo.lnk', command: 'C:\\Startup\\Zalo.lnk', enabled: false },
      ],
      errors: [],
    })),
    startupSet: vi.fn(async (id: string, enabled: boolean) => ({
      id,
      source: 'hkcu_run' as const,
      name: id.split(':')[1],
      command: '',
      enabled,
    })),
    openSettings: vi.fn(async () => {}),
    perfStart: vi.fn(async () => [sample(1_000_000)]),
    perfStop: vi.fn(async () => {}),
    appKill: vi.fn(async () => ({ killed: 1, errors: [], log_error: null })),
    revealPath: vi.fn(async () => {}),
    appIcon: vi.fn(async () => null),
    copyText: vi.fn(async () => {}),
    ...over,
  };
}

export function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}
