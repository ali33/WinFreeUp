//! Cây thư mục gọn trong bộ nhớ: mảng nút + bảng tên chung, con xếp kiểu CSR sau khi `finish`.
use serde::Serialize;

pub type NodeId = u32;
pub const NO_PARENT: NodeId = u32::MAX;
/// Spec 3.1: giao diện chỉ nhận tối đa 200 con lớn nhất mỗi lần mở.
pub const MAX_CHILDREN: usize = 200;

pub const FLAG_DIR: u16 = 1;
pub const FLAG_UNREADABLE: u16 = 2;
pub const FLAG_LINK: u16 = 4;
pub const FLAG_DELETED: u16 = 8;

#[derive(Debug, Clone, Copy)]
struct Node {
    name_off: u32,
    name_len: u16,
    flags: u16,
    parent: NodeId,
    /// Số file trong cây con (file thường tính 1 chính nó).
    files: u32,
    /// Giây Unix, lớn nhất trong cây con.
    modified: u32,
    /// Byte thực chiếm trên đĩa của cả cây con.
    bytes: u64,
}

/// Dựng cây trong lúc quét. Nút cha có thể thêm sau nút con (MFT) — `finish` tự sắp lại.
#[derive(Default)]
pub struct TreeBuilder {
    nodes: Vec<Node>,
    names: String,
}

impl TreeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(nodes: usize) -> Self {
        TreeBuilder { nodes: Vec::with_capacity(nodes), names: String::with_capacity(nodes.saturating_mul(16)) }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Thêm một nút; `bytes`, `modified` là của riêng nút (file) — thư mục truyền 0.
    /// Nút mang `FLAG_LINK` (junction, tên thứ hai của hard link) không được đếm là file.
    pub fn add(&mut self, parent: NodeId, name: &str, flags: u16, bytes: u64, modified: u32) -> NodeId {
        // NO_PARENT (u32::MAX) là giá trị canh, nên id hợp lệ phải nhỏ hơn nó.
        debug_assert!(self.nodes.len() < NO_PARENT as usize, "quá nhiều nút cho NodeId u32");
        debug_assert!(self.names.len() <= u32::MAX as usize, "bảng tên vượt u32");
        let id = self.nodes.len() as NodeId;
        let name = truncate_name(name);
        let name_off = self.names.len() as u32;
        self.names.push_str(name);
        let is_file = flags & FLAG_DIR == 0;
        self.nodes.push(Node {
            name_off,
            name_len: name.len() as u16,
            flags,
            parent,
            files: u32::from(is_file && flags & FLAG_LINK == 0),
            modified,
            bytes,
        });
        id
    }

    pub fn set_parent(&mut self, id: NodeId, parent: NodeId) {
        self.nodes[id as usize].parent = parent;
    }

    pub fn add_flags(&mut self, id: NodeId, flags: u16) {
        self.nodes[id as usize].flags |= flags;
    }

    /// Cộng dồn byte/số file/ngày sửa lên mọi tổ tiên và xếp con. `root` là gốc (ổ đĩa).
    /// Nút không nối được về gốc (mồ côi, vòng cha–con do MFT hỏng) bị ẩn: gắn `FLAG_DELETED`
    /// và cắt cha, để `path`/`remove` không bao giờ lặp vô tận.
    pub fn finish(self, root: NodeId) -> DiskTree {
        let TreeBuilder { mut nodes, names } = self;
        let n = nodes.len();
        debug_assert!(n < NO_PARENT as usize, "quá nhiều nút cho NodeId u32");
        // CSR: đếm con của từng nút.
        let mut start = vec![0u32; n + 1];
        for (i, node) in nodes.iter().enumerate() {
            if i as NodeId != root && (node.parent as usize) < n {
                start[node.parent as usize + 1] += 1;
            }
        }
        for i in 0..n {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut list = vec![0u32; start[n] as usize];
        for (i, node) in nodes.iter().enumerate() {
            if i as NodeId != root && (node.parent as usize) < n {
                let p = node.parent as usize;
                list[fill[p] as usize] = i as NodeId;
                fill[p] += 1;
            }
        }
        // Hậu thứ tự lặp từ gốc: con xong mới tới cha. Mỗi nút có đúng một cha nên không thăm lại.
        let mut order = Vec::with_capacity(n);
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            order.push(id);
            let (a, b) = (start[id as usize] as usize, start[id as usize + 1] as usize);
            stack.extend_from_slice(&list[a..b]);
        }
        let mut reachable = vec![false; n];
        for &id in &order {
            reachable[id as usize] = true;
        }
        for (node, _) in nodes.iter_mut().zip(&reachable).filter(|(_, r)| !**r) {
            node.flags |= FLAG_DELETED;
            node.parent = NO_PARENT;
        }
        for &id in order.iter().rev() {
            let node = nodes[id as usize];
            if node.parent != NO_PARENT && id != root {
                let p = &mut nodes[node.parent as usize];
                p.bytes = p.bytes.saturating_add(node.bytes);
                p.files = p.files.saturating_add(node.files);
                p.modified = p.modified.max(node.modified);
            }
        }
        // Mỗi danh sách con xếp sẵn theo byte giảm dần, cùng byte thì theo tên.
        for id in 0..n {
            let (a, b) = (start[id] as usize, start[id + 1] as usize);
            list[a..b].sort_by(|&x, &y| {
                let (nx, ny) = (&nodes[x as usize], &nodes[y as usize]);
                ny.bytes.cmp(&nx.bytes).then_with(|| name_of(&names, nx).cmp(name_of(&names, ny)))
            });
        }
        DiskTree { nodes, names, child_start: start, child_list: list, root }
    }
}

fn truncate_name(name: &str) -> &str {
    if name.len() <= u16::MAX as usize {
        return name;
    }
    let mut end = u16::MAX as usize;
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}

fn name_of<'a>(names: &'a str, n: &Node) -> &'a str {
    &names[n.name_off as usize..n.name_off as usize + n.name_len as usize]
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NodeView {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub files: u64,
    pub modified: u32,
    pub is_dir: bool,
    pub is_link: bool,
    pub unreadable: bool,
    pub has_children: bool,
    /// Điền ở tầng dịch vụ (cần luật bảo vệ); cây để `false`.
    pub protected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RestSummary {
    pub count: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChildrenPage {
    pub parent: NodeView,
    pub items: Vec<NodeView>,
    /// "(+N mục nhỏ khác, X GB)" khi có hơn 200 con.
    pub rest: Option<RestSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Removed {
    pub bytes: u64,
    pub files: u64,
}

pub struct DiskTree {
    nodes: Vec<Node>,
    names: String,
    child_start: Vec<u32>,
    child_list: Vec<NodeId>,
    root: NodeId,
}

impl DiskTree {
    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Nút còn trong cây: có thật, chưa bị xóa, và không nằm trong một thư mục đã xóa.
    pub fn contains(&self, id: NodeId) -> bool {
        if id as usize >= self.nodes.len() {
            return false;
        }
        let mut cur = id;
        loop {
            let n = &self.nodes[cur as usize];
            if n.flags & FLAG_DELETED != 0 {
                return false;
            }
            if cur == self.root || n.parent == NO_PARENT {
                return true;
            }
            cur = n.parent;
        }
    }

    /// `id` phải hợp lệ (`< len()`), nếu không sẽ panic.
    pub fn name(&self, id: NodeId) -> &str {
        name_of(&self.names, &self.nodes[id as usize])
    }

    /// `id` phải hợp lệ (`< len()`), nếu không sẽ panic.
    pub fn bytes(&self, id: NodeId) -> u64 {
        self.nodes[id as usize].bytes
    }

    /// `id` phải hợp lệ (`< len()`), nếu không sẽ panic.
    pub fn files(&self, id: NodeId) -> u64 {
        u64::from(self.nodes[id as usize].files)
    }

    /// `id` phải hợp lệ (`< len()`), nếu không sẽ panic.
    pub fn is_dir(&self, id: NodeId) -> bool {
        self.nodes[id as usize].flags & FLAG_DIR != 0
    }

    /// `id` phải hợp lệ (`< len()`), nếu không sẽ panic.
    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        let p = self.nodes[id as usize].parent;
        (id != self.root && p != NO_PARENT).then_some(p)
    }

    /// Đường dẫn đầy đủ: tên gốc (vd `C:\`) nối các tên con bằng `\`.
    /// Nút không còn trong cây (`contains` = false: đã xóa, mồ côi, nằm trong vòng) ⇒ chuỗi rỗng,
    /// không bao giờ trả một đường giả.
    pub fn path(&self, id: NodeId) -> String {
        if !self.contains(id) {
            return String::new();
        }
        let mut parts = Vec::new();
        let mut cur = id;
        while cur != NO_PARENT && cur != self.root {
            parts.push(self.name(cur));
            cur = self.nodes[cur as usize].parent;
        }
        let mut out = self.name(self.root).to_string();
        for p in parts.iter().rev() {
            if !out.ends_with('\\') {
                out.push('\\');
            }
            out.push_str(p);
        }
        out
    }

    fn live_children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        let (a, b) = (self.child_start[id as usize] as usize, self.child_start[id as usize + 1] as usize);
        self.child_list[a..b].iter().copied().filter(|&c| self.nodes[c as usize].flags & FLAG_DELETED == 0)
    }

    /// `id` phải hợp lệ (`< len()`), nếu không sẽ panic. Nút không còn trong cây có `path` rỗng.
    pub fn view(&self, id: NodeId) -> NodeView {
        let n = &self.nodes[id as usize];
        NodeView {
            id,
            name: self.name(id).to_string(),
            path: self.path(id),
            bytes: n.bytes,
            files: u64::from(n.files),
            modified: n.modified,
            is_dir: n.flags & FLAG_DIR != 0,
            is_link: n.flags & FLAG_LINK != 0,
            unreadable: n.flags & FLAG_UNREADABLE != 0,
            has_children: self.live_children(id).next().is_some(),
            protected: false,
        }
    }

    /// Tối đa `limit` con lớn nhất (đã xếp sẵn), phần còn lại gộp thành một dòng tổng.
    pub fn children(&self, id: NodeId, limit: usize) -> Option<ChildrenPage> {
        if !self.contains(id) {
            return None;
        }
        let mut items = Vec::new();
        let mut rest = RestSummary { count: 0, bytes: 0 };
        for c in self.live_children(id) {
            if items.len() < limit {
                items.push(self.view(c));
            } else {
                rest.count += 1;
                rest.bytes += self.nodes[c as usize].bytes;
            }
        }
        Some(ChildrenPage { parent: self.view(id), items, rest: (rest.count > 0).then_some(rest) })
    }

    /// Sau khi xóa vào Thùng rác: đánh dấu nút đã xóa và trừ byte/số file khỏi mọi tổ tiên (không quét lại).
    /// `modified` của tổ tiên KHÔNG được tính lại (giữ ngày sửa lớn nhất lúc quét).
    /// Cần đường dẫn của nút thì lấy `path` TRƯỚC khi gọi — sau đó `path` trả chuỗi rỗng.
    pub fn remove(&mut self, id: NodeId) -> Option<Removed> {
        if !self.contains(id) || id == self.root {
            return None;
        }
        let (bytes, files) = (self.nodes[id as usize].bytes, self.nodes[id as usize].files);
        self.nodes[id as usize].flags |= FLAG_DELETED;
        let mut cur = self.nodes[id as usize].parent;
        while cur != NO_PARENT {
            let p = &mut self.nodes[cur as usize];
            p.bytes = p.bytes.saturating_sub(bytes);
            p.files = p.files.saturating_sub(files);
            cur = if cur == self.root { NO_PARENT } else { p.parent };
        }
        Some(Removed { bytes, files: u64::from(files) })
    }

    /// Tìm con theo tên (không phân biệt hoa thường) — dùng trong test và khi đối chiếu hai bộ quét.
    pub fn child_named(&self, id: NodeId, name: &str) -> Option<NodeId> {
        self.live_children(id).find(|&c| self.name(c).eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C:\ ─ big (dir) ─ a.bin 300, b.bin 100 ; small.txt 5
    fn sample() -> (DiskTree, NodeId, NodeId, NodeId) {
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "C:\\", FLAG_DIR, 0, 0);
        let big = b.add(root, "big", FLAG_DIR, 0, 0);
        let a = b.add(big, "a.bin", 0, 300, 1_700_000_000);
        b.add(big, "b.bin", 0, 100, 1_600_000_000);
        b.add(root, "small.txt", 0, 5, 1_500_000_000);
        (b.finish(root), root, big, a)
    }

    #[test]
    fn totals_roll_up_to_every_ancestor() {
        let (t, root, big, _) = sample();
        assert_eq!(t.bytes(root), 405);
        assert_eq!(t.files(root), 3);
        assert_eq!(t.bytes(big), 400);
        assert_eq!(t.view(big).modified, 1_700_000_000);
    }

    #[test]
    fn children_are_sorted_largest_first_with_paths() {
        let (t, root, big, _) = sample();
        let page = t.children(root, MAX_CHILDREN).unwrap();
        assert_eq!(page.items.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), vec!["big", "small.txt"]);
        assert_eq!(page.items[0].path, "C:\\big");
        assert!(page.items[0].has_children && page.items[0].is_dir);
        assert!(page.rest.is_none());
        assert_eq!(t.children(big, MAX_CHILDREN).unwrap().items[0].path, "C:\\big\\a.bin");
    }

    #[test]
    fn more_than_the_limit_collapses_into_one_summary_row() {
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "D:\\", FLAG_DIR, 0, 0);
        for i in 1..=205u64 {
            b.add(root, &format!("f{i}"), 0, i, 0);
        }
        let t = b.finish(root);
        let page = t.children(root, MAX_CHILDREN).unwrap();
        assert_eq!(page.items.len(), 200);
        assert_eq!(page.items[0].bytes, 205);
        assert_eq!(page.items[199].bytes, 6);
        assert_eq!(page.rest, Some(RestSummary { count: 5, bytes: 1 + 2 + 3 + 4 + 5 }));
    }

    #[test]
    fn exactly_the_limit_has_no_summary_row() {
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "D:\\", FLAG_DIR, 0, 0);
        for i in 0..200u64 {
            b.add(root, &format!("f{i}"), 0, i, 0);
        }
        let t = b.finish(root);
        let page = t.children(root, MAX_CHILDREN).unwrap();
        assert_eq!(page.items.len(), 200);
        assert!(page.rest.is_none());
    }

    #[test]
    fn remove_subtracts_from_ancestors_and_hides_the_node() {
        let (mut t, root, big, a) = sample();
        assert_eq!(t.remove(a), Some(Removed { bytes: 300, files: 1 }));
        assert_eq!(t.bytes(big), 100);
        assert_eq!(t.bytes(root), 105);
        assert_eq!(t.files(root), 2);
        assert!(!t.contains(a));
        assert_eq!(t.path(a), "");
        assert_eq!(t.children(big, MAX_CHILDREN).unwrap().items.len(), 1);
        assert_eq!(t.remove(a), None, "xóa lần hai không trừ thêm");
        assert_eq!(t.remove(root), None, "không xóa gốc");
        assert!(t.children(a, MAX_CHILDREN).is_none());
    }

    #[test]
    fn parents_added_after_children_still_roll_up() {
        // Thứ tự bản ghi MFT: con có thể đứng trước cha.
        let mut b = TreeBuilder::new();
        let file = b.add(NO_PARENT, "x.dat", 0, 50, 0);
        let dir = b.add(NO_PARENT, "dir", FLAG_DIR, 0, 0);
        let root = b.add(NO_PARENT, "E:\\", FLAG_DIR, 0, 0);
        b.set_parent(file, dir);
        b.set_parent(dir, root);
        let t = b.finish(root);
        assert_eq!(t.bytes(root), 50);
        assert_eq!(t.path(file), "E:\\dir\\x.dat");
    }

    #[test]
    fn links_are_not_counted_as_files() {
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "C:\\", FLAG_DIR, 0, 0);
        b.add(root, "junction", FLAG_DIR | FLAG_LINK, 0, 0);
        b.add(root, "f", 0, 1, 0);
        let t = b.finish(root);
        assert_eq!(t.files(root), 1);
    }

    #[test]
    fn second_name_of_a_hard_link_adds_no_bytes_and_no_file() {
        // Bộ quét thêm tên đầu với dung lượng thật, các tên sau với FLAG_LINK và 0 byte.
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "C:\\", FLAG_DIR, 0, 0);
        let x = b.add(root, "x", FLAG_DIR, 0, 0);
        let y = b.add(root, "y", FLAG_DIR, 0, 0);
        b.add(x, "same.bin", 0, 4096, 0);
        let second = b.add(y, "same.bin", FLAG_LINK, 0, 0);
        let t = b.finish(root);
        assert_eq!((t.bytes(root), t.files(root)), (4096, 1));
        assert_eq!((t.bytes(y), t.files(y)), (0, 0));
        assert!(t.view(second).is_link, "tên thứ hai vẫn hiện ra");
    }

    #[test]
    fn unreadable_folder_is_still_a_listed_node() {
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "C:\\", FLAG_DIR, 0, 0);
        let locked = b.add(root, "locked", FLAG_DIR, 0, 0);
        b.add_flags(locked, FLAG_UNREADABLE);
        let t = b.finish(root);
        let page = t.children(root, MAX_CHILDREN).unwrap();
        assert_eq!(page.items.len(), 1);
        let v = &page.items[0];
        assert!(v.unreadable && v.is_dir && !v.has_children);
        assert_eq!(v.path, "C:\\locked");
        assert_eq!(t.child_named(root, "LOCKED"), Some(locked));
    }

    #[test]
    fn removing_inside_an_already_removed_folder_does_not_subtract_twice() {
        let (mut t, root, big, a) = sample();
        assert_eq!(t.remove(big), Some(Removed { bytes: 400, files: 2 }));
        assert!(!t.contains(a), "con của thư mục đã xóa cũng không còn");
        assert_eq!(t.remove(a), None);
        assert!(t.children(a, MAX_CHILDREN).is_none());
        assert_eq!((t.bytes(root), t.files(root)), (5, 1));
    }

    #[test]
    fn orphans_and_parent_cycles_are_hidden_and_never_hang() {
        let mut b = TreeBuilder::new();
        let root = b.add(NO_PARENT, "F:\\", FLAG_DIR, 0, 0);
        b.add(root, "ok", 0, 7, 0);
        let p = b.add(NO_PARENT, "p", FLAG_DIR, 0, 0);
        let q = b.add(p, "q", FLAG_DIR, 0, 0);
        b.set_parent(p, q); // vòng p ↔ q, không nối về gốc
        let lost = b.add(12345, "lost", 0, 99, 0); // cha không tồn tại
        let mut t = b.finish(root);
        assert_eq!((t.bytes(root), t.files(root)), (7, 1));
        for id in [p, q, lost] {
            assert!(!t.contains(id));
            assert!(t.children(id, MAX_CHILDREN).is_none());
            assert_eq!(t.parent_of(id), None);
            assert_eq!(t.path(id), "", "không trả đường giả cho nút ngoài cây");
        }
        assert_eq!(t.remove(q), None);
    }
}
