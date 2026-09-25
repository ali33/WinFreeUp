// Khớp serde của crate winfreeup-tweaks (tên trường snake_case) — đừng đổi tên.
import type { RestorePointStatus } from '../../api/types';

export type TweakGroup = 'privacy' | 'bloatware';
export type TweakLevel = 'basic' | 'recommended' | 'aggressive';
export type TweakRisk = 'safe' | 'caution';
export type Restart = 'none' | 'explorer' | 'logoff' | 'reboot';

export type TweakStatus =
  | { status: 'applied' }
  | { status: 'not_applied' }
  | { status: 'partial' }
  | { status: 'unsupported'; reason: string }
  | { status: 'managed' }
  | { status: 'not_present' };

export type TweakView = {
  id: string;
  group: TweakGroup;
  level: TweakLevel;
  risk: TweakRisk;
  needs_restart: Restart;
  has_undo: boolean;
  errors: string[];
} & TweakStatus;

export interface SystemInfo {
  build: number;
  edition: string;
  managed: boolean;
  other_user: boolean;
}

export interface ReadResult {
  system: SystemInfo;
  tweaks: TweakView[];
  /** Mã thông báo, vd `undo_corrupt:<chi tiết>`. */
  notices: string[];
}

export type TweakOutcome = { id: string; errors: string[]; store_opened: string[] } & TweakStatus;

export interface RunReport {
  outcomes: TweakOutcome[];
  restart: Restart;
  notices: string[];
}

export type TweakEvent =
  | { kind: 'started'; id: string; index: number; total: number }
  | { kind: 'finished'; outcome: TweakOutcome };

/** Cầu nối tới vỏ Tauri cho tab Tinh chỉnh. Bản thật ở `api.ts`; test dùng bản giả. */
export interface TweakApi {
  read(): Promise<ReadResult>;
  prepareRestorePoint(): Promise<RestorePointStatus>;
  apply(ids: string[], allUsers: boolean, onEvent: (e: TweakEvent) => void): Promise<RunReport>;
  revert(ids: string[], onEvent: (e: TweakEvent) => void): Promise<RunReport>;
  restartExplorer(): Promise<void>;
}
