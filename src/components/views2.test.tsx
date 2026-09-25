import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { FluentProvider, webLightTheme } from '@fluentui/react-components';
import type { ReactNode } from 'react';
import type { CleanSummary, GroupClean, GroupScan } from '../api/types';
import { initialState, reducer, type Action, type State } from '../state/machine';
import { RestorePointView } from './RestorePointView';
import { CleaningView } from './CleaningView';
import { ResultView } from './ResultView';

const GB = 1024 ** 3;
const wrap = (ui: ReactNode) => render(<FluentProvider theme={webLightTheme}>{ui}</FluentProvider>);
const run = (s: State, ...a: Action[]) => a.reduce(reducer, s);

const scan = (id: string, risk: GroupScan['risk'], bytes: number): GroupScan => ({
  id,
  risk,
  default_selected: risk === 'safe',
  result: { total_bytes: bytes, file_count: 1, top_items: [], estimated: false, notices: [] },
  error: null,
});

const SCANS = [scan('user_temp', 'safe', GB), scan('component_store', 'caution', 2 * GB), scan('recycle_bin', 'caution', GB)];

const ok = (id: string, bytes: number, skipped = 0): GroupClean => ({
  id,
  report: { bytes_freed: bytes, files_deleted: 3, skipped_locked: skipped, errors: [], dry_run: false },
  error: null,
});

function atRestorePoint(): State {
  return run(
    initialState,
    { type: 'SCAN_STARTED' },
    { type: 'SCAN_FINISHED', groups: SCANS },
    { type: 'TOGGLE', id: 'recycle_bin' },
    { type: 'REQUEST_CLEAN' },
    { type: 'CONFIRM_ACCEPTED' },
  );
}

function atResult(summary: CleanSummary, free: { before?: number; after?: number } = {}): State {
  let s = run(
    initialState,
    { type: 'FREE_SPACE', bytes: free.before ?? 10 * GB },
    { type: 'SCAN_STARTED' },
    { type: 'SCAN_FINISHED', groups: SCANS },
    { type: 'REQUEST_CLEAN' },
    { type: 'CLEAN_DONE', summary },
  );
  if (free.after !== undefined) s = reducer(s, { type: 'FREE_AFTER', bytes: free.after });
  return s;
}

const SUMMARY: CleanSummary = { groups: [ok('user_temp', GB, 4)], log_path: 'C:\\Users\\a\\AppData\\Local\\WinFreeUp\\logs\\2026-09-25_101010.log', dry_run: false, log_write_failed: false };

describe('RestorePointView', () => {
  it('đang tạo điểm khôi phục thì có vòng quay', () => {
    wrap(<RestorePointView state={atRestorePoint()} onContinue={() => {}} onAbort={() => {}} />);
    expect(screen.getByRole('progressbar')).toBeTruthy();
    expect(screen.getByText('Đang tạo điểm khôi phục hệ thống…')).toBeTruthy();
  });

  it('không tạo được: băng hổ phách nguyên văn, người dùng chọn dọn tiếp hay dừng', () => {
    const s = reducer(atRestorePoint(), { type: 'RESTORE_RESULT', status: { status: 'failed', message: 'System Protection is off' } });
    const onContinue = vi.fn();
    const onAbort = vi.fn();
    wrap(<RestorePointView state={s} onContinue={onContinue} onAbort={onAbort} />);
    expect(screen.getByText(/System Protection is off/)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Vẫn dọn tiếp' }));
    fireEvent.click(screen.getByRole('button', { name: 'Dừng lại' }));
    expect(onContinue).toHaveBeenCalled();
    expect(onAbort).toHaveBeenCalled();
  });
});

describe('CleaningView', () => {
  it('tiến độ từng nhóm, DISM có thanh % riêng, không có nút nào', () => {
    let s = run(
      initialState,
      { type: 'SCAN_STARTED' },
      { type: 'SCAN_FINISHED', groups: SCANS },
      { type: 'TOGGLE', id: 'component_store' },
      { type: 'REQUEST_CLEAN' },
      { type: 'CONFIRM_ACCEPTED' },
      { type: 'RESTORE_RESULT', status: { status: 'created' } },
    );
    s = run(
      s,
      { type: 'CLEAN_EVENT', event: { kind: 'started', id: 'user_temp' } },
      { type: 'CLEAN_EVENT', event: { kind: 'finished', result: ok('user_temp', GB) } },
      { type: 'CLEAN_EVENT', event: { kind: 'started', id: 'component_store' } },
      { type: 'CLEAN_EVENT', event: { kind: 'percent', id: 'component_store', percent: 55.5 } },
    );
    wrap(<CleaningView state={s} />);
    expect(screen.getByText('56%')).toBeTruthy();
    expect(screen.getByText('1 GB')).toBeTruthy();
    expect(screen.queryAllByRole('button')).toHaveLength(0);
    expect(screen.getAllByRole('progressbar').length).toBeGreaterThanOrEqual(2);
  });
});

describe('ResultView', () => {
  it('GB thực lấy lại = trống sau − trước', () => {
    wrap(<ResultView state={atResult(SUMMARY, { before: 10 * GB, after: 12.5 * GB })} onOpenLog={async () => {}} onHome={() => {}} />);
    expect(screen.getByText('Đã lấy lại')).toBeTruthy();
    expect(screen.getByText('2,5 GB')).toBeTruthy();
    expect(screen.getByText('bỏ qua 4 file đang bị khóa')).toBeTruthy();
  });

  // Review Focus 5
  it('trống sau nhỏ hơn trước thì hiện 0, không số âm', () => {
    wrap(<ResultView state={atResult(SUMMARY, { before: 10 * GB, after: 9 * GB })} onOpenLog={async () => {}} onHome={() => {}} />);
    expect(screen.getByText('0 MB')).toBeTruthy();
    expect(screen.queryByText(/^\s*-\s*\d/)).toBeNull();
  });

  it('đang đo thì có vòng quay; đo hỏng thì nói rõ', () => {
    const measuring = atResult(SUMMARY);
    const { rerender } = wrap(<ResultView state={measuring} onOpenLog={async () => {}} onHome={() => {}} />);
    expect(screen.getByText('Đang đo dung lượng trống…')).toBeTruthy();
    rerender(
      <FluentProvider theme={webLightTheme}>
        <ResultView state={reducer(measuring, { type: 'FREE_AFTER_FAILED' })} onOpenLog={async () => {}} onHome={() => {}} />
      </FluentProvider>,
    );
    expect(screen.getByText('Không đo được dung lượng trống sau khi dọn.')).toBeTruthy();
  });

  it('chạy thử: hiện số lẽ ra lấy lại', () => {
    const dry: CleanSummary = { ...SUMMARY, dry_run: true, groups: [ok('user_temp', GB), ok('recycle_bin', GB / 2)] };
    wrap(<ResultView state={atResult(dry)} onOpenLog={async () => {}} onHome={() => {}} />);
    expect(screen.getByText('Chạy thử xong — lẽ ra lấy lại')).toBeTruthy();
    expect(screen.getByText('1,5 GB')).toBeTruthy();
  });

  it('nhóm not_scanned hiện câu tiếng Việt, không lộ mã thô', () => {
    const s: CleanSummary = { groups: [{ id: 'user_temp', report: null, error: 'not_scanned' }], log_path: 'x', dry_run: false, log_write_failed: false };
    wrap(<ResultView state={atResult(s)} onOpenLog={async () => {}} onHome={() => {}} />);
    expect(screen.queryByText(/not_scanned/)).toBeNull();
  });

  it('Xem nhật ký có vòng quay khi đang mở; Về đầu', async () => {
    let finish: () => void = () => {};
    const onOpenLog = vi.fn(() => new Promise<void>((r) => (finish = r)));
    const onHome = vi.fn();
    wrap(<ResultView state={atResult(SUMMARY, { after: 11 * GB })} onOpenLog={onOpenLog} onHome={onHome} />);
    fireEvent.click(screen.getByRole('button', { name: 'Xem nhật ký' }));
    expect(await screen.findByText('Đang mở…')).toBeTruthy();
    finish();
    expect(await screen.findByRole('button', { name: 'Xem nhật ký' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Về đầu' }));
    expect(onHome).toHaveBeenCalled();
  });
});
