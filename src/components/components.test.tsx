import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import { NoticeBar } from './NoticeBar';
import { ErrorBoundary } from './ErrorBoundary';
import { Busy } from './Busy';
import { pushNotice } from '../errors/errors';

const wrap = (ui: ReactNode) => render(<FluentProvider theme={webLightTheme}>{ui}</FluentProvider>);

describe('NoticeBar', () => {
  it('hiện nguyên văn, số lần lặp và nút đóng', () => {
    const notices = pushNotice(pushNotice([], 'warning', 'Chrome đang mở'), 'warning', 'Chrome đang mở');
    const onDismiss = vi.fn();
    wrap(<NoticeBar notices={notices} onDismiss={onDismiss} />);
    expect(screen.getByText(/Chrome đang mở/).textContent).toContain('(×2)');
    fireEvent.click(screen.getByRole('button', { name: 'Đóng' }));
    expect(onDismiss).toHaveBeenCalledWith(notices[0].id);
  });

  it('không có gì thì không vẽ', () => {
    const { container } = wrap(<NoticeBar notices={[]} onDismiss={() => {}} />);
    expect(container.querySelector('.wfu-notices')).toBeNull();
  });
});

describe('ErrorBoundary', () => {
  it('lỗi khi vẽ thì hiện băng đỏ nguyên văn và nút Về đầu', () => {
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    const onReset = vi.fn();
    function Boom(): ReactNode {
      throw new Error('không đọc được top_items');
    }
    wrap(
      <ErrorBoundary onReset={onReset}>
        <Boom />
      </ErrorBoundary>,
    );
    expect(screen.getByText('Giao diện gặp lỗi: không đọc được top_items')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Về đầu' }));
    expect(onReset).toHaveBeenCalled();
    err.mockRestore();
  });
});

describe('Busy', () => {
  it('có vòng quay và nhãn', () => {
    wrap(<Busy label="Đang quét" />);
    expect(screen.getByRole('progressbar')).toBeTruthy();
    expect(screen.getByText('Đang quét')).toBeTruthy();
  });
});
