import { StrictMode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { act, configure, fireEvent, render, screen, within } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { KhamMayApi, PerfTick } from '../api/types';
import { app, deferred, fakeApi, GB, tick } from '../testing/fakeApi';
import { PerfView } from './PerfView';

configure({ asyncUtilTimeout: 5000 });

function setup(over: Partial<KhamMayApi> = {}) {
  let push: (t: PerfTick) => void = () => {};
  let fail: (m: string) => void = () => {};
  const api = fakeApi({
    perfStart: vi.fn(async (onTick: (t: PerfTick) => void, onError: (m: string) => void) => {
      push = onTick;
      fail = onError;
      return [];
    }),
    ...over,
  });
  const notify = vi.fn();
  const view = render(
    <FluentProvider theme={webLightTheme}>
      <PerfView api={api} notify={notify} />
    </FluentProvider>,
  );
  const send = (t: PerfTick) => act(() => push(t));
  return { api, notify, view, send, fail: (m: string) => act(() => fail(m)) };
}

function appRow(name: string): HTMLElement {
  return screen.getByText(name).closest('tr') as HTMLElement;
}

const T0 = new Date(2026, 8, 25, 14, 3, 12).getTime();

describe('Bộ nhớ & Hiệu năng', () => {
  it('chưa có mẫu thì vòng quay; có mẫu thì bốn biểu đồ và bảng app sắp theo RAM', async () => {
    const { api, send } = setup();
    expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy();
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalledTimes(1));
    send(tick(T0));
    expect(screen.getByRole('img', { name: 'CPU: 20%' })).toBeTruthy();
    expect(screen.getByRole('img', { name: /^RAM: 50% · 8 GB \/ 16 GB$/ })).toBeTruthy();
    expect(screen.getByRole('img', { name: 'Hoạt động đĩa: 10%' })).toBeTruthy();
    expect(screen.getByRole('img', { name: 'Mạng: ↑ 1 KB/s · ↓ 4 KB/s' })).toBeTruthy();
    const names = screen.getAllByText(/Google Chrome|Service Host/).map((e) => e.textContent);
    expect(names).toEqual(['Google Chrome', 'Service Host']);
  });

  it('rời mục thì dừng lấy mẫu', async () => {
    const { api, view } = setup();
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalled());
    view.unmount();
    expect(api.perfStop).toHaveBeenCalledTimes(1);
  });

  it('sắp theo cột CPU khi bấm tiêu đề, bấm lần nữa thì đảo chiều', async () => {
    const { send } = setup();
    await vi.waitFor(() => expect(screen.queryByText('Đang bắt đầu đo…')).toBeTruthy());
    send(tick(T0, { apps: [app('a.exe', 'Nhẹ', 3 * GB, { cpu: 1 }), app('b.exe', 'Nặng', GB, { cpu: 80 })] }));
    fireEvent.click(screen.getByRole('button', { name: 'CPU' }));
    expect(screen.getAllByText(/^(Nhẹ|Nặng)$/).map((e) => e.textContent)).toEqual(['Nặng', 'Nhẹ']);
    fireEvent.click(screen.getByRole('button', { name: /^CPU/ }));
    expect(screen.getAllByText(/^(Nhẹ|Nặng)$/).map((e) => e.textContent)).toEqual(['Nhẹ', 'Nặng']);
  });

  it('tiến trình thiết yếu khóa nút Kết thúc; app thường kết thúc sau khi xác nhận', async () => {
    const { api, send } = setup();
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalled());
    send(tick(T0));
    const svc = within(appRow('Service Host')).getByRole('button', { name: 'Kết thúc app', hidden: true }) as HTMLButtonElement;
    expect(svc.disabled).toBe(true);
    expect(svc.title).toBe('Tiến trình thiết yếu của Windows');
    fireEvent.click(within(appRow('Google Chrome')).getByRole('button', { name: 'Kết thúc app', hidden: true }));
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText('Kết thúc «Google Chrome»?')).toBeTruthy();
    fireEvent.click(within(dialog).getByRole('button', { name: 'Kết thúc', hidden: true }));
    expect(await screen.findByText('Đã kết thúc «Google Chrome».')).toBeTruthy();
    expect(api.appKill).toHaveBeenCalledWith('chrome.exe');
  });

  it('đang kết thúc thì khóa nút khác; lõi từ chối thì băng đỏ câu dễ hiểu', async () => {
    const kill = deferred<never>();
    const { send, notify } = setup({ appKill: vi.fn(() => kill.promise) });
    await vi.waitFor(() => expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy());
    send(tick(T0, { apps: [app('a.exe', 'Một', 2 * GB), app('b.exe', 'Hai', GB)] }));
    fireEvent.click(within(appRow('Một')).getByRole('button', { name: 'Kết thúc app', hidden: true }));
    fireEvent.click(within(await screen.findByRole('dialog')).getByRole('button', { name: 'Kết thúc', hidden: true }));
    expect(await screen.findByText('Đang kết thúc…')).toBeTruthy();
    expect((within(appRow('Hai')).getByRole('button', { name: 'Kết thúc app', hidden: true }) as HTMLButtonElement).disabled).toBe(true);
    await act(async () => kill.reject('essential'));
    expect(notify).toHaveBeenCalledWith('error', 'Không kết thúc được «Một»: Đây là tiến trình thiết yếu của Windows — không kết thúc được.');
  });

  it('mở ra từng tiến trình và mở vị trí file', async () => {
    const { api, send } = setup();
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalled());
    send(tick(T0));
    fireEvent.click(screen.getByRole('button', { name: 'Xem các tiến trình của Google Chrome' }));
    expect(screen.getByText('chrome.exe · PID 100')).toBeTruthy();
    fireEvent.click(within(appRow('Google Chrome')).getByRole('button', { name: 'Mở vị trí file', hidden: true }));
    expect(api.revealPath).toHaveBeenCalledWith('C:\\Apps\\chrome.exe');
  });

  it('cơn giật: giờ, thời lượng, chỉ số, 3 app ngốn nhất, cờ hạ xung; dải đỏ trên cả bốn biểu đồ', async () => {
    const { send, view } = setup();
    await vi.waitFor(() => expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy());
    send(tick(T0 + 60_000));
    expect(screen.getByText('Chưa ghi nhận cơn giật nào trong 5 phút qua.')).toBeTruthy();
    send(
      tick(T0 + 61_000, {
        stutters: [
          {
            start_ms: T0,
            end_ms: T0 + 5000,
            cpu: true,
            disk: false,
            top_cpu: [
              { key: 'x', name: 'Game', avg: 71.4 },
              { key: 'y', name: 'Defender', avg: 20 },
            ],
            top_disk: [],
            throttled: true,
          },
        ],
      }),
    );
    expect(screen.getByText('14:03:12 · 5 giây · CPU')).toBeTruthy();
    expect(screen.getByText('Ngốn nhất: Game 71%, Defender 20%')).toBeTruthy();
    expect(screen.getByText('Trùng lúc hạ xung')).toBeTruthy();
    expect(view.container.querySelectorAll('[data-band]')).toHaveLength(4);
  });

  it('hạ xung, nhiệt độ và các nguồn không đo được', async () => {
    const { send, notify } = setup();
    await vi.waitFor(() => expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy());
    send(tick(T0, { throttled: true }));
    expect(screen.getByText(/CPU đang bị hạ xung/)).toBeTruthy();
    expect(screen.getByText('Nhiệt độ CPU: 55 °C')).toBeTruthy();
    const degraded = (t: number) =>
      tick(t, {
        throttled: null,
        perf_error: 'PDH \\Processor Information(_Total)\\% Processor Performance: 0xC0000BB8',
        temp: { state: 'unavailable', code: 'stuck', detail: '27.8' },
        net_app_error: 'ETW WinFreeUp-Net: Access is denied.',
        apps: [app('a.exe', 'Một', GB, { net_up_bps: null, net_down_bps: null })],
      });
    send(degraded(T0 + 1000));
    send(degraded(T0 + 2000));
    expect(screen.getByText('Máy này không cho đọc nhiệt độ')).toBeTruthy();
    expect(screen.getByText(/^Không đo được hạ xung: PDH/)).toBeTruthy();
    expect(screen.getByText('–')).toBeTruthy();
    expect(notify.mock.calls.filter((c) => c[1].startsWith('Không theo dõi được mạng'))).toHaveLength(1);
  });

  it('bắt đầu đo hỏng thì băng đỏ; lỗi từng mẫu thì hổ phách', async () => {
    const { notify } = setup({ perfStart: vi.fn(async () => Promise.reject('PDH open: 0xC0000BB8')) });
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith('error', 'Không bắt đầu đo được: PDH open: 0xC0000BB8'));
    const s = setup();
    await vi.waitFor(() => expect(s.api.perfStart).toHaveBeenCalled());
    s.fail('NtQuerySystemInformation failed: 0xC0000017');
    expect(s.notify).toHaveBeenCalledWith('warning', 'Lỗi khi lấy mẫu: NtQuerySystemInformation failed: 0xC0000017');
  });

  it('giãn chu kỳ thì báo cho người dùng biết', async () => {
    const { send } = setup();
    await vi.waitFor(() => expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy());
    send(tick(T0, { interval_ms: 2000 }));
    expect(screen.getByText('Đang lấy mẫu mỗi 2 giây để WinFreeUp không làm máy nặng thêm.')).toBeTruthy();
  });

  it('StrictMode: mẫu tới lắng nghe của lần gắn đã gỡ thì bị bỏ, không nhân đôi', async () => {
    const pushes: ((t: PerfTick) => void)[] = [];
    const api = fakeApi({
      perfStart: vi.fn(async (onTick: (t: PerfTick) => void) => {
        pushes.push(onTick);
        return [];
      }),
    });
    render(
      <StrictMode>
        <FluentProvider theme={webLightTheme}>
          <PerfView api={api} notify={vi.fn()} />
        </FluentProvider>
      </StrictMode>,
    );
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalledTimes(2));
    expect(api.perfStop).toHaveBeenCalledTimes(1);
    act(() => pushes[0](tick(T0)));
    expect(screen.getByText('Đang bắt đầu đo…')).toBeTruthy();
    act(() => pushes[1](tick(T0)));
    expect(screen.getByRole('img', { name: 'CPU: 20%' })).toBeTruthy();
  });
});
