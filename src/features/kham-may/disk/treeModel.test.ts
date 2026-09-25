import { describe, expect, it } from 'vitest';
import { GB, PAGES, ROOT, node } from '../testing/fakeApi';
import { applyDelete, collapse, isHiberfil, reopen, share, startTree, visibleRows, withPage } from './treeModel';

function opened() {
  let m = startTree(ROOT);
  for (const id of [0, 1, 2, 3]) m = withPage(m, PAGES[id]);
  return m;
}

describe('cây phía giao diện', () => {
  it('trải các tầng đang mở, dòng tổng ở cuối tầng', () => {
    const rows = visibleRows(withPage(startTree(ROOT), PAGES[0]));
    expect(rows.map((r) => (r.kind === 'node' ? r.node.name : 'rest'))).toEqual(['Users', 'Windows', 'hiberfil.sys', 'rest']);
    const rest = rows[3];
    expect(rest.kind === 'rest' && rest.rest.count).toBe(3);
  });

  it('mở sâu thì thụt lề, đóng thì ẩn con nhưng nhớ để mở lại', () => {
    const m = opened();
    const rows = visibleRows(m);
    const film = rows.find((r) => r.kind === 'node' && r.node.name === 'film.mkv');
    expect(film && film.depth).toBe(3);
    const closed = collapse(m, 1);
    expect(visibleRows(closed).some((r) => r.kind === 'node' && r.node.name === 'an')).toBe(false);
    const again = reopen(closed, 1);
    expect(again && visibleRows(again).some((r) => r.kind === 'node' && r.node.name === 'an')).toBe(true);
    expect(reopen(m, 99)).toBeNull();
  });

  it('sắp theo tên hoặc ngày sửa, mặc định theo dung lượng', () => {
    let m = startTree(ROOT);
    m = withPage(m, PAGES[0]);
    const byName = visibleRows(m, 'name').filter((r) => r.kind === 'node').map((r) => (r.kind === 'node' ? r.node.name : ''));
    expect(byName).toEqual(['hiberfil.sys', 'Users', 'Windows']);
  });

  it('% so với cha an toàn với cha 0 byte', () => {
    expect(share(5, 10)).toBe(50);
    expect(share(5, 0)).toBe(0);
    expect(share(20, 10)).toBe(100);
  });

  it('xóa: bỏ dòng và trừ dung lượng khỏi mọi tổ tiên tới gốc', () => {
    const m = applyDelete(opened(), 3, { bytes: 30 * GB, files: 4 });
    const names = visibleRows(m).map((r) => (r.kind === 'node' ? r.node.name : 'rest'));
    expect(names).not.toContain('Downloads');
    expect(names).not.toContain('film.mkv');
    expect(m.root?.bytes).toBe(30 * GB);
    expect(m.pages[0].items.find((i) => i.name === 'Users')?.bytes).toBe(0);
    expect(m.pages[1].parent.bytes).toBe(0);
    expect(m.pages[2].parent.files).toBe(0);
    expect(m.pages[3]).toBeUndefined();
  });

  it('xóa một thư mục thì dọn luôn trang và quan hệ cha của mọi con cháu', () => {
    const m = applyDelete(opened(), 2, { bytes: 30 * GB, files: 4 });
    expect(m.pages[2]).toBeUndefined();
    expect(m.pages[3]).toBeUndefined();
    expect(m.parentOf[2]).toBeUndefined();
    expect(m.parentOf[3]).toBeUndefined();
    expect(m.parentOf[4]).toBeUndefined();
    expect(m.expanded[3]).toBeUndefined();
    expect(m.pages[1].items).toEqual([]);
  });

  it('xóa một id không có trên màn thì không đổi gì', () => {
    const m = opened();
    expect(applyDelete(m, 999, { bytes: 1, files: 1 })).toBe(m);
  });

  it('nhận ra hiberfil.sys ở gốc ổ, không nhận ở chỗ khác', () => {
    expect(isHiberfil(node(6, 'hiberfil.sys', 1, { path: 'C:\\hiberfil.sys' }))).toBe(true);
    expect(isHiberfil(node(7, 'hiberfil.sys', 1, { path: 'C:\\old\\hiberfil.sys' }))).toBe(false);
  });
});
