import { describe, expect, it } from 'vitest';
import { currentPreset, defaultSelection, needsConfirm, pendingChanges, presetSelection, revertable, revertOnly, selectable, visible } from './presets';
import { sample, tw } from './testdata';
import type { TweakView } from './types';

const all = sample();

describe('mức sẵn', () => {
  it('Cơ bản ⊂ Khuyến nghị ⊂ Triệt để, bỏ mục không hỗ trợ và app không có', () => {
    expect(presetSelection(all, 'basic')).toEqual(['ads_id', 'app_clipchamp']);
    expect(presetSelection(all, 'recommended')).toEqual(['ads_id', 'app_clipchamp', 'svc_diagtrack', 'app_weather']);
    expect(presetSelection(all, 'aggressive')).toEqual([
      'ads_id',
      'app_clipchamp',
      'svc_diagtrack',
      'app_weather',
      'cloud_clipboard',
      'app_game_bar',
    ]);
  });

  it('mục bị tổ chức quản lý không bao giờ được tích', () => {
    const list = [tw('a'), tw('b', { status: 'managed' })];
    expect(presetSelection(list, 'aggressive')).toEqual(['a']);
    expect(selectable(list[1])).toBe(false);
  });

  it('nhận diện mức đang khớp, không phụ thuộc thứ tự tích', () => {
    expect(currentPreset(all, ['app_clipchamp', 'ads_id'])).toBe('basic');
    expect(currentPreset(all, presetSelection(all, 'aggressive'))).toBe('aggressive');
    expect(currentPreset(all, ['ads_id'])).toBe('custom');
    expect(currentPreset(all, [])).toBe('custom');
  });

  it('danh mục chỉ có mục Cơ bản ⇒ ba mức trùng nhau ⇒ báo mức thấp nhất', () => {
    const list = [tw('a'), tw('b')];
    expect(currentPreset(list, ['a', 'b'])).toBe('basic');
  });
});

// Lệch có chủ ý (quyết định của người dùng): mở tab chỉ tích sẵn mục quyền riêng tư của mức Cơ bản,
// không tích sẵn mục gỡ app; bấm nút «Cơ bản» vẫn tích đủ cả hai nhóm.
describe('lựa chọn mặc định khi mở tab', () => {
  it('chỉ gồm mục quyền riêng tư của mức Cơ bản, không có mục gỡ app', () => {
    expect(defaultSelection(all)).toEqual(['ads_id']);
    const byId = new Map(all.map((t) => [t.id, t]));
    for (const id of defaultSelection(all)) expect(byId.get(id)?.group, id).toBe('privacy');
  });

  it('bấm «Cơ bản» vẫn tích cả mục gỡ app', () => {
    expect(presetSelection(all, 'basic')).toContain('app_clipchamp');
  });

  it('lựa chọn mặc định không khớp mức nào khi mức Cơ bản có app ⇒ «Tùy chỉnh»', () => {
    expect(currentPreset(all, defaultSelection(all))).toBe('custom');
  });

  it('mức Cơ bản không có app nào trên máy ⇒ mặc định trùng Cơ bản', () => {
    const list = [tw('a'), tw('x', { group: 'bloatware', status: 'not_present' })];
    expect(defaultSelection(list)).toEqual(['a']);
    expect(currentPreset(list, defaultSelection(list))).toBe('basic');
  });
});

describe('đếm thay đổi', () => {
  it('bỏ qua mục đã ở trạng thái đích', () => {
    expect(pendingChanges(all, ['ads_id', 'app_clipchamp', 'cloud_clipboard'])).toEqual(['ads_id', 'cloud_clipboard']);
  });

  it('hoàn tác chỉ tính mục có gì để trả', () => {
    expect(revertable(all, ['ads_id', 'app_clipchamp', 'cloud_clipboard', 'recall_off'])).toEqual(['app_clipchamp', 'cloud_clipboard']);
  });

  it('app không có trên máy bị ẩn', () => {
    expect(all.filter(visible).map((t) => t.id)).not.toContain('app_tiktok');
  });
});

describe('hộp xác nhận', () => {
  it('liệt kê mục caution; bật «mọi tài khoản» thì thêm mọi app sẽ gỡ', () => {
    expect(needsConfirm(all, ['ads_id', 'svc_diagtrack', 'app_weather'], false).map((t) => t.id)).toEqual(['svc_diagtrack']);
    expect(needsConfirm(all, ['ads_id', 'svc_diagtrack', 'app_weather'], true).map((t) => t.id)).toEqual(['svc_diagtrack', 'app_weather']);
    expect(needsConfirm(all, ['ads_id'], false)).toEqual([]);
  });
});

// Lệch có chủ ý (yêu cầu sau rà Task 11): mục sai build/edition mà WinFreeUp đã áp dụng trước đó ⇒ chỉ được hoàn tác.
describe('mục chỉ hoàn tác', () => {
  const ro = tw('old_tweak', { status: 'unsupported', reason: 'build_max:19045', has_undo: true } as Partial<TweakView>);
  const list = [...sample(), ro];

  it('nhận ra mục không hỗ trợ mà có ảnh chụp; mục không hỗ trợ thường thì không', () => {
    expect(revertOnly(ro)).toBe(true);
    expect(revertOnly(list.find((t) => t.id === 'recall_off')!)).toBe(false);
    expect(revertOnly(tw('x', { has_undo: true }))).toBe(false);
  });

  it('không vào mức sẵn, lựa chọn mặc định, số thay đổi; có vào hoàn tác', () => {
    for (const l of ['basic', 'recommended', 'aggressive'] as const) expect(presetSelection(list, l)).not.toContain('old_tweak');
    expect(defaultSelection(list)).not.toContain('old_tweak');
    expect(pendingChanges(list, ['old_tweak'])).toEqual([]);
    expect(revertable(list, ['old_tweak'])).toEqual(['old_tweak']);
  });

  it('tích thêm mục chỉ hoàn tác không làm đổi mức sẵn đang khớp', () => {
    expect(currentPreset(list, [...presetSelection(list, 'basic'), 'old_tweak'])).toBe('basic');
    expect(currentPreset(list, ['old_tweak'])).toBe('custom');
  });
});
