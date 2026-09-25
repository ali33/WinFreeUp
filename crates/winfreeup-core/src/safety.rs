//! Luật an toàn (spec mục 6): gốc được phép + canonicalize, không đi theo reparse point,
//! file khóa/không quyền thì bỏ qua và đếm.
use std::fs;
use std::io;
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};

use crate::error::{io_err, CoreError, Result};

const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_LOCK_VIOLATION: i32 = 33;

pub fn is_reparse_point(meta: &fs::Metadata) -> bool {
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

/// Chuẩn hóa thư mục cha rồi ghép tên: bản thân liên kết không bị phân giải sang đích.
fn canonical_location(path: &Path) -> io::Result<PathBuf> {
    match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
            Ok(fs::canonicalize(parent)?.join(name))
        }
        _ => fs::canonicalize(path),
    }
}

#[derive(Debug, Clone)]
pub struct Guard {
    roots: Vec<PathBuf>,
}

impl Guard {
    /// Gốc không tồn tại bị bỏ qua (không có gì để xóa ở đó).
    pub fn new(roots: &[PathBuf]) -> Guard {
        Guard { roots: roots.iter().filter_map(|r| fs::canonicalize(r).ok()).collect() }
    }

    pub fn is_allowed(&self, path: &Path) -> bool {
        match canonical_location(path) {
            Ok(loc) => self.roots.iter().any(|r| loc.starts_with(r)),
            Err(_) => false,
        }
    }

    pub fn check(&self, path: &Path) -> Result<()> {
        if self.is_allowed(path) {
            Ok(())
        } else {
            Err(CoreError::OutsideRoots(path.to_path_buf()))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted(u64),
    WouldDelete(u64),
    Locked,
    Vanished,
}

fn remove_entry(path: &Path, dir_link: bool) -> io::Result<()> {
    if dir_link {
        fs::remove_dir(path)
    } else {
        fs::remove_file(path)
    }
}

fn is_locked(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(ERROR_SHARING_VIOLATION) | Some(ERROR_LOCK_VIOLATION))
}

pub fn delete_path(guard: &Guard, path: &Path, dry_run: bool) -> Result<DeleteOutcome> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(DeleteOutcome::Vanished),
        Err(e) => return Err(io_err(path, e)),
    };
    guard.check(path)?;
    let reparse = is_reparse_point(&meta);
    if meta.is_dir() && !reparse {
        return Err(CoreError::System(format!(
            "refusing to delete a directory as a file: {}",
            path.display()
        )));
    }
    let bytes = if reparse { 0 } else { meta.len() };
    if dry_run {
        return Ok(DeleteOutcome::WouldDelete(bytes));
    }
    let dir_link = reparse && meta.file_attributes() & FILE_ATTRIBUTE_DIRECTORY != 0;
    let err = match remove_entry(path, dir_link) {
        Ok(()) => return Ok(DeleteOutcome::Deleted(bytes)),
        Err(e) => e,
    };
    if err.kind() == io::ErrorKind::NotFound {
        return Ok(DeleteOutcome::Vanished);
    }
    if is_locked(&err) {
        return Ok(DeleteOutcome::Locked);
    }
    if err.kind() == io::ErrorKind::PermissionDenied {
        if meta.file_attributes() & FILE_ATTRIBUTE_READONLY != 0 {
            let mut perms = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            if fs::set_permissions(path, perms).is_ok() && remove_entry(path, dir_link).is_ok() {
                return Ok(DeleteOutcome::Deleted(bytes));
            }
        }
        return Ok(DeleteOutcome::Locked);
    }
    Err(io_err(path, err))
}

/// Xóa thư mục nếu rỗng, nằm trong gốc được phép và không phải reparse point.
pub fn remove_empty_dir(guard: &Guard, dir: &Path) -> bool {
    if !guard.is_allowed(dir) {
        return false;
    }
    match fs::symlink_metadata(dir) {
        Ok(m) if m.is_dir() && !is_reparse_point(&m) => fs::remove_dir(dir).is_ok(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::write_file;
    use std::fs;
    use std::os::windows::fs::OpenOptionsExt;

    fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        (tmp, root, outside)
    }

    #[test]
    fn deleting_outside_allowed_roots_is_refused() {
        let (_t, root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let guard = Guard::new(std::slice::from_ref(&root));
        for dry in [false, true] {
            let err = delete_path(&guard, &secret, dry).unwrap_err();
            assert!(matches!(err, CoreError::OutsideRoots(_)), "{err}");
        }
        assert!(secret.exists());
    }

    #[test]
    fn dotdot_escape_is_refused() {
        let (_t, root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let sneaky = root.join("..").join("outside").join("secret.txt");
        let guard = Guard::new(&[root]);
        assert!(matches!(delete_path(&guard, &sneaky, false), Err(CoreError::OutsideRoots(_))));
        assert!(secret.exists());
    }

    #[test]
    fn file_inside_root_is_deleted_and_bytes_reported() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("a").join("x.tmp"), 7);
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Deleted(7));
        assert!(!f.exists());
    }

    #[test]
    fn dry_run_keeps_the_file() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("x.tmp"), 5);
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, true).unwrap(), DeleteOutcome::WouldDelete(5));
        assert!(f.exists());
    }

    #[test]
    fn locked_file_is_skipped_not_failed() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("busy.tmp"), 5);
        let _handle = fs::OpenOptions::new().read(true).share_mode(0).open(&f).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Locked);
        assert!(f.exists());
    }

    // Review Focus 2
    #[test]
    fn readonly_file_is_deleted_not_counted_as_locked() {
        let (_t, root, _o) = setup();
        let f = write_file(&root.join("ro.tmp"), 4);
        let mut p = fs::metadata(&f).unwrap().permissions();
        p.set_readonly(true);
        fs::set_permissions(&f, p).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Deleted(4));
        assert!(!f.exists());
    }

    // Review Focus 3
    #[test]
    fn vanished_file_is_reported_as_vanished() {
        let (_t, root, _o) = setup();
        let guard = Guard::new(std::slice::from_ref(&root));
        assert_eq!(delete_path(&guard, &root.join("gone.tmp"), false).unwrap(), DeleteOutcome::Vanished);
    }

    #[test]
    fn junction_is_removed_without_touching_its_target() {
        let (_t, root, outside) = setup();
        let secret = write_file(&outside.join("secret.txt"), 5);
        let link = root.join("link");
        junction::create(&outside, &link).unwrap();
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &link, false).unwrap(), DeleteOutcome::Deleted(0));
        assert!(fs::symlink_metadata(&link).is_err());
        assert!(secret.exists());
    }

    #[test]
    fn plain_directory_is_refused_by_delete_path() {
        let (_t, root, _o) = setup();
        let d = root.join("dir");
        fs::create_dir_all(&d).unwrap();
        let guard = Guard::new(&[root]);
        assert!(matches!(delete_path(&guard, &d, false), Err(CoreError::System(_))));
        assert!(d.exists());
    }

    #[test]
    fn path_longer_than_260_chars_is_deleted() {
        let (_t, root, _o) = setup();
        let mut deep = root.clone();
        for i in 0..25 {
            deep = deep.join(format!("thu_muc_sau_{i:02}"));
        }
        let f = write_file(&deep.join("tệp tạm.tmp"), 3);
        assert!(f.as_os_str().len() > 260);
        let guard = Guard::new(&[root]);
        assert_eq!(delete_path(&guard, &f, false).unwrap(), DeleteOutcome::Deleted(3));
    }

    #[test]
    fn remove_empty_dir_respects_emptiness_and_roots() {
        let (_t, root, outside) = setup();
        let empty = root.join("empty");
        fs::create_dir_all(&empty).unwrap();
        let full = root.join("full");
        write_file(&full.join("f"), 1);
        let guard = Guard::new(&[root]);
        assert!(remove_empty_dir(&guard, &empty));
        assert!(!empty.exists());
        assert!(!remove_empty_dir(&guard, &full));
        assert!(full.exists());
        assert!(!remove_empty_dir(&guard, &outside));
        assert!(outside.exists());
    }
}
