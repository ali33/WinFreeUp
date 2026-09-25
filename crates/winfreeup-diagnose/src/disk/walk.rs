//! WalkScanner: duyệt song song mọi ổ (FAT32, exFAT, USB, mạng) — đường chậm nhưng chạy ở đâu cũng được.
//! Mỗi thư mục đọc bằng `GetFileInformationByHandleEx(FileIdBothDirectoryInfo)`: một lời gọi trả cả
//! dung lượng thực chiếm (AllocationSize), mã file (FileId — để hard link chỉ đếm một lần) và reparse tag.
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_NO_MORE_FILES, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FileAttributeTagInfo, FileIdBothDirectoryInfo, GetFileInformationByHandle,
    GetFileInformationByHandleEx, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_BOTH_DIR_INFO,
    FILE_LIST_DIRECTORY, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use winfreeup_core::{CancelToken, CoreError, Result};

use super::scan::{DiskScanner, ScanProgress, ScanStatus, Throttle};
use super::tree::{DiskTree, NodeId, TreeBuilder, FLAG_DIR, FLAG_LINK, FLAG_UNREADABLE, NO_PARENT};
use crate::util::{filetime_to_unix, wide};

/// Bit «name surrogate» của reparse tag: junction, symlink… trỏ sang chỗ khác ⇒ không đi theo.
/// Thư mục OneDrive (tag cloud) không có bit này ⇒ vẫn là thư mục thật, vẫn duyệt vào.
const NAME_SURROGATE: u32 = 0x2000_0000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    /// Junction/symlink (reparse point có bit name surrogate).
    pub is_link: bool,
    pub alloc: u64,
    pub modified: u32,
    pub file_id: u64,
}

pub fn is_link_tag(attributes: u32, reparse_tag: u32) -> bool {
    attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 && reparse_tag & NAME_SURROGATE != 0
}

/// `C:\a` ⇒ `\\?\C:\a` để vượt giới hạn 260 ký tự.
fn long_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with(r"\\?\") {
        path.to_path_buf()
    } else if let Some(unc) = s.strip_prefix(r"\\") {
        PathBuf::from(format!(r"\\?\UNC\{unc}"))
    } else {
        PathBuf::from(format!(r"\\?\{s}"))
    }
}

fn open_dir(w: &[u16], flags: u32) -> std::io::Result<HANDLE> {
    // SAFETY: w là chuỗi kết thúc NUL; không có security attributes/template; người gọi đóng handle.
    let h = unsafe {
        CreateFileW(
            w.as_ptr(),
            FILE_LIST_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | flags,
            std::ptr::null_mut(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(h)
    }
}

/// (ổ, mã file) của handle — để biết hai handle có cùng trỏ một thư mục không.
fn identity(h: HANDLE) -> Option<(u32, u64)> {
    // SAFETY: BY_HANDLE_FILE_INFORMATION là POD, toàn số 0 hợp lệ.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: h là handle hợp lệ; info khả ghi.
    let ok = unsafe { GetFileInformationByHandle(h, &mut info) };
    (ok != 0).then(|| (info.dwVolumeSerialNumber, (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow)))
}

/// Mở thư mục để liệt kê mà KHÔNG đi theo junction/symlink, kể cả khi nó bị tráo giữa lúc liệt kê cha và lúc
/// mở con. `Ok(None)` ⇒ bản thân `dir` là link. Reparse point không có bit name surrogate (thư mục OneDrive…)
/// được mở lại bình thường để bộ lọc cloud liệt kê đủ, rồi đối chiếu mã file với handle đầu cho chắc cùng một chỗ.
fn open_dir_no_follow(dir: &Path) -> std::io::Result<Option<HANDLE>> {
    let w = wide(long_path(dir));
    let h = open_dir(&w, FILE_FLAG_OPEN_REPARSE_POINT)?;
    let mut tag = FILE_ATTRIBUTE_TAG_INFO { FileAttributes: 0, ReparseTag: 0 };
    // SAFETY: h là handle hợp lệ vừa mở; tag khả ghi đúng kích thước truyền vào.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            h,
            FileAttributeTagInfo,
            (&mut tag as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    if ok == 0 {
        let err = std::io::Error::last_os_error();
        // SAFETY: h mở ở trên, đóng đúng một lần.
        unsafe { CloseHandle(h) };
        return Err(err);
    }
    if tag.FileAttributes & FILE_ATTRIBUTE_REPARSE_POINT == 0 {
        return Ok(Some(h));
    }
    if is_link_tag(tag.FileAttributes, tag.ReparseTag) {
        // SAFETY: như trên.
        unsafe { CloseHandle(h) };
        return Ok(None);
    }
    let again = open_dir(&w, 0);
    let same = again.as_ref().ok().and_then(|&h2| identity(h2)).is_some_and(|id| identity(h) == Some(id));
    // SAFETY: như trên.
    unsafe { CloseHandle(h) };
    match again {
        Ok(h2) if same => Ok(Some(h2)),
        Ok(h2) => {
            // SAFETY: h2 vừa mở, đóng đúng một lần.
            unsafe { CloseHandle(h2) };
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Đọc toàn bộ mục trong một thư mục (bỏ `.` và `..`). Bản thân `dir` là junction/symlink ⇒ lỗi, không đi theo.
pub fn read_dir_entries(dir: &Path) -> std::io::Result<Vec<DirEntry>> {
    read_dir(dir)?.ok_or_else(|| std::io::Error::other(format!("reparse link, not followed: {}", dir.display())))
}

/// `Ok(None)` ⇒ `dir` là link (không đọc).
fn read_dir(dir: &Path) -> std::io::Result<Option<Vec<DirEntry>>> {
    let Some(h) = open_dir_no_follow(dir)? else { return Ok(None) };
    let mut out = Vec::new();
    // u64 để bộ đệm căn 8 byte như cấu trúc yêu cầu.
    let mut buf = vec![0u64; 64 * 1024 / 8];
    let result = loop {
        let ok = unsafe {
            GetFileInformationByHandleEx(h, FileIdBothDirectoryInfo, buf.as_mut_ptr().cast(), (buf.len() * 8) as u32)
        };
        if ok == 0 {
            let err = unsafe { GetLastError() };
            break if err == ERROR_NO_MORE_FILES { Ok(()) } else { Err(std::io::Error::from_raw_os_error(err as i32)) };
        }
        let base = buf.as_ptr() as *const u8;
        let mut off = 0usize;
        loop {
            // SAFETY: hệ điều hành ghi các bản ghi FILE_ID_BOTH_DIR_INFO nối nhau, căn 8 byte, trong `buf`.
            let info = unsafe { &*(base.add(off) as *const FILE_ID_BOTH_DIR_INFO) };
            let name_len = info.FileNameLength as usize / 2;
            let name_ptr = unsafe { base.add(off + std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName)) } as *const u16;
            let name = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(name_ptr, name_len) });
            if name != "." && name != ".." {
                let attrs = info.FileAttributes;
                out.push(DirEntry {
                    name,
                    is_dir: attrs & FILE_ATTRIBUTE_DIRECTORY != 0,
                    // Khi có FILE_ATTRIBUTE_REPARSE_POINT, trường EaSize chứa reparse tag.
                    is_link: is_link_tag(attrs, info.EaSize),
                    alloc: info.AllocationSize.max(0) as u64,
                    modified: filetime_to_unix(info.LastWriteTime),
                    file_id: info.FileId as u64,
                });
            }
            if info.NextEntryOffset == 0 {
                break;
            }
            off += info.NextEntryOffset as usize;
        }
    };
    // SAFETY: h do open_dir_no_follow trả, đóng đúng một lần.
    unsafe { CloseHandle(h) };
    result.map(|_| Some(out))
}

/// Mã file đã gặp, chia 16 ngăn để các luồng ít tranh khóa.
struct SeenIds([Mutex<HashSet<u64>>; 16]);

impl SeenIds {
    fn new() -> Self {
        SeenIds(std::array::from_fn(|_| Mutex::new(HashSet::new())))
    }
    /// true nếu đây là lần đầu gặp mã này. Mã 0 (hệ tệp không có mã file) luôn coi là lần đầu.
    fn first(&self, id: u64) -> bool {
        if id == 0 {
            return true;
        }
        self.0[(id % 16) as usize].lock().map(|mut s| s.insert(id)).unwrap_or(true)
    }
}

struct Shared<'a> {
    builder: Mutex<TreeBuilder>,
    queue: Mutex<Vec<(NodeId, PathBuf)>>,
    wake: Condvar,
    /// Số thư mục đang chờ hoặc đang đọc; về 0 là xong.
    pending: AtomicUsize,
    seen: SeenIds,
    files: AtomicU64,
    bytes: AtomicU64,
    throttle: Throttle,
    cancel: &'a CancelToken,
    progress: &'a dyn ScanProgress,
}

impl Shared<'_> {
    fn next_job(&self) -> Option<(NodeId, PathBuf)> {
        let mut q = self.queue.lock().ok()?;
        loop {
            if self.cancel.is_cancelled() {
                return None;
            }
            if let Some(job) = q.pop() {
                return Some(job);
            }
            if self.pending.load(Ordering::SeqCst) == 0 {
                return None;
            }
            q = self.wake.wait_timeout(q, Duration::from_millis(50)).ok()?.0;
        }
    }

    fn visit(&self, node: NodeId, dir: &Path) {
        match read_dir(dir) {
            Err(_) => {
                if let Ok(mut b) = self.builder.lock() {
                    b.add_flags(node, FLAG_UNREADABLE);
                }
            }
            // Bị tráo thành junction/symlink sau khi liệt kê cha: giữ nút, gắn cờ link, không đi vào.
            Ok(None) => {
                if let Ok(mut b) = self.builder.lock() {
                    b.add_flags(node, FLAG_LINK);
                }
            }
            Ok(Some(entries)) => {
                let mut subdirs = Vec::new();
                if let Ok(mut b) = self.builder.lock() {
                    for e in entries {
                        if e.is_link {
                            let flags = FLAG_LINK | if e.is_dir { FLAG_DIR } else { 0 };
                            b.add(node, &e.name, flags, 0, e.modified);
                        } else if e.is_dir {
                            let id = b.add(node, &e.name, FLAG_DIR, 0, e.modified);
                            subdirs.push((id, dir.join(&e.name)));
                        } else if self.seen.first(e.file_id) {
                            b.add(node, &e.name, 0, e.alloc, e.modified);
                            self.files.fetch_add(1, Ordering::Relaxed);
                            self.bytes.fetch_add(e.alloc, Ordering::Relaxed);
                        } else {
                            // Tên thứ hai của cùng một file (hard link): hiện ra nhưng không đếm lại.
                            b.add(node, &e.name, FLAG_LINK, 0, e.modified);
                        }
                    }
                }
                if !subdirs.is_empty() {
                    self.pending.fetch_add(subdirs.len(), Ordering::SeqCst);
                    if let Ok(mut q) = self.queue.lock() {
                        q.extend(subdirs);
                    }
                    self.wake.notify_all();
                }
            }
        }
        if self.throttle.ready() {
            self.progress.report(&ScanStatus {
                files: self.files.load(Ordering::Relaxed),
                bytes: self.bytes.load(Ordering::Relaxed),
                current: dir.display().to_string(),
                percent: None,
            });
        }
    }
}

pub struct WalkScanner {
    pub threads: usize,
}

impl Default for WalkScanner {
    /// Thời gian chủ yếu là chờ mở thư mục, không phải CPU: đo trên NVMe (16 luồng CPU), ổ C: 3,8 triệu mục
    /// mất 437 giây với 8 luồng, 85 giây với 32 luồng ⇒ gấp đôi số luồng CPU, trong khoảng 8–32.
    fn default() -> Self {
        let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        WalkScanner { threads: (n * 2).clamp(8, 32) }
    }
}

impl DiskScanner for WalkScanner {
    fn scan(&self, root: &Path, cancel: &CancelToken, progress: &dyn ScanProgress) -> Result<DiskTree> {
        let meta = std::fs::metadata(root).map_err(|e| winfreeup_core::error::io_err(root, e))?;
        if !meta.is_dir() {
            return Err(CoreError::System(format!("not a directory: {}", root.display())));
        }
        let mut builder = TreeBuilder::with_capacity(1 << 16);
        let root_id = builder.add(NO_PARENT, &root.display().to_string(), FLAG_DIR, 0, 0);
        let shared = Shared {
            builder: Mutex::new(builder),
            queue: Mutex::new(vec![(root_id, root.to_path_buf())]),
            wake: Condvar::new(),
            pending: AtomicUsize::new(1),
            seen: SeenIds::new(),
            files: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            throttle: Throttle::new(Duration::from_millis(100)),
            cancel,
            progress,
        };
        std::thread::scope(|s| {
            for _ in 0..self.threads.max(1) {
                s.spawn(|| {
                    while let Some((node, dir)) = shared.next_job() {
                        shared.visit(node, &dir);
                        if shared.pending.fetch_sub(1, Ordering::SeqCst) == 1 {
                            shared.wake.notify_all();
                        }
                    }
                });
            }
        });
        if cancel.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        progress.report(&ScanStatus {
            files: shared.files.load(Ordering::Relaxed),
            bytes: shared.bytes.load(Ordering::Relaxed),
            current: root.display().to_string(),
            percent: None,
        });
        let builder = shared.builder.into_inner().map_err(|e| CoreError::System(e.to_string()))?;
        Ok(builder.finish(root_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::tree::MAX_CHILDREN;
    use std::fs;
    use std::os::windows::io::AsRawHandle;
    use std::process::Command;

    fn write(path: &Path, len: usize) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![b'x'; len]).unwrap();
    }

    fn scan(root: &Path) -> DiskTree {
        WalkScanner { threads: 4 }.scan(root, &CancelToken::new(), &|_: &ScanStatus| {}).unwrap()
    }

    #[test]
    fn allocated_size_is_counted_not_logical_size() {
        let t = tempfile::tempdir().unwrap();
        write(&t.path().join("a").join("one.bin"), 1);
        write(&t.path().join("a").join("big.bin"), 100_000);
        let tree = scan(t.path());
        let a = tree.child_named(tree.root(), "a").unwrap();
        let one = tree.child_named(a, "one.bin").unwrap();
        // 1 byte thường nằm ngay trong MFT (0 byte cấp phát) hoặc chiếm trọn một cụm — không bao giờ đúng 1.
        assert_ne!(tree.bytes(one), 1);
        let big = tree.child_named(a, "big.bin").unwrap();
        assert!(tree.bytes(big) >= 100_000 && tree.bytes(big).is_multiple_of(512), "{}", tree.bytes(big));
        assert_eq!(tree.files(tree.root()), 2);
    }

    #[test]
    fn hard_link_is_counted_once() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("x").join("data.bin");
        write(&f, 200_000);
        fs::create_dir_all(t.path().join("y")).unwrap();
        fs::hard_link(&f, t.path().join("y").join("same.bin")).unwrap();
        let tree = scan(t.path());
        let one = tree.bytes(tree.child_named(tree.root(), "x").unwrap()) + tree.bytes(tree.child_named(tree.root(), "y").unwrap());
        assert!((200_000..400_000).contains(&one), "{one}");
        assert_eq!(tree.files(tree.root()), 1);
    }

    #[test]
    fn junction_pointing_outside_is_not_followed() {
        let t = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(&outside.path().join("huge.bin"), 500_000);
        fs::create_dir_all(t.path().join("in")).unwrap();
        junction::create(outside.path(), t.path().join("in").join("link")).unwrap();
        let tree = scan(t.path());
        assert_eq!(tree.bytes(tree.root()), 0);
        let inn = tree.child_named(tree.root(), "in").unwrap();
        let link = tree.children(inn, MAX_CHILDREN).unwrap().items[0].clone();
        assert!(link.is_link && !link.has_children);
    }

    #[test]
    fn unreadable_directory_stays_a_node_and_is_flagged() {
        let t = tempfile::tempdir().unwrap();
        let locked = t.path().join("khoa");
        write(&locked.join("secret.bin"), 50_000);
        let deny = Command::new("icacls").arg(&locked).args(["/deny", "*S-1-1-0:(RD)"]).output().unwrap();
        assert!(deny.status.success(), "{}", String::from_utf8_lossy(&deny.stdout));
        let tree = scan(t.path());
        let _ = Command::new("icacls").arg(&locked).args(["/remove:d", "*S-1-1-0"]).output();
        let node = tree.view(tree.child_named(tree.root(), "khoa").unwrap());
        assert!(node.unreadable);
        assert_eq!(node.bytes, 0);
    }

    #[test]
    fn sparse_file_counts_only_allocated_clusters() {
        use windows_sys::Win32::System::Ioctl::FSCTL_SET_SPARSE;
        use windows_sys::Win32::System::IO::DeviceIoControl;
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("sparse.bin");
        {
            let f = fs::File::create(&path).unwrap();
            let mut ret = 0u32;
            let ok = unsafe {
                DeviceIoControl(f.as_raw_handle(), FSCTL_SET_SPARSE, std::ptr::null(), 0, std::ptr::null_mut(), 0, &mut ret, std::ptr::null_mut())
            };
            assert_ne!(ok, 0, "{}", std::io::Error::last_os_error());
            f.set_len(64 * 1024 * 1024).unwrap();
        }
        let tree = scan(t.path());
        assert_eq!(fs::metadata(&path).unwrap().len(), 64 * 1024 * 1024);
        assert!(tree.bytes(tree.root()) < 1024 * 1024, "{}", tree.bytes(tree.root()));
    }

    #[test]
    fn long_and_vietnamese_names_are_read() {
        let t = tempfile::tempdir().unwrap();
        let mut deep = t.path().to_path_buf();
        for i in 0..12 {
            deep.push(format!("thư-mục-khá-dài-số-{i:02}-để-vượt-260-ký-tự"));
        }
        let long = long_path(&deep.join("tệp.bin"));
        fs::create_dir_all(long_path(&deep)).unwrap();
        fs::write(&long, vec![0u8; 10_000]).unwrap();
        assert!(deep.to_string_lossy().len() > 260);
        let tree = scan(t.path());
        assert_eq!(tree.files(tree.root()), 1);
    }

    #[test]
    fn cancel_stops_the_scan() {
        let t = tempfile::tempdir().unwrap();
        write(&t.path().join("a.bin"), 10);
        let c = CancelToken::new();
        c.cancel();
        let r = WalkScanner { threads: 2 }.scan(t.path(), &c, &|_: &ScanStatus| {});
        assert!(matches!(r, Err(CoreError::Cancelled)));
    }

    #[test]
    fn progress_is_reported_at_least_once_with_totals() {
        let t = tempfile::tempdir().unwrap();
        write(&t.path().join("a.bin"), 10_000);
        let seen = Mutex::new(Vec::new());
        WalkScanner { threads: 2 }.scan(t.path(), &CancelToken::new(), &|s: &ScanStatus| seen.lock().unwrap().push(s.clone())).unwrap();
        let last = seen.into_inner().unwrap().pop().unwrap();
        assert_eq!(last.files, 1);
        assert!(last.bytes >= 10_000);
    }

    #[test]
    fn file_id_zero_is_never_treated_as_a_hard_link() {
        // FAT32/exFAT/ổ mạng có thể trả FileId = 0 cho mọi file: gộp theo mã thì cả ổ chỉ còn một file.
        let seen = SeenIds::new();
        assert!(seen.first(0) && seen.first(0));
        assert!(seen.first(42));
        assert!(!seen.first(42));
    }

    #[test]
    fn read_dir_entries_does_not_follow_a_junction() {
        let t = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(&outside.path().join("huge.bin"), 10);
        let link = t.path().join("link");
        junction::create(outside.path(), &link).unwrap();
        assert!(read_dir_entries(&link).is_err());
        assert_eq!(read_dir_entries(outside.path()).unwrap().len(), 1);
    }

    #[test]
    fn link_tags_are_recognised() {
        assert!(is_link_tag(FILE_ATTRIBUTE_REPARSE_POINT, 0xA000_0003), "junction");
        assert!(is_link_tag(FILE_ATTRIBUTE_REPARSE_POINT, 0xA000_000C), "symlink");
        assert!(!is_link_tag(FILE_ATTRIBUTE_REPARSE_POINT, 0x9000_601A), "thư mục OneDrive");
        assert!(!is_link_tag(0, 0xA000_0003));
    }
}
