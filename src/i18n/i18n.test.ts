import { describe, expect, it, vi } from 'vitest';
import { t, hasKey } from './index';
import vi_ from './vi.json';
import { GROUP_IDS, groupDesc, groupName, noticeText } from '../catalog';

describe('t()', () => {
  it('thay tham số {tên}', () => {
    expect(t('preview.clean', { size: '1,5 GB' })).toBe('Dọn 1,5 GB');
  });

  it('thiếu khoá thì trả về chính khoá và cảnh báo console', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    expect(t('khong.co.khoa.nay')).toBe('khong.co.khoa.nay');
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });

  it('mọi chuỗi đều khác rỗng', () => {
    for (const [k, v] of Object.entries(vi_)) {
      expect(typeof v, k).toBe('string');
      expect((v as string).trim().length, k).toBeGreaterThan(0);
    }
  });
});

describe('catalog', () => {
  it('có đúng 9 nhóm theo thứ tự lõi', () => {
    expect(GROUP_IDS).toEqual([
      'user_temp', 'system_temp', 'browser_cache', 'win_caches', 'delivery_opt',
      'wu_download', 'component_store', 'recycle_bin', 'windows_old',
    ]);
  });

  it('mỗi nhóm có tên và dòng giải thích tiếng Việt', () => {
    for (const id of GROUP_IDS) {
      expect(hasKey(`group.${id}.name`), id).toBe(true);
      expect(hasKey(`group.${id}.desc`), id).toBe(true);
      expect(groupName(id)).not.toContain('group.');
      expect(groupDesc(id)).not.toContain('group.');
    }
  });

  it('dịch mã thông báo trình duyệt đang mở', () => {
    expect(noticeText('browser_running:coccoc')).toContain('Cốc Cốc');
    expect(noticeText('ma_la')).toBe('ma_la');
  });

  it('dịch mã thông báo gốc bị loại và gốc không đọc được, giữ nguyên cả đường dẫn có dấu ":"', () => {
    const path = 'C:\\Users\\a\\AppData\\Local\\Temp';
    const rejected = noticeText(`root_rejected:${path}`);
    expect(rejected).toContain(path);
    expect(rejected).not.toContain('root_rejected:');
    const unreadable = noticeText(`root_unreadable:${path}`);
    expect(unreadable).toContain(path);
    expect(unreadable).not.toContain('root_unreadable:');
  });
});
