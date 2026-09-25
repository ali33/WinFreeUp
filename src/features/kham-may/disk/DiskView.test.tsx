import { describe, expect, it, vi } from 'vitest';
import { act, configure, fireEvent, render, screen, within } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import type { KhamMayApi, ScanSummary } from '../api/types';
import { deferred, fakeApi, ROOT } from '../testing/fakeApi';
import { DiskView, walkReasonText } from './DiskView';

// Hộp thoại Fluent vẽ chậm khi nhiều file test chạy song song trên máy yếu: nới thời gian chờ findBy*.
configure({ asyncUtilTimeout: 5000 });

function setup(over: Partial<KhamMayApi> = {}) {
  const api = fakeApi(over);
  const notify = vi.fn();
  const wrap = (ui: ReactNode) => render(<FluentProvider theme={webLightTheme}>{ui}</FluentProvider>);
  wrap(<DiskView api={api} notify={notify} />);
  return { api, notify };
}

async function scanned(over: Partial<KhamMayApi> = {}) {
  const s = setup(over);
  fireEvent.click(await screen.findByRole('button', { name: 'Quét' }));
  await screen.findByText('Users');
  return s;
}

/** Dòng bảng chứa `name`. Nút trong dòng và trong hộp thoại tra với `hidden: true`: bộ quản lý modal của
 *  Fluent (tabster) trong jsdom có lúc gắn `aria-hidden` nhầm chỗ khi nhiều file test chạy song song. */
function row(name: string): HTMLElement {
  return screen.getByText(name).closest('tr') as HTMLElement;
}

async function openTo(...names: string[]) {
  for (const n of names) {
    fireEvent.click(await screen.findByRole('button', { name: `Mở ${n}` }));
  }
}

describe('Ổ đĩa', () => {
  it('liệt kê ổ, chọn sẵn C:, quét rồi hiện tầng đầu có 🔒 và dòng tổng', async () => {
    const { api } = await scanned();
    expect(api.diskScan).toHaveBeenCalledWith('C:\\', expect.any(Function));
    expect(api.treeChildren).toHaveBeenCalledWith(0);
    expect(within(row('Windows')).getByText('🔒')).toBeTruthy();
    expect(within(row('Users')).getByText('🔒')).toBeTruthy();
    expect(screen.getByText('(+3 mục nhỏ khác, 2 GB)')).toBeTruthy();
    expect(screen.getByText('Quét xong trong 4,2 giây.')).toBeTruthy();
  });

  it('quét chậm thì băng hổ phách nêu lý do nguyên văn', async () => {
    const plan = { mode: 'walk' as const, reason: { code: 'mft_failed' as const, message: 'Access is denied. (os error 5)' } };
    const { notify } = await scanned({ diskScan: vi.fn(async () => ({ root: ROOT, plan, elapsed_ms: 1 })) });
    expect(notify).toHaveBeenCalledWith('warning', 'Đang dùng chế độ quét chậm vì không đọc được bảng MFT (Access is denied. (os error 5)).');
    expect(walkReasonText({ mode: 'walk', reason: { code: 'not_ntfs', fs: 'FAT32' } })).toContain('FAT32');
  });

  it('đang quét có vòng quay, thanh tiến độ, số file và nút Hủy', async () => {
    const run = deferred<ScanSummary>();
    let report: (s: { files: number; bytes: number; current: string; percent: number | null }) => void = () => {};
    const { api } = setup({
      diskScan: vi.fn((_r: string, cb) => {
        report = cb;
        return run.promise;
      }),
    });
    fireEvent.click(await screen.findByRole('button', { name: 'Quét' }));
    act(() => report({ files: 12345, bytes: 3 * 1024 ** 3, current: 'C:\\Windows\\WinSxS', percent: 40 }));
    expect(screen.getAllByRole('progressbar').some((e) => e.getAttribute('aria-valuenow') === '0.4')).toBe(true);
    expect(screen.getByText(/12\.345 file · 3 GB/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Hủy' }));
    expect(api.diskScanCancel).toHaveBeenCalled();
    expect(await screen.findByRole('button', { name: 'Đang hủy…' })).toBeTruthy();
    await act(async () => run.reject('cancelled'));
    expect(await screen.findByRole('button', { name: 'Quét' })).toBeTruthy();
  });

  it('hủy giữa chừng không báo lỗi; lỗi thật thì băng đỏ nguyên văn', async () => {
    const { notify } = setup({ diskScan: vi.fn(async () => Promise.reject('cancelled')) });
    fireEvent.click(await screen.findByRole('button', { name: 'Quét' }));
    await screen.findByRole('button', { name: 'Quét' });
    expect(notify).not.toHaveBeenCalled();
  });

  it('quét lại giữ cây cũ mờ dưới lớp phủ, không xoá trắng, khóa nút xóa', async () => {
    const again = deferred<ScanSummary>();
    const { api } = await scanned();
    vi.mocked(api.diskScan).mockImplementationOnce(() => again.promise);
    fireEvent.click(screen.getByRole('button', { name: 'Quét lại' }));
    expect(await screen.findByRole('button', { name: 'Hủy' })).toBeTruthy();
    expect(row('Users').closest('.km-overlay-host')?.getAttribute('aria-busy')).toBe('true');
    const dels = screen.getAllByRole('button', { name: 'Xóa vào Thùng rác', hidden: true });
    expect(dels.every((b) => (b as HTMLButtonElement).disabled)).toBe(true);
    await act(async () => again.reject('cancelled'));
    expect(await screen.findByRole('button', { name: 'Quét lại' })).toBeTruthy();
    expect(screen.getByText('Users')).toBeTruthy();
  });

  it('hủy không được thì trả nút Hủy lại và báo hổ phách', async () => {
    const run = deferred<ScanSummary>();
    const { notify } = setup({ diskScan: vi.fn(() => run.promise), diskScanCancel: vi.fn(async () => Promise.reject('no_scan')) });
    fireEvent.click(await screen.findByRole('button', { name: 'Quét' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Hủy' }));
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('warning', 'no_scan'));
    expect((screen.getByRole('button', { name: 'Hủy' }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('lỗi quét thật lên băng đỏ', async () => {
    const { notify } = setup({ diskScan: vi.fn(async () => Promise.reject('The device is not ready.')) });
    fireEvent.click(await screen.findByRole('button', { name: 'Quét' }));
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('error', 'Không quét được: The device is not ready.'));
  });

  it('mở tầng con, xóa vào Thùng rác: hộp xác nhận ghi tên, dung lượng, số file; xong trừ ngay', async () => {
    const { api } = await scanned();
    await openTo('Users', 'an');
    await screen.findByText('Downloads');
    fireEvent.click(within(row('Downloads')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText(/«Downloads» · 30 GB · 4 file/)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    await screen.findByText('Đã chuyển «Downloads» (30 GB) vào Thùng rác.');
    expect(api.diskDelete).toHaveBeenCalledWith(3);
    expect(screen.queryByText('Downloads')).toBeNull();
    expect(within(row('Users')).getByText('0 MB')).toBeTruthy();
    expect(within(row('C:\\')).getByText('30 GB')).toBeTruthy();
  });

  it('dòng được bảo vệ khóa nút xóa; đang xóa thì khóa mọi nút xóa khác', async () => {
    const del = deferred<never>();
    await scanned({ diskDelete: vi.fn(() => del.promise) });
    expect((within(row('Windows')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }) as HTMLButtonElement).disabled).toBe(true);
    await openTo('Users', 'an');
    await screen.findByText('Downloads');
    fireEvent.click(within(row('Downloads')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    expect(await screen.findByText('Đang xóa…')).toBeTruthy();
    const others = screen.getAllByRole('button', { name: 'Xóa vào Thùng rác', hidden: true });
    expect(others.every((b) => (b as HTMLButtonElement).disabled)).toBe(true);
  });

  it('xóa bị từ chối vì bảo vệ thì băng đỏ câu dễ hiểu, cây giữ nguyên', async () => {
    const { notify } = await scanned({ diskDelete: vi.fn(async () => Promise.reject('protected')) });
    await openTo('Users', 'an');
    await screen.findByText('Downloads');
    fireEvent.click(within(row('Downloads')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Xóa vào Thùng rác', hidden: true }));
    await vi.waitFor(() =>
      expect(notify).toHaveBeenCalledWith('error', 'Không xóa được «Downloads»: Đây là thư mục của Windows hoặc của chương trình — không xóa được.'),
    );
    expect(screen.getByText('Downloads')).toBeTruthy();
  });

  it('mở tầng hỏng thì băng hổ phách, cây cũ vẫn còn', async () => {
    const { notify } = await scanned({
      treeChildren: vi.fn(async (id: number) => {
        if (id === 0) return (await import('../testing/fakeApi')).PAGES[0];
        throw 'unknown_node';
      }),
    });
    fireEvent.click(screen.getByRole('button', { name: 'Mở Users' }));
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('warning', 'Không mở được thư mục: Cây thư mục đã cũ, hãy quét lại.'));
    expect(screen.getByText('Windows')).toBeTruthy();
  });

  it('hiberfil.sys có gợi ý tắt ngủ đông, chỉ là chữ', async () => {
    const { api } = await scanned();
    expect(screen.getByText(/Tắt ngủ đông để lấy lại 8 GB/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Cách làm' }));
    expect(await screen.findByText(/powercfg \/hibernate off/)).toBeTruthy();
    expect(api.diskDelete).not.toHaveBeenCalled();
  });

  it('mở Explorer và sao chép đường dẫn', async () => {
    const { api } = await scanned();
    fireEvent.click(within(row('Users')).getByRole('button', { name: 'Mở trong Explorer', hidden: true }));
    fireEvent.click(within(row('Users')).getByRole('button', { name: 'Sao chép đường dẫn', hidden: true }));
    expect(api.diskReveal).toHaveBeenCalledWith(1);
    await vi.waitFor(() => expect(api.copyText).toHaveBeenCalledWith('C:\\Users'));
    expect(await screen.findByText('Đã chép đường dẫn.')).toBeTruthy();
  });

  it('không đọc được danh sách ổ thì băng đỏ', async () => {
    const { notify } = setup({ diskVolumes: vi.fn(async () => Promise.reject('RPC failed')) });
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('error', 'Không đọc được danh sách ổ đĩa: RPC failed'));
  });
});
