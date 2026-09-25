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

/** Dịch mã thông báo của lõi (vd "browser_running:chrome"). Mã lạ giữ nguyên để vẫn thấy được. */
export function noticeText(code: string): string {
  const [kind, arg] = code.split(':');
  if (kind === 'browser_running' && arg) {
    return t('notice.browserRunning', { name: t(`browser.${arg}`) });
  }
  return code;
}
