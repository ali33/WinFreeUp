import type { ChildrenPage, NodeView, Removed, RestSummary } from '../api/types';

export type SortKey = 'size' | 'name' | 'modified';

/** Trạng thái cây phía giao diện: chỉ giữ những tầng người dùng đã mở (lõi giữ cả cây). */
export interface TreeModel {
  root: NodeView | null;
  pages: Record<number, ChildrenPage>;
  expanded: Record<number, boolean>;
  parentOf: Record<number, number>;
}

export const emptyTree: TreeModel = { root: null, pages: {}, expanded: {}, parentOf: {} };

export function startTree(root: NodeView): TreeModel {
  return { root, pages: {}, expanded: { [root.id]: true }, parentOf: {} };
}

/** Lưu một tầng vừa tải và mở nó. */
export function withPage(m: TreeModel, page: ChildrenPage): TreeModel {
  const parentOf = { ...m.parentOf };
  for (const it of page.items) parentOf[it.id] = page.parent.id;
  return { ...m, pages: { ...m.pages, [page.parent.id]: page }, expanded: { ...m.expanded, [page.parent.id]: true }, parentOf };
}

export function collapse(m: TreeModel, id: number): TreeModel {
  return { ...m, expanded: { ...m.expanded, [id]: false } };
}

/** Đã có tầng con trong bộ nhớ thì mở lại không cần gọi lõi. */
export function reopen(m: TreeModel, id: number): TreeModel | null {
  return m.pages[id] ? { ...m, expanded: { ...m.expanded, [id]: true } } : null;
}

export type Row =
  | { kind: 'node'; node: NodeView; depth: number; parentBytes: number; expanded: boolean }
  | { kind: 'rest'; parentId: number; rest: RestSummary; depth: number };

function sorted(items: NodeView[], key: SortKey): NodeView[] {
  if (key === 'size') return items;
  const copy = [...items];
  if (key === 'name') copy.sort((a, b) => a.name.localeCompare(b.name, 'vi'));
  else copy.sort((a, b) => b.modified - a.modified);
  return copy;
}

/** Trải cây đang mở thành các dòng bảng, gồm dòng tổng «(+N mục nhỏ khác)» cuối mỗi tầng. */
export function visibleRows(m: TreeModel, key: SortKey = 'size'): Row[] {
  const out: Row[] = [];
  if (!m.root) return out;
  const walk = (parent: NodeView, depth: number) => {
    const page = m.pages[parent.id];
    if (!page || !m.expanded[parent.id]) return;
    for (const n of sorted(page.items, key)) {
      const expanded = !!m.expanded[n.id] && !!m.pages[n.id];
      out.push({ kind: 'node', node: n, depth, parentBytes: page.parent.bytes, expanded });
      if (expanded) walk(n, depth + 1);
    }
    if (page.rest) out.push({ kind: 'rest', parentId: parent.id, rest: page.rest, depth });
  };
  walk(m.root, 0);
  return out;
}

/** % so với cha, 0–100, không bao giờ NaN. */
export function share(bytes: number, parentBytes: number): number {
  if (!(parentBytes > 0)) return 0;
  return Math.min(100, Math.max(0, (bytes / parentBytes) * 100));
}

function minus(n: NodeView, r: Removed): NodeView {
  return { ...n, bytes: Math.max(0, n.bytes - r.bytes), files: Math.max(0, n.files - r.files) };
}

/**
 * Sau khi xóa vào Thùng rác: bỏ dòng đó khỏi tầng cha và trừ dung lượng/số file khỏi MỌI tổ tiên
 * đang hiện trên màn (kể cả gốc) — không quét lại (spec 3.3).
 */
export function applyDelete(m: TreeModel, id: number, removed: Removed): TreeModel {
  const parentId = m.parentOf[id];
  if (parentId === undefined) return m;
  const ancestors = new Set<number>();
  for (let cur: number | undefined = parentId; cur !== undefined; cur = m.parentOf[cur]) ancestors.add(cur);
  // Mục bị xóa và mọi con cháu đã tải: dọn trang, quan hệ cha và cờ mở, không để rác trong bộ nhớ.
  const gone = new Set<number>([id]);
  for (const k of Object.keys(m.parentOf)) {
    const start = Number(k);
    for (let cur: number | undefined = start; cur !== undefined; cur = m.parentOf[cur]) {
      if (cur === id) {
        gone.add(start);
        break;
      }
    }
  }
  const pages: Record<number, ChildrenPage> = {};
  for (const [k, page] of Object.entries(m.pages)) {
    const pid = Number(k);
    if (gone.has(pid)) continue;
    let items = page.items.map((it) => (ancestors.has(it.id) ? minus(it, removed) : it));
    if (pid === parentId) items = items.filter((it) => it.id !== id);
    pages[pid] = { ...page, parent: ancestors.has(pid) ? minus(page.parent, removed) : page.parent, items };
  }
  const root = m.root && ancestors.has(m.root.id) ? minus(m.root, removed) : m.root;
  const parentOf = { ...m.parentOf };
  const expanded = { ...m.expanded };
  for (const g of gone) {
    delete parentOf[g];
    delete expanded[g];
  }
  return { ...m, root, pages, parentOf, expanded };
}

/** hiberfil.sys ngay dưới gốc một ổ ⇒ hiện gợi ý tắt ngủ đông. */
export function isHiberfil(n: NodeView): boolean {
  return /^[a-z]:\\hiberfil\.sys$/i.test(n.path);
}
