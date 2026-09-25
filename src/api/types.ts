// Khớp serde của winfreeup-core (tên trường snake_case) — đừng đổi tên.
export type RiskLevel = 'safe' | 'caution' | 'risky';

export interface Item {
  path: string;
  bytes: number;
}

export interface ScanResult {
  total_bytes: number;
  file_count: number;
  top_items: Item[];
  estimated: boolean;
  notices: string[];
}

export interface GroupScan {
  id: string;
  risk: RiskLevel;
  default_selected: boolean;
  result: ScanResult | null;
  error: string | null;
}

export interface CleanReport {
  bytes_freed: number;
  files_deleted: number;
  skipped_locked: number;
  errors: string[];
  dry_run: boolean;
}

export interface GroupClean {
  id: string;
  report: CleanReport | null;
  error: string | null;
}

export type CleanEvent =
  | { kind: 'started'; id: string }
  | { kind: 'percent'; id: string; percent: number }
  | { kind: 'finished'; result: GroupClean };

export type RestorePointStatus =
  | { status: 'not_needed' }
  | { status: 'created' }
  | { status: 'skipped' }
  | { status: 'failed'; message: string };

export interface CleanSummary {
  groups: GroupClean[];
  log_path: string;
  dry_run: boolean;
  log_write_failed: boolean;
}

export interface AppInfo {
  version: string;
  dry_run: boolean;
  /** Dạng "C:\\". */
  system_drive: string;
}

/** Cầu nối tới vỏ Tauri. Bản thật ở `api/tauri.ts` (Task 13); test dùng bản giả. */
export interface Api {
  appInfo(): Promise<AppInfo>;
  diskFree(): Promise<number>;
  scanAll(onGroup: (g: GroupScan) => void): Promise<GroupScan[]>;
  cancelScan(): Promise<void>;
  prepareRestorePoint(ids: string[], dryRun: boolean): Promise<RestorePointStatus>;
  clean(ids: string[], dryRun: boolean, onEvent: (e: CleanEvent) => void): Promise<CleanSummary>;
  openLogFolder(): Promise<void>;
}
