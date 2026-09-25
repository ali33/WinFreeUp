// Dữ liệu mẫu cho test của tab Tinh chỉnh.
import type { ReadResult, RunReport, TweakView } from './types';

export function tw(id: string, over: Partial<TweakView> = {}): TweakView {
  return {
    id,
    group: 'privacy',
    level: 'basic',
    risk: 'safe',
    needs_restart: 'none',
    has_undo: false,
    errors: [],
    status: 'not_applied',
    ...over,
  } as TweakView;
}

/** Hai mục mỗi mức + một mục không hỗ trợ + một app không có trên máy. */
export function sample(): TweakView[] {
  return [
    tw('ads_id'),
    tw('app_clipchamp', { group: 'bloatware', status: 'applied', has_undo: true }),
    tw('svc_diagtrack', { level: 'recommended', risk: 'caution' }),
    tw('app_weather', { group: 'bloatware', level: 'recommended' }),
    tw('recall_off', { level: 'recommended', status: 'unsupported', reason: 'build_min:26100' } as Partial<TweakView>),
    tw('cloud_clipboard', { level: 'aggressive', status: 'partial' }),
    tw('app_game_bar', { group: 'bloatware', level: 'aggressive', risk: 'caution' }),
    tw('app_tiktok', { group: 'bloatware', status: 'not_present' }),
  ];
}

export function readResult(tweaks: TweakView[] = sample(), over: Partial<ReadResult> = {}): ReadResult {
  return { system: { build: 26200, edition: 'Pro', managed: false, other_user: false }, tweaks, notices: [], ...over };
}

export function report(over: Partial<RunReport> = {}): RunReport {
  return { outcomes: [], restart: 'none', notices: [], ...over };
}
