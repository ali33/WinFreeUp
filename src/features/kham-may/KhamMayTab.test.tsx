import { describe, expect, it, vi } from 'vitest';
import { configure, fireEvent, render, screen } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { KhamMayApi } from './api/types';
import { fakeApi } from './testing/fakeApi';
import { KhamMayTab } from './KhamMayTab';

configure({ asyncUtilTimeout: 5000 });

function setup(over: Partial<KhamMayApi> = {}) {
  const api = fakeApi(over);
  const notify = vi.fn();
  const onGoClean = vi.fn();
  const ui = (active: boolean) => (
    <FluentProvider theme={webLightTheme}>
      <KhamMayTab api={api} notify={notify} onGoClean={onGoClean} active={active} />
    </FluentProvider>
  );
  const view = render(ui(true));
  return { api, notify, onGoClean, setActive: (a: boolean) => view.rerender(ui(a)) };
}

describe('Tab Khám máy', () => {
  it('mặc định mở Tổng quan và khám ngay', async () => {
    const { api } = setup();
    expect((screen.getByRole('tab', { name: 'Tổng quan' }) as HTMLElement).getAttribute('aria-selected')).toBe('true');
    await vi.waitFor(() => expect(api.healthCheck).toHaveBeenCalledTimes(1));
    expect(api.perfStart).not.toHaveBeenCalled();
  });

  it('Bộ nhớ & Hiệu năng chỉ lấy mẫu khi đang xem; quay lại Tổng quan không khám lại', async () => {
    const { api } = setup();
    await vi.waitFor(() => expect(api.healthCheck).toHaveBeenCalled());
    fireEvent.click(screen.getByRole('tab', { name: 'Bộ nhớ & Hiệu năng' }));
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole('tab', { name: 'Tổng quan' }));
    await vi.waitFor(() => expect(api.perfStop).toHaveBeenCalledTimes(1));
    expect(api.healthCheck).toHaveBeenCalledTimes(1);
  });

  it('App chuyển sang tab khác (active = false) thì cũng dừng lấy mẫu, quay lại thì chạy tiếp', async () => {
    const { api, setActive } = setup();
    fireEvent.click(screen.getByRole('tab', { name: 'Bộ nhớ & Hiệu năng' }));
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalledTimes(1));
    setActive(false);
    await vi.waitFor(() => expect(api.perfStop).toHaveBeenCalledTimes(1));
    setActive(true);
    await vi.waitFor(() => expect(api.perfStart).toHaveBeenCalledTimes(2));
    expect((screen.getByRole('tab', { name: 'Bộ nhớ & Hiệu năng' }) as HTMLElement).getAttribute('aria-selected')).toBe('true');
    expect(api.healthCheck).toHaveBeenCalledTimes(1);
  });

  it('App đang ở tab khác thì mở mục Bộ nhớ & Hiệu năng cũng không lấy mẫu', async () => {
    const { api, setActive } = setup();
    setActive(false);
    fireEvent.click(screen.getByRole('tab', { name: 'Bộ nhớ & Hiệu năng' }));
    await new Promise((r) => setTimeout(r, 50));
    expect(api.perfStart).not.toHaveBeenCalled();
  });

  it('Ổ đĩa giữ nguyên khi chuyển mục qua lại, không liệt kê ổ lại', async () => {
    const { api } = setup();
    fireEvent.click(screen.getByRole('tab', { name: 'Ổ đĩa' }));
    await vi.waitFor(() => expect(api.diskVolumes).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole('tab', { name: 'Tổng quan' }));
    fireEvent.click(screen.getByRole('tab', { name: 'Ổ đĩa' }));
    await new Promise((r) => setTimeout(r, 50));
    expect(api.diskVolumes).toHaveBeenCalledTimes(1);
  });

  it('nút trên dòng khám dẫn sang mục Ổ đĩa hoặc sang tab Dọn dẹp', async () => {
    const { api, onGoClean } = setup({
      healthCheck: vi.fn(async () => [{ id: 'disk_full' as const, level: 'critical' as const, value: 3, detail: null }]),
    });
    fireEvent.click(await screen.findByRole('button', { name: 'Dọn dẹp' }));
    expect(onGoClean).toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Xem Ổ đĩa' }));
    expect((screen.getByRole('tab', { name: 'Ổ đĩa' }) as HTMLElement).getAttribute('aria-selected')).toBe('true');
    await vi.waitFor(() => expect(api.diskVolumes).toHaveBeenCalled());
  });
});
