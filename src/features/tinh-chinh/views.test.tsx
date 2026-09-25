import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import { TinhChinhView } from './TinhChinhView';
import { readResult, report, sample, tw } from './testdata';
import { itemName } from './labels';
import type { RunReport, TweakApi, TweakEvent, TweakView } from './types';

/** Nút chân trang khoá bằng `disabledFocusable` (aria-disabled) để focus không rơi về body. */
const off = (el: HTMLElement) => (el as HTMLButtonElement).disabled || el.getAttribute('aria-disabled') === 'true';

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
    expect(off(screen.getByRole('button', { name: /Hoàn tác đã chọn/ }))).toBe(true);
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
    expect(off(screen.getByRole('button', { name: 'Áp dụng 1 thay đổi' }))).toBe(true);
    const row = screen.getByTestId('tweak-app_clipchamp');
    expect((within(row).getByRole('button', { name: 'Cài lại từ Store' }) as HTMLButtonElement).disabled).toBe(true);
    expect((within(screen.getByTestId('tweak-ads_id')).getByRole('checkbox') as HTMLInputElement).disabled).toBe(false);
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

  it('đọc lại sau khi chạy hỏng ⇒ vẫn giữ bảng kết quả, hiện lỗi nguyên văn và nút Đọc lại', async () => {
    const read = vi.fn().mockResolvedValueOnce(readResult()).mockRejectedValueOnce('Access is denied.').mockResolvedValue(readResult());
    wrap(<TinhChinhView api={fakeApi({ read })} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    expect(await screen.findByText('Không đọc được trạng thái máy: Access is denied.')).toBeTruthy();
    expect(screen.getByText('Kết quả')).toBeTruthy();
    expect(screen.getByText('✓ Xong')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Đọc lại' }));
    expect(await screen.findByText('Quyền riêng tư & quảng cáo')).toBeTruthy();
  });

  // Rà Task 11 (HIGH): chỉ báo phải nằm cạnh nút ở chân trang dính, không trôi khỏi khung nhìn khi cuộn.
  it('đang tạo điểm khôi phục ⇒ vòng quay trong chân trang, cạnh nút', async () => {
    const api = fakeApi({ prepareRestorePoint: vi.fn(() => new Promise<never>(() => {})) });
    const { container } = wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    const footer = container.querySelector('.tc-footer') as HTMLElement;
    expect(await within(footer).findByRole('progressbar')).toBeTruthy();
    expect(within(footer.querySelector('.tc-footer-status') as HTMLElement).getByText('Đang tạo điểm khôi phục hệ thống…')).toBeTruthy();
  });

  it('đang chạy ⇒ tiến độ «i/n tên mục» trong chân trang', async () => {
    const api = fakeApi({
      apply: vi.fn((_ids: string[], _all: boolean, onEvent: (e: TweakEvent) => void) => {
        onEvent({ kind: 'started', id: 'ads_id', index: 0, total: 1 });
        return new Promise<RunReport>(() => {});
      }),
    });
    const { container } = wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    const footer = container.querySelector('.tc-footer') as HTMLElement;
    expect(await within(footer).findByText(`Đang áp dụng 0/1… ${itemName('ads_id')}`)).toBeTruthy();
    expect(within(footer).getByRole('progressbar')).toBeTruthy();
    // Chỉ báo cho trình đọc màn hình khi đổi mục, không đọc lại «i/n» mỗi sự kiện.
    expect(footer.querySelector('[aria-live]')?.textContent).toBe(itemName('ads_id'));
  });

  it('không tạo được điểm khôi phục ⇒ băng hổ phách, chọn Vẫn áp dụng hoặc Dừng lại', async () => {
    const api = fakeApi({ prepareRestorePoint: vi.fn(async () => ({ status: 'failed' as const, message: 'System Protection is off' })) });
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    expect(await screen.findByText('Không tạo được điểm khôi phục')).toBeTruthy();
    expect(screen.getByText(/System Protection is off/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Dừng lại' }));
    expect(screen.queryByText('Không tạo được điểm khôi phục')).toBeNull();
    expect(api.apply).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Vẫn áp dụng' }));
    expect(await screen.findByText('Kết quả')).toBeTruthy();
    expect(api.apply).toHaveBeenCalledTimes(1);
  });

  it('đọc lại ⇒ phủ mờ, có vòng quay, danh sách cũ vẫn còn', async () => {
    const read = vi.fn().mockResolvedValueOnce(readResult()).mockReturnValue(new Promise(() => {}));
    const { container } = wrap(<TinhChinhView api={fakeApi({ read })} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    expect(await screen.findByText('Kết quả')).toBeTruthy();
    await waitFor(() => expect(container.querySelector('.tc-dim')).toBeTruthy());
    expect(within(container.querySelector('.tc-dim') as HTMLElement).getAllByRole('progressbar').length).toBeGreaterThan(0);
    expect(within(container.querySelector('.tc-footer') as HTMLElement).getByRole('progressbar')).toBeTruthy();
    expect(screen.getByText('Quyền riêng tư & quảng cáo')).toBeTruthy();
    expect(screen.getByTestId('tweak-ads_id')).toBeTruthy();
  });

  it('«mọi tài khoản» ⇒ ô tích có cảnh báo; hộp xác nhận liệt kê app, không lặp app đã ở mục Cân nhắc', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun={false} />);
    const box = await screen.findByRole('checkbox', { name: /Nâng cao: gỡ cho mọi tài khoản/ });
    expect(screen.getByText(/Khó hoàn tác; bản cập nhật Windows lớn/)).toBeTruthy();
    fireEvent.click(box);
    fireEvent.click(screen.getByRole('button', { name: 'Triệt để' }));
    fireEvent.click(screen.getByRole('button', { name: /^Áp dụng \d+ thay đổi$/ }));
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByText(/Gỡ cho mọi tài khoản và chặn cài lại/)).toBeTruthy();
    expect(within(dialog).getByText(itemName('app_weather'))).toBeTruthy();
    expect(within(dialog).getAllByText(itemName('app_game_bar'))).toHaveLength(1);
  });

  it('chạy bằng tài khoản admin khác ⇒ luôn hỏi lại, hộp xác nhận nhắc cảnh báo', async () => {
    const data = readResult(sample(), { system: { build: 26200, edition: 'Pro', managed: false, other_user: true } });
    wrap(<TinhChinhView api={fakeApi({ read: vi.fn(async () => data) })} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    const dialog = await screen.findByRole('alertdialog');
    expect(within(dialog).getByText(/tài khoản quản trị khác/)).toBeTruthy();
  });

  it.each([
    ['logoff', 'Đăng xuất rồi đăng nhập lại để hoàn tất.'],
    ['reboot', 'Khởi động lại máy để hoàn tất.'],
  ] as const)('cần %s ⇒ có lời nhắc', async (restart, text) => {
    wrap(<TinhChinhView api={fakeApi({ apply: vi.fn(async () => report({ restart })) })} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    expect(await screen.findByText(text)).toBeTruthy();
  });

  it('đang khởi động lại Explorer ⇒ vòng quay, nút Đóng khoá, focus không rơi về body', async () => {
    let done: () => void = () => {};
    const api = fakeApi({ restartExplorer: vi.fn(() => new Promise<void>((r) => (done = r))) });
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    const btn = await screen.findByRole('button', { name: 'Khởi động lại Explorer' });
    btn.focus();
    fireEvent.click(btn);
    expect(await screen.findByText('Đang khởi động lại Explorer…')).toBeTruthy();
    expect(off(screen.getByRole('button', { name: 'Đóng' }))).toBe(true);
    expect(document.activeElement).not.toBe(document.body);
    done();
    await waitFor(() => expect(off(screen.getByRole('button', { name: 'Đóng' }))).toBe(false));
  });

  // Lệch có chủ ý (yêu cầu sau rà Task 11): mục sai build đã áp dụng trước đó ⇒ tích được để hoàn tác.
  it('mục chỉ hoàn tác ⇒ nhãn không hỗ trợ kèm dòng hoàn tác được, ô tích bật, nút Hoàn tác đếm nó', async () => {
    const data = readResult([...sample(), tw('old_tweak', { status: 'unsupported', reason: 'build_max:19045', has_undo: true } as Partial<TweakView>)]);
    wrap(<TinhChinhView api={fakeApi({ read: vi.fn(async () => data) })} notify={vi.fn()} dryRun={false} />);
    const row = await screen.findByTestId('tweak-old_tweak');
    expect(within(row).getByText(/^Không hỗ trợ trên máy này/)).toBeTruthy();
    expect(within(row).getByText('Có thể hoàn tác thay đổi WinFreeUp đã làm trước đây')).toBeTruthy();
    const box = within(row).getByRole('checkbox') as HTMLInputElement;
    expect(box.disabled).toBe(false);
    fireEvent.click(box);
    expect(screen.getByRole('button', { name: 'Hoàn tác đã chọn (1)' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Áp dụng 1 thay đổi' })).toBeTruthy();
  });

  // Rà lần 2 (MEDIUM): «Đọc lại» sau khi đọc lại hỏng không được làm mất bảng kết quả.
  it('bấm Đọc lại khi đang xem kết quả ⇒ bảng kết quả còn nguyên trong lúc đọc và sau khi đọc xong', async () => {
    let finish: (r: ReturnType<typeof readResult>) => void = () => {};
    const read = vi
      .fn()
      .mockResolvedValueOnce(readResult())
      .mockRejectedValueOnce('Access is denied.')
      .mockReturnValueOnce(new Promise((r) => (finish = r)));
    wrap(<TinhChinhView api={fakeApi({ read })} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Đọc lại' }));
    expect(await screen.findByText('Đang đọc trạng thái máy… (liệt kê app mất vài giây)')).toBeTruthy();
    expect(screen.getByText('Kết quả')).toBeTruthy();
    finish(readResult());
    expect(await screen.findByText('Quyền riêng tư & quảng cáo')).toBeTruthy();
    expect(screen.getByText('Kết quả')).toBeTruthy();
  });

  // Rà lần 2 (MEDIUM): khung kết quả không bị dựng lại khi dữ liệu bị dọn ⇒ không mất trạng thái đang khởi động lại Explorer.
  it('đang khởi động lại Explorer mà đọc lại hỏng ⇒ vẫn khoá, không bấm được lần hai', async () => {
    let failRead: (e: unknown) => void = () => {};
    const read = vi
      .fn()
      .mockResolvedValueOnce(readResult())
      .mockReturnValueOnce(new Promise((_, rej) => (failRead = rej)));
    const api = fakeApi({ read, restartExplorer: vi.fn(() => new Promise<void>(() => {})) });
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Khởi động lại Explorer' }));
    expect(await screen.findByText('Đang khởi động lại Explorer…')).toBeTruthy();
    failRead('Access is denied.');
    expect(await screen.findByText('Không đọc được trạng thái máy: Access is denied.')).toBeTruthy();
    expect(screen.getByText('Đang khởi động lại Explorer…')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Khởi động lại Explorer' })).toBeNull();
    expect(off(screen.getByRole('button', { name: 'Đóng' }))).toBe(true);
    expect(api.restartExplorer).toHaveBeenCalledTimes(1);
  });

  // Rà lần 2 (LOW): đóng kết quả / dừng sau điểm khôi phục hỏng ⇒ focus về nút Áp dụng, không rơi về body.
  it('đóng kết quả ⇒ focus về nút Áp dụng', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    const close = await screen.findByRole('button', { name: 'Đóng' });
    await waitFor(() => expect(off(close)).toBe(false));
    close.focus();
    fireEvent.click(close);
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Áp dụng 1 thay đổi' })));
  });

  it('dừng sau điểm khôi phục hỏng ⇒ focus về nút Áp dụng', async () => {
    const api = fakeApi({ prepareRestorePoint: vi.fn(async () => ({ status: 'failed' as const, message: 'off' })) });
    wrap(<TinhChinhView api={api} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Dừng lại' }));
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Áp dụng 1 thay đổi' })));
  });

  // Rà lần 2 (LOW): khung kết quả là vùng có tên, không phải aria-live (đã nhận focus ⇒ không đọc hai lần).
  it('khung kết quả là region mang tên «Kết quả», không aria-live', async () => {
    wrap(<TinhChinhView api={fakeApi()} notify={vi.fn()} dryRun={false} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Áp dụng 1 thay đổi' }));
    const region = await screen.findByRole('region', { name: 'Kết quả' });
    expect(region.closest('[aria-live]')).toBeNull();
  });
});
