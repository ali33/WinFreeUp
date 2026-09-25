import { describe, expect, it, vi } from 'vitest';
import { act, configure, fireEvent, render, screen, within } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import { StrictMode } from 'react';
import type { Finding, KhamMayApi, StartupEntry } from '../api/types';
import { deferred, fakeApi, FINDINGS } from '../testing/fakeApi';
import { OverviewView } from './OverviewView';
import { StartupList } from './StartupList';

configure({ asyncUtilTimeout: 5000 });

function setup(over: Partial<KhamMayApi> = {}) {
  const api = fakeApi(over);
  const notify = vi.fn();
  const onNavigate = vi.fn();
  render(
    <StrictMode>
      <FluentProvider theme={webLightTheme}>
        <OverviewView api={api} notify={notify} onNavigate={onNavigate} />
      </FluentProvider>
    </StrictMode>,
  );
  return { api, notify, onNavigate };
}

function findingRow(id: string): HTMLElement {
  return document.querySelector(`[data-finding="${id}"]`) as HTMLElement;
}

describe('Tổng quan', () => {
  it('khám đúng một lần kể cả StrictMode, hiện 9 dòng theo thứ tự bảng', async () => {
    const { api, notify } = setup();
    await screen.findByText(/Ổ hệ thống chỉ còn 9,5% trống/);
    expect(api.healthCheck).toHaveBeenCalledTimes(1);
    expect(api.healthThrottle).toHaveBeenCalledTimes(1);
    const ids = Array.from(document.querySelectorAll('[data-finding]')).map((e) => e.getAttribute('data-finding'));
    expect(ids).toEqual(['disk_full', 'disk_health', 'system_hdd', 'ram_pressure', 'startup_apps', 'uptime', 'power_saver', 'cpu_throttle', 'cpu_hot']);
    expect(await screen.findByText('0 vấn đề nghiêm trọng · 2 vấn đề nên xử lý')).toBeTruthy();
    await screen.findByText('(1/2 đang bật)');
    expect(notify).not.toHaveBeenCalledWith('error', expect.anything());
  });

  it('dòng nào xong hiện dòng đó; dòng chưa xong có vòng quay; hạ xung đo riêng 10 giây', async () => {
    const check = deferred<Finding[]>();
    const throttle = deferred<Finding>();
    let push: (f: Finding) => void = () => {};
    setup({
      healthCheck: vi.fn((cb: (f: Finding) => void) => {
        push = cb;
        return check.promise;
      }),
      healthThrottle: vi.fn(() => throttle.promise),
    });
    expect(await screen.findByText('Đang đo tốc độ CPU trong 10 giây…')).toBeTruthy();
    act(() => push(FINDINGS[0]));
    expect(within(findingRow('disk_full')).getByText('🟠 Nên xử lý')).toBeTruthy();
    expect(within(findingRow('disk_health')).getByRole('progressbar')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Khám lại' }) as HTMLButtonElement).disabled).toBe(true);
    await act(async () => check.resolve(FINDINGS));
    await act(async () => throttle.resolve({ id: 'cpu_throttle', level: 'warn', value: null, detail: null }));
    expect(within(findingRow('cpu_throttle')).getByText('CPU đang bị hạ xung (thường do nóng hoặc chế độ nguồn).')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Khám lại' }) as HTMLButtonElement).disabled).toBe(false);
  });

  it('nguồn không đo được ghi lý do nguyên văn', async () => {
    setup();
    expect(await screen.findByText('Không đo được: HRESULT Call failed with: 0x8004100C')).toBeTruthy();
    expect(within(findingRow('cpu_hot')).getByText('⚪ Không đo được')).toBeTruthy();
  });

  it('nút dẫn tới chỗ xử lý', async () => {
    const findings: Finding[] = [
      { id: 'disk_full', level: 'critical', value: 3, detail: null },
      { id: 'power_saver', level: 'warn', value: null, detail: null },
      { id: 'uptime', level: 'warn', value: 9, detail: null },
      { id: 'ram_pressure', level: 'warn', value: 90, detail: null },
    ];
    const { api, onNavigate } = setup({ healthCheck: vi.fn(async () => findings) });
    fireEvent.click(await screen.findByRole('button', { name: 'Dọn dẹp' }));
    expect(onNavigate).toHaveBeenCalledWith('clean');
    fireEvent.click(screen.getByRole('button', { name: 'Xem Ổ đĩa' }));
    expect(onNavigate).toHaveBeenCalledWith('disk');
    fireEvent.click(screen.getByRole('button', { name: 'Xem Bộ nhớ & Hiệu năng' }));
    expect(onNavigate).toHaveBeenCalledWith('perf');
    fireEvent.click(screen.getByRole('button', { name: 'Mở cài đặt Nguồn' }));
    expect(api.openSettings).toHaveBeenCalledWith('power');
    fireEvent.click(screen.getByRole('button', { name: 'Gợi ý' }));
    expect(await screen.findByText('Khởi động lại đúng cách')).toBeTruthy();
  });

  it('khám hỏng hẳn thì băng đỏ; đo hạ xung hỏng thì dòng đó «Không đo được»', async () => {
    const { notify } = setup({
      healthCheck: vi.fn(async () => Promise.reject('busy')),
      healthThrottle: vi.fn(async () => Promise.reject('PDH open: 0xC0000BB8')),
    });
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('error', 'Không khám được: WinFreeUp đang bận với thao tác trước, hãy đợi xong rồi thử lại.'));
    expect(await screen.findByText('Không đo được: PDH open: 0xC0000BB8')).toBeTruthy();
  });

  it('khám hỏng hẳn thì dòng chưa có kết quả ghi «Không đo được», không quay mãi', async () => {
    setup({ healthCheck: vi.fn(async () => Promise.reject('busy')) });
    await vi.waitFor(() =>
      expect(within(findingRow('disk_full')).getByText('Không đo được: WinFreeUp đang bận với thao tác trước, hãy đợi xong rồi thử lại.')).toBeTruthy(),
    );
    expect(within(findingRow('disk_full')).getByText('⚪ Không đo được')).toBeTruthy();
    await vi.waitFor(() => expect(document.querySelectorAll('[data-finding] [role="progressbar"]').length).toBe(0));
    expect(screen.queryByText('Không thấy vấn đề nào.')).toBeNull();
    expect(screen.getByText('Có mục chưa đo được — xem lý do ở từng dòng.')).toBeTruthy();
  });

  it('dòng lõi không trả kết quả thì «Không đo được» với lý do rõ', async () => {
    setup({ healthCheck: vi.fn(async () => [{ id: 'disk_full' as const, level: 'ok' as const, value: 40, detail: null }]) });
    await vi.waitFor(() =>
      expect(within(findingRow('disk_health')).getByText('Không đo được: Lõi không trả kết quả cho mục này.')).toBeTruthy(),
    );
    expect(within(findingRow('disk_health')).getByText('⚪ Không đo được')).toBeTruthy();
    expect(screen.queryByText('Không thấy vấn đề nào.')).toBeNull();
  });

  it('mọi dòng Ổn thì mới hiện «Không thấy vấn đề nào.»', async () => {
    const allOk: Finding[] = FINDINGS.map((f) => ({ ...f, level: 'ok' as const, detail: null, value: 1 }));
    setup({ healthCheck: vi.fn(async () => allOk) });
    expect(await screen.findByText('Không thấy vấn đề nào.')).toBeTruthy();
    expect(screen.queryByText('Có mục chưa đo được — xem lý do ở từng dòng.')).toBeNull();
  });

  it('bật/tắt app khởi động thì dòng startup_apps cập nhật số đếm', async () => {
    setup();
    await screen.findByText('12 app tự chạy cùng máy — máy khởi động chậm và tốn RAM.');
    fireEvent.click(await screen.findByRole('switch', { name: 'OneDrive' }));
    expect(await within(findingRow('startup_apps')).findByText('0 app tự chạy cùng máy.')).toBeTruthy();
    expect(within(findingRow('startup_apps')).getByText('🟢 Ổn')).toBeTruthy();
  });

  it('danh sách khởi động đọc thiếu nguồn thì không tự sửa số đếm', async () => {
    setup({
      startupList: vi.fn(async () => ({
        entries: [{ id: 'hkcu_run:OneDrive', source: 'hkcu_run' as const, name: 'OneDrive', command: 'x', enabled: true }],
        errors: ['RegOpenKeyExW: Access is denied.'],
      })),
    });
    await screen.findByText('12 app tự chạy cùng máy — máy khởi động chậm và tốn RAM.');
    fireEvent.click(await screen.findByRole('switch', { name: 'OneDrive' }));
    await screen.findByText('(0/1 đang bật)');
    expect(within(findingRow('startup_apps')).getByText('12 app tự chạy cùng máy — máy khởi động chậm và tốn RAM.')).toBeTruthy();
  });

  it('khám lại giữ kết quả cũ dưới lớp phủ mờ, dòng nào có kết quả mới thì thay dần', async () => {
    const second = deferred<Finding[]>();
    let push: (f: Finding) => void = () => {};
    const healthCheck = vi.fn(async (cb: (f: Finding) => void) => {
      FINDINGS.forEach(cb);
      return FINDINGS;
    });
    const { api } = setup({ healthCheck });
    await screen.findByText(/9,5%/);
    await vi.waitFor(() => expect((screen.getByRole('button', { name: 'Khám lại' }) as HTMLButtonElement).disabled).toBe(false));
    healthCheck.mockImplementationOnce((cb) => {
      push = cb;
      return second.promise;
    });
    fireEvent.click(screen.getByRole('button', { name: 'Khám lại' }));
    await vi.waitFor(() => expect(api.healthCheck).toHaveBeenCalledTimes(2));
    const list = document.querySelector('.km-findings') as HTMLElement;
    expect(list.getAttribute('aria-busy')).toBe('true');
    expect(document.querySelector('.km-overlay [role="progressbar"]')).toBeTruthy();
    // Kết quả cũ vẫn còn, không xoá trắng.
    expect(within(findingRow('disk_full')).getByText(/9,5%/)).toBeTruthy();
    expect(within(findingRow('uptime')).getByText('🟢 Ổn')).toBeTruthy();
    act(() => push({ id: 'disk_full', level: 'ok', value: 20, detail: null }));
    expect(within(findingRow('disk_full')).getByText('Ổ hệ thống còn 20% trống.')).toBeTruthy();
    await act(async () => second.resolve([{ id: 'disk_full', level: 'ok', value: 20, detail: null }]));
    await vi.waitFor(() => expect(list.getAttribute('aria-busy')).toBe('false'));
    expect(document.querySelector('.km-overlay')).toBeNull();
    // Lần khám mới không trả dòng uptime ⇒ không giữ số cũ như thể vừa đo.
    expect(within(findingRow('uptime')).getByText('Không đo được: Lõi không trả kết quả cho mục này.')).toBeTruthy();
  });
});

describe('Danh sách khởi động', () => {
  function renderList(over: Partial<KhamMayApi> = {}) {
    const api = fakeApi(over);
    const notify = vi.fn();
    render(
      <FluentProvider theme={webLightTheme}>
        <StartupList api={api} notify={notify} />
      </FluentProvider>,
    );
    return { api, notify };
  }

  it('hiện số app đang bật, nguồn và dòng lệnh; bật/tắt ghi qua lõi', async () => {
    const { api } = renderList();
    expect(await screen.findByText('(1/2 đang bật)')).toBeTruthy();
    expect(screen.getByText('· Riêng bạn')).toBeTruthy();
    const sw = screen.getByRole('switch', { name: 'OneDrive' }) as HTMLInputElement;
    expect(sw.checked).toBe(true);
    fireEvent.click(sw);
    await vi.waitFor(() => expect(api.startupSet).toHaveBeenCalledWith('hkcu_run:OneDrive', false));
    expect(await screen.findByText('(0/2 đang bật)')).toBeTruthy();
  });

  it('đổi không được thì băng đỏ nguyên văn, công tắc giữ trạng thái cũ', async () => {
    const { notify } = renderList({ startupSet: vi.fn(async () => Promise.reject('RegSetValueExW: Access is denied. (os error 5)')) });
    fireEvent.click(await screen.findByRole('switch', { name: 'OneDrive' }));
    await vi.waitFor(() =>
      expect(notify).toHaveBeenCalledWith('error', 'Không đổi được «OneDrive»: RegSetValueExW: Access is denied. (os error 5)'),
    );
    expect((screen.getByRole('switch', { name: 'OneDrive' }) as HTMLInputElement).checked).toBe(true);
  });

  it('đang bật/tắt thì có vòng quay và khoá công tắc đó', async () => {
    const set = deferred<StartupEntry>();
    renderList({ startupSet: vi.fn(() => set.promise) });
    fireEvent.click(await screen.findByRole('switch', { name: 'OneDrive' }));
    expect(await screen.findByRole('progressbar')).toBeTruthy();
    expect((screen.getByRole('switch', { name: 'OneDrive' }) as HTMLInputElement).disabled).toBe(true);
    await act(async () => set.resolve({ id: 'hkcu_run:OneDrive', source: 'hkcu_run', name: 'OneDrive', command: '', enabled: false }));
    expect(screen.queryByRole('progressbar')).toBeNull();
    expect((screen.getByRole('switch', { name: 'OneDrive' }) as HTMLInputElement).disabled).toBe(false);
  });

  it('đọc hỏng hẳn thì băng đỏ và ghi lỗi tại chỗ, không nói là không có app', async () => {
    const { notify } = renderList({ startupList: vi.fn(async () => Promise.reject('RegOpenKeyExW: Access is denied.')) });
    expect(await screen.findByText('Không đọc được danh sách khởi động: RegOpenKeyExW: Access is denied.')).toBeTruthy();
    expect(screen.queryByText('Không có app nào tự chạy cùng máy.')).toBeNull();
    expect(notify).toHaveBeenCalledWith('error', 'Không đọc được danh sách khởi động: RegOpenKeyExW: Access is denied.');
  });

  it('notify đổi thì không đọc lại danh sách', async () => {
    const api = fakeApi();
    const { rerender } = render(
      <FluentProvider theme={webLightTheme}>
        <StartupList api={api} notify={vi.fn()} />
      </FluentProvider>,
    );
    await screen.findByText('(1/2 đang bật)');
    rerender(
      <FluentProvider theme={webLightTheme}>
        <StartupList api={api} notify={vi.fn()} />
      </FluentProvider>,
    );
    await screen.findByText('(1/2 đang bật)');
    expect(api.startupList).toHaveBeenCalledTimes(1);
  });

  it('một nguồn đọc không được thì băng hổ phách, phần còn lại vẫn hiện', async () => {
    const { notify } = renderList({
      startupList: vi.fn(async () => ({ entries: [], errors: ['RegOpenKeyExW Software\\WOW6432Node: Access is denied.'] })),
    });
    expect(await screen.findByText('Không có app nào tự chạy cùng máy.')).toBeTruthy();
    expect(notify).toHaveBeenCalledWith('warning', 'Không đọc được danh sách khởi động: RegOpenKeyExW Software\\WOW6432Node: Access is denied.');
  });
});
