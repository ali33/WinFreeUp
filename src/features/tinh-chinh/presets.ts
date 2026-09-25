import type { TweakLevel, TweakView } from './types';

export const LEVELS: TweakLevel[] = ['basic', 'recommended', 'aggressive'];
export type Preset = TweakLevel | 'custom';

const rank = (l: TweakLevel) => LEVELS.indexOf(l);

/** App không có trên máy và chưa từng bị gỡ ⇒ không hiện (spec 4.1). */
export function visible(t: TweakView): boolean {
  return t.status !== 'not_present';
}

/** Chỉ mục đọc được trạng thái bình thường mới được tích (không hỗ trợ / bị quản lý ⇒ không bao giờ). */
export function selectable(t: TweakView): boolean {
  return t.status === 'applied' || t.status === 'not_applied' || t.status === 'partial';
}

/** Bấm mức sẵn ⇒ tích mọi mục có `level` ≤ mức đó (spec mục 5), theo thứ tự danh mục. */
export function presetSelection(tweaks: TweakView[], level: TweakLevel): string[] {
  return tweaks.filter((t) => selectable(t) && rank(t.level) <= rank(level)).map((t) => t.id);
}

/**
 * Lựa chọn khi mở tab: mức Cơ bản nhưng chỉ giữ mục quyền riêng tư — không tích sẵn mục gỡ app
 * (quyết định của người dùng, lệch có chủ ý so với kế hoạch). Bấm nút «Cơ bản» vẫn tích đủ cả hai nhóm.
 */
export function defaultSelection(list: TweakView[]): string[] {
  const privacy = new Set(list.filter((t) => t.group === 'privacy').map((t) => t.id));
  return presetSelection(list, 'basic').filter((id) => privacy.has(id));
}

function sameSet(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  const s = new Set(a);
  return b.every((x) => s.has(x));
}

/** Mức đang khớp đúng tập đã tích; không khớp mức nào ⇒ `custom` («Tùy chỉnh»). Trùng nhiều mức ⇒ mức thấp nhất. */
export function currentPreset(tweaks: TweakView[], selected: string[]): Preset {
  if (selected.length === 0) return 'custom';
  return LEVELS.find((l) => sameSet(presetSelection(tweaks, l), selected)) ?? 'custom';
}

/** Số thay đổi thật: mục đã tích mà chưa ở trạng thái đích. */
export function pendingChanges(tweaks: TweakView[], selected: string[]): string[] {
  const s = new Set(selected);
  return tweaks.filter((t) => s.has(t.id) && selectable(t) && t.status !== 'applied').map((t) => t.id);
}

/** Mục đã tích có gì để hoàn tác: có ảnh chụp, hoặc đang ở trạng thái đã áp dụng/một phần. */
export function revertable(tweaks: TweakView[], selected: string[]): string[] {
  const s = new Set(selected);
  return tweaks
    .filter((t) => s.has(t.id) && selectable(t) && (t.has_undo || t.status === 'applied' || t.status === 'partial'))
    .map((t) => t.id);
}

/** Mục cần hộp xác nhận: `caution`, và mọi app sẽ gỡ khi bật «mọi tài khoản» (spec mục 5). */
export function needsConfirm(tweaks: TweakView[], ids: string[], allUsers: boolean): TweakView[] {
  const s = new Set(ids);
  return tweaks.filter((t) => s.has(t.id) && (t.risk === 'caution' || (allUsers && t.group === 'bloatware')));
}
