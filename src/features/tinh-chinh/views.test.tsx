import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import { TinhChinhView } from './TinhChinhView';
import { readResult, report, sample } from './testdata';
import type { RunReport, TweakApi } from './types';

const wrap = (ui: ReactNode) => render(<FluentProvider theme={webLightTheme}>{ui}</FluentProvider>);

function fakeApi(over: Partial<TweakApi> = {}): TweakApi {
  return {
    read: vi.fn(async () => readResult()),
    prepareRestorePoint: vi.fn(async () => ({ status: 'created' as const })),
    apply: vi.fn(async (ids: string[]) =>
      report({ outcomes: ids.map((id) => ({ id, status: 'applied' as const, errors: [], store_opened: [] })), restart: 'explorer' }),
    ),
    revert: vi.fn(async () => report()),
    restartExplorer: vi.fn(async () => {}),
    ...over,
  };
}

describe('TinhChinhView', () => {
  // Lệch kế hoạch (quyết định người dùng, Task 9/10): lần đầu chỉ tích mục quyền riêng tư của mức Cơ bản
  // (`defaultSelection`), không tích app ⇒ mẫu có app Cơ bản (app_clipchamp) nên nút mức hiện «Tùy chỉnh».
  it('lần đầu có vòng quay, rồi tích sẵn mục quyền riêng tư Cơ bản, app không có trên máy bị ẩn', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun={false} />);
    expect(screen.getByRole('progressbar')).toBeTruthy();
    expect(await screen.findByText('Gỡ app (2 đang có · 1 đã gỡ)')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Cơ bản' }).getAttribute('aria-pressed')).toBe('false');
    expect(screen.getByText('Tùy chỉnh')).toBeTruthy();
    expect(screen.queryByText('TikTok')).toBeNull();
    expect(screen.getByRole('button', { name: 'Áp dụng 1 thay đổi' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Hoàn tác đã chọn (0)' })).toBeTruthy();
  });

  it('bấm Cơ bản ⇒ tích cả app mức Cơ bản, nút mức được tô', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Cơ bản' }));
    expect(screen.getByRole('button', { name: 'Cơ bản' }).getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByRole('button', { name: 'Áp dụng 1 thay đổi' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Hoàn tác đã chọn (1)' })).toBeTruthy();
  });

  it('mục không hỗ trợ hiện lý do và không tích được', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun={false} />);
    const row = await screen.findByTestId('tweak-recall_off');
    expect(within(row).getByText('Không hỗ trợ trên máy này — cần Windows 11 24H2 trở lên')).toBeTruthy();
    expect((within(row).getByRole('checkbox') as HTMLInputElement).disabled).toBe(true);
  });

  it('bấm Khuyến nghị ⇒ đếm lại; tự bỏ một mục ⇒ «Tùy chỉnh»', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Khuyến nghị' }));
    expect(screen.getByRole('button', { name: 'Áp dụng 3 thay đổi' })).toBeTruthy();
    fireEvent.click(within(screen.getByTestId('tweak-ads_id')).getByRole('checkbox'));
    expect(screen.getByText('Tùy chỉnh')).toBeTruthy();
  });

  it('có mục Cân nhắc ⇒ hộp xác nhận liệt kê; đồng ý ⇒ áp dụng, hiện kết quả và nút khởi động lại Explorer', async () => {
    const api = fakeApi();
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Khuyến nghị' }));
    fireEvent.click(screen.getByRole('button', { name: 'Áp dụng 3 thay đổi' }));
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByText('Tắt dịch vụ gửi dữ liệu chẩn đoán (DiagTrack)')).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Áp dụng' }));
    expect(await screen.findByText('Kết quả')).toBeTruthy();
    expect(api.apply).toHaveBeenCalledWith(['ads_id', 'svc_diagtrack', 'app_weather'], false, expect.any(Function));
    fireEvent.click(screen.getByRole('button', { name: 'Khởi động lại Explorer' }));
    await waitFor(() => expect(api.restartExplorer).toHaveBeenCalled());
  });

  it('lỗi từng mục hiện nguyên văn trong kết quả', async () => {
    const api = fakeApi({
      apply: vi.fn(async () => report({ outcomes: [{ id: 'ads_id', status: 'not_applied', errors: ['ads_id#0: Access is denied.'], store_opened: [] }] })),
    });
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    expect(await screen.findByText('ads_id#0: Access is denied.')).toBeTruthy();
    expect(screen.getByText('✗ Lỗi')).toBeTruthy();
  });

  it('đang áp dụng ⇒ khoá mọi nút, có vòng quay, báo App đang bận', async () => {
    let finish: () => void = () => {};
    const api = fakeApi({ apply: vi.fn(() => new Promise<RunReport>((r) => (finish = () => r(report())))) });
    const onBusyChange = vi.fn();
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} onBusyChange={onBusyChange} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    expect(await screen.findByText(/Đang áp dụng 0\/1/)).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Khuyến nghị' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: /Hoàn tác đã chọn/ }) as HTMLButtonElement).disabled).toBe(true);
    expect(onBusyChange).toHaveBeenLastCalledWith(true);
    finish();
    expect(await screen.findByText('Kết quả')).toBeTruthy();
    expect(onBusyChange).toHaveBeenLastCalledWith(false);
  });

  it('app đã gỡ có nút «Cài lại từ Store»', async () => {
    const api = fakeApi();
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    const row = await screen.findByTestId('tweak-app_clipchamp');
    fireEvent.click(within(row).getByRole('button', { name: 'Cài lại từ Store' }));
    await waitFor(() => expect(api.revert).toHaveBeenCalledWith(['app_clipchamp'], expect.any(Function)));
  });

  it('chạy thử ⇒ chỉ xem, nút Áp dụng/Hoàn tác bị khoá', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun />);
    expect(await screen.findByText('Chạy thử: tab này chỉ xem trạng thái, không áp dụng hay hoàn tác.')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Áp dụng 1 thay đổi' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('chạy bằng tài khoản admin khác ⇒ băng hổ phách cảnh báo', async () => {
    const data = readResult(sample(), { system: { build: 26200, edition: 'Pro', managed: false, other_user: true } });
    wrap(<TinhChinhView api={fakeApi({ read: vi.fn(async () => data) })} notify={vi.fn()} dryRun={false} />);
    expect(await screen.findByText(/tài khoản quản trị khác/)).toBeTruthy();
  });

  it('đọc hỏng ⇒ báo đỏ qua notify và có nút Đọc lại', async () => {
    const notify = vi.fn();
    const read = vi.fn().mockRejectedValueOnce('Access is denied.').mockResolvedValue(readResult());
    wrap(<TinhChinhView api={fakeApi({ read })} notify={notify} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Đọc lại' }));
    expect(notify).toHaveBeenCalledWith('error', 'Không đọc được trạng thái máy: Access is denied.');
    expect(await screen.findByText('Quyền riêng tư & quảng cáo')).toBeTruthy();
  });

  it('đọc lại sau khi chạy hỏng ⇒ màn không trống: hiện lỗi nguyên văn và nút Đọc lại', async () => {
    const read = vi.fn().mockResolvedValueOnce(readResult()).mockRejectedValueOnce('Access is denied.').mockResolvedValue(readResult());
    wrap(<TinhChinhView api={fakeApi({ read })} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    expect(await screen.findByText('Không đọc được trạng thái máy: Access is denied.')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Đọc lại' }));
    expect(await screen.findByText('Quyền riêng tư & quảng cáo')).toBeTruthy();
  });
});
