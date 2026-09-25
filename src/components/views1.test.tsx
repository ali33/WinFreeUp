import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import type { GroupScan } from '../api/types';
import { initialState, reducer, type Action, type State } from '../state/machine';
import { WelcomeView } from './WelcomeView';
import { ScanningView } from './ScanningView';
import { PreviewView } from './PreviewView';
import { ConfirmDialog } from './ConfirmDialog';

const GB = 1024 ** 3;
const wrap = (ui: ReactNode) => render(<FluentProvider theme={webLightTheme}>{ui}</FluentProvider>);
const run = (s: State, ...a: Action[]) => a.reduce(reducer, s);

function scan(id: string, risk: GroupScan['risk'], bytes: number, extra: Partial<GroupScan> = {}): GroupScan {
  return {
    id,
    risk,
    default_selected: risk === 'safe',
    result: {
      total_bytes: bytes,
      file_count: 2,
      top_items: [
        { path: 'C:\\Users\\a\\AppData\\Local\\Temp\\setup-lon.exe', bytes: bytes * 0.75 },
        { path: 'C:\\Users\\a\\AppData\\Local\\Temp\\nho.tmp', bytes: bytes * 0.25 },
      ],
      estimated: false,
      notices: [],
    },
    error: null,
    ...extra,
  };
}

const SCANS = [
  scan('user_temp', 'safe', GB),
  scan('browser_cache', 'safe', 0, { result: null, error: 'Access is denied. (os error 5)' }),
  scan('component_store', 'caution', 2 * GB, { result: { total_bytes: 2 * GB, file_count: 0, top_items: [], estimated: true, notices: [] } }),
  scan('recycle_bin', 'caution', GB / 2),
  scan('windows_old', 'risky', 10 * GB),
];
const preview = () => run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_FINISHED', groups: SCANS });

describe('WelcomeView', () => {
  it('đang đọc dung lượng thì có vòng quay; đọc xong thì hiện số', () => {
    const { rerender } = wrap(<WelcomeView state={initialState} onScan={() => {}} />);
    expect(screen.getByRole('progressbar')).toBeTruthy();
    rerender(
      <FluentProvider theme={webLightTheme}>
        <WelcomeView state={reducer(initialState, { type: 'FREE_SPACE', bytes: 1.5 * GB })} onScan={() => {}} />
      </FluentProvider>,
    );
    expect(screen.getByText('Ổ C: còn trống 1,5 GB')).toBeTruthy();
  });

  it('không đọc được thì nói rõ, không quay mãi', () => {
    wrap(<WelcomeView state={reducer(initialState, { type: 'FREE_SPACE_FAILED' })} onScan={() => {}} />);
    expect(screen.queryByRole('progressbar')).toBeNull();
    expect(screen.getByText('Chưa đọc được dung lượng trống của ổ đĩa.')).toBeTruthy();
  });

  it('bấm Quét', () => {
    const onScan = vi.fn();
    wrap(<WelcomeView state={initialState} onScan={onScan} />);
    fireEvent.click(screen.getByRole('button', { name: 'Quét' }));
    expect(onScan).toHaveBeenCalled();
  });
});

describe('ScanningView', () => {
  it('mỗi nhóm đang chờ có vòng quay riêng; nhóm xong hiện dung lượng; nút Hủy', () => {
    const s = run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_GROUP_DONE', group: SCANS[0] });
    const onCancel = vi.fn();
    wrap(<ScanningView state={s} onCancel={onCancel} />);
    expect(screen.getAllByRole('progressbar')).toHaveLength(8);
    expect(screen.getByText('1 GB')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Hủy' }));
    expect(onCancel).toHaveBeenCalled();
  });

  it('đang hủy thì thay nút bằng vòng quay «Đang hủy…»', () => {
    const s = run(initialState, { type: 'SCAN_STARTED' }, { type: 'SCAN_CANCEL_REQUESTED' });
    wrap(<ScanningView state={s} onCancel={() => {}} />);
    expect(screen.queryByRole('button', { name: 'Hủy' })).toBeNull();
    expect(screen.getByText('Đang hủy…')).toBeTruthy();
  });
});

describe('PreviewView', () => {
  it('nút Dọn luôn đúng số đang chọn', () => {
    const s = preview();
    const { rerender } = wrap(<PreviewView state={s} onToggle={() => {}} onClean={() => {}} onRescan={() => {}} />);
    expect(screen.getByRole('button', { name: 'Dọn 1 GB' })).toBeTruthy();
    rerender(
      <FluentProvider theme={webLightTheme}>
        <PreviewView state={reducer(s, { type: 'TOGGLE', id: 'recycle_bin' })} onToggle={() => {}} onClean={() => {}} onRescan={() => {}} />
      </FluentProvider>,
    );
    expect(screen.getByRole('button', { name: 'Dọn 1,5 GB' })).toBeTruthy();
  });

  it('tích ô gọi onToggle; nhóm lỗi không tích được và hiện lỗi nguyên văn', () => {
    const onToggle = vi.fn();
    wrap(<PreviewView state={preview()} onToggle={onToggle} onClean={() => {}} onRescan={() => {}} />);
    fireEvent.click(screen.getByRole('checkbox', { name: 'Thùng rác' }));
    expect(onToggle).toHaveBeenCalledWith('recycle_bin');
    expect((screen.getByRole('checkbox', { name: 'Bộ nhớ đệm trình duyệt' }) as HTMLInputElement).disabled).toBe(true);
    expect(screen.getByText('Không quét được nhóm này: Access is denied. (os error 5)')).toBeTruthy();
  });

  it('nhãn mức rủi ro, số ước tính và danh sách 20 mục lớn nhất', () => {
    wrap(<PreviewView state={preview()} onToggle={() => {}} onClean={() => {}} onRescan={() => {}} />);
    expect(screen.getByText('Rủi ro')).toBeTruthy();
    expect(screen.getAllByText('Cân nhắc')).toHaveLength(2);
    expect(screen.getByText('khoảng 2 GB')).toBeTruthy();
    fireEvent.click(screen.getAllByRole('button', { name: 'Xem 20 mục lớn nhất' })[0]);
    expect(screen.getByText('C:\\Users\\a\\AppData\\Local\\Temp\\setup-lon.exe')).toBeTruthy();
  });

  it('bỏ hết lựa chọn thì nút Dọn bị khoá', () => {
    const s = reducer(preview(), { type: 'TOGGLE', id: 'user_temp' });
    wrap(<PreviewView state={s} onToggle={() => {}} onClean={() => {}} onRescan={() => {}} />);
    expect((screen.getByRole('button', { name: 'Dọn 0 MB' }) as HTMLButtonElement).disabled).toBe(true);
  });
});

describe('ConfirmDialog', () => {
  it('nhóm Cân nhắc: liệt kê nhóm, nút Dọn bấm được ngay', () => {
    const s = run(preview(), { type: 'TOGGLE', id: 'recycle_bin' }, { type: 'REQUEST_CLEAN' });
    const onAccept = vi.fn();
    wrap(<ConfirmDialog state={s} onAccept={onAccept} onCancel={() => {}} />);
    expect(screen.getByText('Thùng rác')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Dọn' }));
    expect(onAccept).toHaveBeenCalled();
  });

  // Review Focus 1
  it('nhóm Rủi ro: phải gõ XOA — chấp nhận «xóa», không chấp nhận «XO»', async () => {
    const user = userEvent.setup();
    const s = run(preview(), { type: 'TOGGLE', id: 'windows_old' }, { type: 'REQUEST_CLEAN' });
    wrap(<ConfirmDialog state={s} onAccept={() => {}} onCancel={() => {}} />);
    const ok = () => screen.getByRole('button', { name: 'Dọn' }) as HTMLButtonElement;
    const box = screen.getByRole('textbox', { name: 'Gõ XOA để xác nhận' });
    expect(ok().disabled).toBe(true);
    await user.type(box, 'XO');
    expect(ok().disabled).toBe(true);
    await user.clear(box);
    await user.type(box, 'xóa');
    expect(ok().disabled).toBe(false);
  });

  it('Quay lại gọi onCancel', () => {
    const s = run(preview(), { type: 'TOGGLE', id: 'recycle_bin' }, { type: 'REQUEST_CLEAN' });
    const onCancel = vi.fn();
    wrap(<ConfirmDialog state={s} onAccept={() => {}} onCancel={onCancel} />);
    fireEvent.click(screen.getByRole('button', { name: 'Quay lại' }));
    expect(onCancel).toHaveBeenCalled();
  });
});
