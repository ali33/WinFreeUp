// Khớp serde của crate winfreeup-diagnose (tên trường snake_case) — đừng đổi tên.

export type DriveKind = 'fixed' | 'removable' | 'network' | 'cdrom' | 'ram' | 'unknown';

export interface VolumeInfo {
  /** Dạng "C:\\". */
  root: string;
  label: string;
  fs: string;
  total: number;
  free: number;
  kind: DriveKind;
}

export interface ScanStatus {
  files: number;
  bytes: number;
  current: string;
  /** Có khi quét bằng MFT; duyệt thư mục thì null. */
  percent: number | null;
}

export type WalkReason =
  | { code: 'not_ntfs'; fs: string }
  | { code: 'not_local'; kind: DriveKind }
  | { code: 'mft_failed'; message: string };

export type ScanPlan = { mode: 'mft' } | { mode: 'walk'; reason: WalkReason };

export interface NodeView {
  id: number;
  name: string;
  path: string;
  bytes: number;
  files: number;
  /** Giây Unix. */
  modified: number;
  is_dir: boolean;
  is_link: boolean;
  unreadable: boolean;
  has_children: boolean;
  protected: boolean;
}

export interface RestSummary {
  count: number;
  bytes: number;
}

export interface ChildrenPage {
  parent: NodeView;
  items: NodeView[];
  rest: RestSummary | null;
}

export interface ScanSummary {
  root: NodeView;
  plan: ScanPlan;
  elapsed_ms: number;
}

export interface Removed {
  bytes: number;
  files: number;
}

export interface DeleteResult {
  removed: Removed;
  parent: NodeView | null;
  log_error: string | null;
}

export type Level = 'critical' | 'warn' | 'ok' | 'unknown';

export type FindingId =
  | 'disk_full'
  | 'disk_health'
  | 'system_hdd'
  | 'ram_pressure'
  | 'startup_apps'
  | 'uptime'
  | 'power_saver'
  | 'cpu_throttle'
  | 'cpu_hot';

export interface Finding {
  id: FindingId;
  level: Level;
  value: number | null;
  detail: string | null;
}

export type StartupSource = 'hkcu_run' | 'hklm_run' | 'hklm_run32' | 'user_folder' | 'common_folder';

export interface StartupEntry {
  id: string;
  source: StartupSource;
  name: string;
  command: string;
  enabled: boolean;
}

export interface StartupList {
  entries: StartupEntry[];
  errors: string[];
}

export interface Sample {
  /** Giờ Unix (ms). */
  t_ms: number;
  dur_ms: number;
  cpu: number;
  ram_pct: number;
  ram_used: number;
  ram_total: number;
  disk_active: number | null;
  net_up_bps: number | null;
  net_down_bps: number | null;
  cpu_perf: number | null;
}

export interface ProcRow {
  pid: number;
  create_time: number;
  name: string;
  ram: number;
  cpu: number;
  disk_bps: number;
  net_up_bps: number | null;
  net_down_bps: number | null;
  essential: boolean;
}

export interface AppRow {
  key: string;
  name: string;
  path: string | null;
  ram: number;
  cpu: number;
  disk_bps: number;
  net_up_bps: number | null;
  net_down_bps: number | null;
  essential: boolean;
  procs: ProcRow[];
}

export interface TopApp {
  key: string;
  name: string;
  avg: number;
}

export interface StutterView {
  start_ms: number;
  end_ms: number;
  cpu: boolean;
  disk: boolean;
  top_cpu: TopApp[];
  top_disk: TopApp[];
  throttled: boolean;
}

export type TempState =
  | { state: 'ok'; celsius: number }
  | { state: 'unavailable'; code: 'no_sensor' | 'out_of_range' | 'stuck'; detail: string };

export interface PerfTick {
  sample: Sample;
  apps: AppRow[];
  stutters: StutterView[];
  throttled: boolean | null;
  perf_error: string | null;
  disk_error: string | null;
  temp: TempState;
  net_app_error: string | null;
  interval_ms: number;
}

export interface KillResult {
  killed: number;
  errors: string[];
  log_error: string | null;
}

/** Cầu nối tới vỏ Tauri cho tab Khám máy. Bản thật ở `api/tauri.ts`; test dùng `testing/fakeApi.ts`. */
export interface KhamMayApi {
  diskVolumes(): Promise<VolumeInfo[]>;
  diskScan(root: string, onProgress: (s: ScanStatus) => void): Promise<ScanSummary>;
  diskScanCancel(): Promise<void>;
  treeChildren(id: number): Promise<ChildrenPage>;
  diskDelete(id: number): Promise<DeleteResult>;
  diskReveal(id: number): Promise<void>;
  healthCheck(onFinding: (f: Finding) => void): Promise<Finding[]>;
  healthThrottle(): Promise<Finding>;
  startupList(): Promise<StartupList>;
  startupSet(id: string, enabled: boolean): Promise<StartupEntry>;
  openSettings(page: 'power' | 'battery' | 'startup' | 'storage'): Promise<void>;
  /** Bắt đầu lấy mẫu; trả lịch sử còn giữ. `onTick` nhận từng mẫu cho tới `perfStop`. */
  perfStart(onTick: (t: PerfTick) => void, onError: (message: string) => void): Promise<Sample[]>;
  perfStop(): Promise<void>;
  appKill(key: string): Promise<KillResult>;
  revealPath(path: string): Promise<void>;
  appIcon(path: string): Promise<string | null>;
  copyText(text: string): Promise<void>;
}

/** Cùng hình dạng `Notify` của v0.1 (`src/state/controller.ts`): App truyền xuống, lỗi lên băng đỏ/hổ phách. */
export type Notify = (severity: 'error' | 'warning', message: string) => void;
