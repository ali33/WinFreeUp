import { t } from './i18n';

/** Trùng thứ tự `cleaners::registry::ALL_IDS` của lõi (spec mục 3). */
export const GROUP_IDS = [
  'user_temp',
  'system_temp',
  'browser_cache',
  'win_caches',
  'delivery_opt',
  'wu_download',
  'component_store',
  'recycle_bin',
  'windows_old',
] as const;

export type GroupId = (typeof GROUP_IDS)[number];

export function groupName(id: string): string {
  return t(`group.${id}.name`);
}

export function groupDesc(id: string): string {
  return t(`group.${id}.desc`);
}

/**
 * Dịch mã thông báo của lõi (vd "browser_running:chrome", "root_rejected:C:\Users\a\AppData").
 * Không dùng `split(':')` để tách phần sau tiền tố — đường dẫn Windows có `C:` nên sẽ bị cắt sai
 * chỗ; dùng `slice` theo độ dài tiền tố để giữ nguyên cả đường dẫn. Mã lạ giữ nguyên để vẫn thấy được.
 */
export function noticeText(code: string): string {
  if (code.startsWith('browser_running:')) {
    const arg = code.slice('browser_running:'.length);
    if (arg) return t('notice.browserRunning', { name: t(`browser.${arg}`) });
  } else if (code.startsWith('root_rejected:')) {
    return t('notice.rootRejected', { path: code.slice('root_rejected:'.length) });
  } else if (code.startsWith('root_unreadable:')) {
    return t('notice.rootUnreadable', { path: code.slice('root_unreadable:'.length) });
  } else if (code.startsWith('unreadable_entries:')) {
    return t('notice.unreadableEntries', { count: code.slice('unreadable_entries:'.length) });
  } else if (code === 'not_scanned') {
    return t('notice.notScanned');
  }
  return code;
}
