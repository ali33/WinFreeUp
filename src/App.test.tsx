import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import type { Api, GroupScan } from './api/types';
import { App } from './App';

const GB = 1024 ** 3;

function fakeApi(over: Partial<Api> = {}): Api {
  const free = [50 * GB, 50 * GB, 52 * GB, 52 * GB];
  const groups: GroupScan[] = [
    { id: 'user_temp', risk: 'safe', default_selected: true, result: { total_bytes: GB, file_count: 3, top_items: [], estimated: false, notices: [] }, error: null },
  ];
  return {
    appInfo: vi.fn(async () => ({ version: '0.1.0', dry_run: false, system_drive: 'C:\\' })),
    diskFree: vi.fn(async () => free.shift() ?? 0),
    scanAll: vi.fn(async (onGroup) => {
      groups.forEach(onGroup);
      return groups;
    }),
    cancelScan: vi.fn(async () => {}),
    prepareRestorePoint: vi.fn(async () => ({ status: 'not_needed' as const })),
    clean: vi.fn(async (ids: string[]) => ({
      groups: ids.map((id) => ({ id, report: { bytes_freed: GB, files_deleted: 3, skipped_locked: 0, errors: [], dry_run: false }, error: null })),
      log_path: 'C:\\x.log',
      dry_run: false,
      log_write_failed: false,
    })),
    openLogFolder: vi.fn(async () => {}),
    ...over,
  };
}

describe('App', () => {
  it('đi trọn luồng Chào → Quét → Xem trước → Dọn → Kết quả → Về đầu', async () => {
    const api = fakeApi();
    render(<App api={api} />);
    expect(await screen.findByText('Ổ C: còn trống 50 GB')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Quét' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Dọn 1 GB' }));
    expect(await screen.findByText('Đã lấy lại')).toBeTruthy();
    expect(await screen.findByText('2 GB')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Về đầu' }));
    expect(await screen.findByRole('button', { name: 'Quét' })).toBeTruthy();
  }, 20000);

  it('chế độ chạy thử có nhãn trên đầu', async () => {
    render(<App api={fakeApi({ appInfo: vi.fn(async () => ({ version: '0.1.0', dry_run: true, system_drive: 'C:\\' })) })} />);
    expect(await screen.findByText('Chạy thử — không xóa gì')).toBeTruthy();
  });

  it('lỗi JavaScript ngoài luồng hiện băng trên màn', async () => {
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    render(<App api={fakeApi()} />);
    await screen.findByText('Ổ C: còn trống 50 GB');
    window.dispatchEvent(new ErrorEvent('error', { message: 'chart is not defined', filename: 'app.js', lineno: 7 }));
    expect(await screen.findByText('chart is not defined (app.js:7)')).toBeTruthy();
    err.mockRestore();
    warn.mockRestore();
  });
});
