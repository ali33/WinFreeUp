//! Ảnh chụp giá trị cũ để hoàn tác (spec mục 3.3): `%LOCALAPPDATA%\WinFreeUp\tweaks-undo.json`.
//! Chỉ ghi lần đầu; ghi file tạm rồi đổi tên; file hỏng ⇒ đổi thành `.bak`, không xoá.
//! Không đọc được / không cất được file hỏng / gặp reparse point ⇒ kho chỉ-đọc: `save()` trả lỗi,
//! không bao giờ ghi đè file đang có.
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::StartType;
use crate::ops::RegData;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Snapshot {
    /// `data = None` ⇒ trước khi áp dụng value không tồn tại.
    Registry { data: Option<RegData> },
    Service { start: StartType },
    Task { enabled: bool },
    /// Gói đã có trên máy trước khi gỡ.
    Appx { family: String },
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct UndoFile {
    version: u32,
    /// tweak_id ⇒ (chỉ số thao tác ⇒ ảnh chụp).
    entries: BTreeMap<String, BTreeMap<usize, Snapshot>>,
}

pub const FILE_NAME: &str = "tweaks-undo.json";
const VERSION: u32 = 1;
/// File lớn hơn mức này coi là hỏng (vài trăm mục chỉ tốn vài chục KB).
const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// `.bak`, `.bak1` … `.bak99`; hết tên ⇒ kho chỉ-đọc.
const MAX_BAK: u32 = 99;
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

pub struct UndoStore {
    path: PathBuf,
    file: UndoFile,
    /// `Some(lý do)` ⇒ không được ghi: `save()` trả `Err`, file trên đĩa giữ nguyên.
    readonly: Option<String>,
}

/// `%LOCALAPPDATA%\WinFreeUp\tweaks-undo.json`.
pub fn default_path(local_appdata: &Path) -> PathBuf {
    local_appdata.join("WinFreeUp").join(FILE_NAME)
}

/// Tên `.bak` chưa dùng: `x.json.bak`, rồi `x.json.bak1` … `x.json.bak99`; hết ⇒ `None`.
/// «Chưa dùng» xét bằng `symlink_metadata` nên junction/symlink treo cũng tính là đã dùng.
fn free_backup_name(path: &Path) -> Option<PathBuf> {
    (0..=MAX_BAK)
        .map(|n| {
            let mut s: OsString = path.as_os_str().to_os_string();
            s.push(".bak");
            if n > 0 {
                s.push(n.to_string());
            }
            PathBuf::from(s)
        })
        .find(|p| matches!(fs::symlink_metadata(p), Err(e) if e.kind() == ErrorKind::NotFound))
}

/// Junction / symlink / reparse point khác (không theo liên kết). Không tồn tại ⇒ `false`.
fn is_reparse_point(p: &Path) -> bool {
    match fs::symlink_metadata(p) {
        #[cfg(windows)]
        Ok(m) => {
            use std::os::windows::fs::MetadataExt;
            m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
        }
        #[cfg(not(windows))]
        Ok(m) => m.file_type().is_symlink(),
        Err(_) => false,
    }
}

fn refuse_reparse(paths: &[&Path]) -> Result<(), String> {
    match paths.iter().find(|p| is_reparse_point(p)) {
        Some(p) => Err(format!("{}: reparse point (junction/symlink) — refused", p.display())),
        None => Ok(()),
    }
}

fn parse(bytes: Vec<u8>) -> Result<UndoFile, String> {
    let text = String::from_utf8(bytes).map_err(|e| format!("not UTF-8: {e}"))?;
    let file = serde_json::from_str::<UndoFile>(&text).map_err(|e| e.to_string())?;
    if file.version != VERSION {
        return Err(format!("unsupported version {}", file.version));
    }
    Ok(file)
}

/// Ghi `bytes` vào `tmp` (tạo mới/cắt cụt), đẩy xuống đĩa rồi mới trả về.
fn write_synced(tmp: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut f = fs::File::create(tmp)?;
    f.write_all(bytes)?;
    f.sync_all()
}

impl UndoStore {
    fn new(path: &Path, file: UndoFile, readonly: Option<String>) -> UndoStore {
        UndoStore { path: path.to_path_buf(), file, readonly }
    }

    fn empty_file() -> UndoFile {
        UndoFile { version: VERSION, ..Default::default() }
    }

    fn tmp_path(&self) -> PathBuf {
        self.path.with_extension("json.tmp")
    }

    /// Không có file ⇒ kho rỗng. File hỏng (không UTF-8, JSON sai, sai phiên bản, quá 1 MB) ⇒
    /// đổi tên thành `.bak`, kho rỗng, và trả thông điệp nguyên văn để giao diện hiện băng hổ
    /// phách `undo_corrupt`. Không đọc được (khoá, ACL), không đổi tên được hoặc gặp reparse point
    /// ⇒ kho rỗng **chỉ-đọc** kèm thông điệp.
    pub fn load(path: &Path) -> (UndoStore, Option<String>) {
        let readonly = |msg: String| (UndoStore::new(path, Self::empty_file(), Some(msg.clone())), Some(msg));
        let parent = path.parent().unwrap_or(Path::new(""));
        if let Err(e) = refuse_reparse(&[parent, path]) {
            return readonly(e);
        }
        let parsed = match fs::symlink_metadata(path) {
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return (UndoStore::new(path, Self::empty_file(), None), None);
            }
            Err(e) => return readonly(format!("{}: {e}", path.display())),
            Ok(m) if m.is_file() && m.len() > MAX_FILE_BYTES => {
                Err(format!("file too large ({} bytes > {MAX_FILE_BYTES})", m.len()))
            }
            Ok(_) => match fs::read(path) {
                Ok(bytes) => parse(bytes),
                Err(e) => return readonly(format!("{}: {e}", path.display())),
            },
        };
        let detail = match parsed {
            Ok(file) => return (UndoStore::new(path, file, None), None),
            Err(detail) => detail,
        };
        let Some(bak) = free_backup_name(path) else {
            return readonly(format!(
                "{}: {detail} ⇒ no free backup name (.bak … .bak{MAX_BAK})",
                path.display()
            ));
        };
        match fs::rename(path, &bak) {
            Ok(()) => (
                UndoStore::new(path, Self::empty_file(), None),
                Some(format!("{}: {detail} ⇒ {}", path.display(), bak.display())),
            ),
            Err(e) => readonly(format!("{}: {detail} ⇒ rename to {} failed: {e}", path.display(), bak.display())),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get(&self, id: &str, op: usize) -> Option<&Snapshot> {
        self.file.entries.get(id).and_then(|m| m.get(&op))
    }

    pub fn has_any(&self, id: &str) -> bool {
        self.file.entries.get(id).is_some_and(|m| !m.is_empty())
    }

    /// Chỉ ghi khi chưa có ảnh chụp cho (id, op). Trả `true` nếu vừa ghi.
    pub fn record_if_absent(&mut self, id: &str, op: usize, snap: Snapshot) -> bool {
        let m = self.file.entries.entry(id.to_string()).or_default();
        if m.contains_key(&op) {
            return false;
        }
        m.insert(op, snap);
        true
    }

    pub fn remove(&mut self, id: &str, op: usize) {
        if let Some(m) = self.file.entries.get_mut(id) {
            m.remove(&op);
            if m.is_empty() {
                self.file.entries.remove(id);
            }
        }
    }

    /// Ghi file tạm cạnh file thật (write_all + sync_all) rồi đổi tên (thay thế nguyên tử trên
    /// cùng ổ). Lỗi ⇒ xoá file tạm. Kho chỉ-đọc hoặc gặp reparse point ⇒ `Err`, không ghi gì.
    pub fn save(&self) -> Result<(), String> {
        if let Some(why) = &self.readonly {
            return Err(format!("{}: undo store is read-only ({why})", self.path.display()));
        }
        let dir = self.path.parent().ok_or_else(|| format!("{}: no parent", self.path.display()))?;
        let tmp = self.tmp_path();
        refuse_reparse(&[dir, &self.path, &tmp])?;
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        refuse_reparse(&[dir, &self.path, &tmp])?;
        let text = serde_json::to_string_pretty(&self.file).map_err(|e| format!("{}: {e}", self.path.display()))?;
        let result = write_synced(&tmp, text.as_bytes())
            .map_err(|e| format!("{}: {e}", tmp.display()))
            .and_then(|()| fs::rename(&tmp, &self.path).map_err(|e| format!("{}: {e}", self.path.display())));
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(v: u32) -> Snapshot {
        Snapshot::Registry { data: Some(RegData::Dword(v)) }
    }

    #[test]
    fn missing_file_is_empty_store() {
        let d = tempfile::tempdir().unwrap();
        let (s, warn) = UndoStore::load(&d.path().join("x.json"));
        assert!(warn.is_none());
        assert!(s.get("a", 0).is_none());
    }

    #[test]
    fn snapshot_is_written_only_the_first_time_and_survives_reload() {
        let d = tempfile::tempdir().unwrap();
        let p = default_path(d.path());
        let (mut s, _) = UndoStore::load(&p);
        assert!(s.record_if_absent("ads_id", 0, snap(1)));
        assert!(!s.record_if_absent("ads_id", 0, snap(0)), "áp dụng lại không đè ảnh chụp gốc");
        s.save().unwrap();
        let (s2, warn) = UndoStore::load(&p);
        assert!(warn.is_none());
        assert_eq!(s2.get("ads_id", 0), Some(&snap(1)));
        assert!(!p.with_extension("json.tmp").exists(), "không để lại file tạm");
    }

    #[test]
    fn remove_last_op_drops_the_tweak() {
        let d = tempfile::tempdir().unwrap();
        let (mut s, _) = UndoStore::load(&d.path().join("u.json"));
        s.record_if_absent("t", 0, snap(1));
        s.record_if_absent("t", 1, Snapshot::Registry { data: None });
        s.remove("t", 0);
        assert!(s.has_any("t"));
        s.remove("t", 1);
        assert!(!s.has_any("t"));
    }

    #[test]
    fn corrupt_file_is_renamed_to_bak_not_deleted() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("tweaks-undo.json");
        fs::write(&p, "{ not json").unwrap();
        let (s, warn) = UndoStore::load(&p);
        assert!(warn.unwrap().contains("tweaks-undo.json"));
        assert!(!s.has_any("x"));
        assert!(!p.exists());
        assert_eq!(fs::read_to_string(d.path().join("tweaks-undo.json.bak")).unwrap(), "{ not json");
        // Hỏng lần hai: không đè .bak cũ.
        fs::write(&p, "[]").unwrap();
        let _ = UndoStore::load(&p);
        assert!(d.path().join("tweaks-undo.json.bak1").exists());
        assert_eq!(fs::read_to_string(d.path().join("tweaks-undo.json.bak")).unwrap(), "{ not json");
    }

    #[test]
    fn json_shape_is_stable() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("u.json");
        let (mut s, _) = UndoStore::load(&p);
        s.record_if_absent("svc_diagtrack", 0, Snapshot::Service { start: StartType::Auto });
        s.save().unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["entries"]["svc_diagtrack"]["0"]["kind"], "service");
        assert_eq!(v["entries"]["svc_diagtrack"]["0"]["start"], "auto");
    }

    fn entries_in(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    #[test]
    fn non_utf8_file_is_renamed_to_bak() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("tweaks-undo.json");
        fs::write(&p, [0xFF, 0xFE, 0x00]).unwrap();
        let (s, warn) = UndoStore::load(&p);
        assert!(warn.unwrap().contains("UTF-8"));
        assert!(!p.exists());
        assert_eq!(fs::read(d.path().join("tweaks-undo.json.bak")).unwrap(), [0xFF, 0xFE, 0x00]);
        s.save().expect("kho sau khi cất .bak vẫn ghi được");
    }

    #[test]
    fn unknown_version_is_renamed_to_bak() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("tweaks-undo.json");
        fs::write(&p, r#"{"version":2,"entries":{}}"#).unwrap();
        let (_, warn) = UndoStore::load(&p);
        assert!(warn.unwrap().contains("unsupported version 2"));
        assert!(!p.exists());
        assert!(d.path().join("tweaks-undo.json.bak").exists());
    }

    #[test]
    fn file_over_1_mb_is_treated_as_corrupt() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("tweaks-undo.json");
        fs::write(&p, vec![b' '; 1024 * 1024 + 1]).unwrap();
        let (_, warn) = UndoStore::load(&p);
        assert!(warn.unwrap().contains("too large"));
        assert!(!p.exists());
        assert!(d.path().join("tweaks-undo.json.bak").exists());
    }

    #[test]
    fn unreadable_file_makes_store_readonly_and_save_does_not_overwrite() {
        // Đường dẫn là một thư mục ⇒ đọc lỗi khác NotFound (Windows: Access is denied).
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("tweaks-undo.json");
        fs::create_dir(&p).unwrap();
        fs::write(p.join("keep.txt"), "x").unwrap();
        let (mut s, warn) = UndoStore::load(&p);
        assert!(warn.unwrap().contains("tweaks-undo.json"));
        assert!(s.record_if_absent("t", 0, snap(1)));
        assert!(s.save().unwrap_err().contains("read-only"));
        assert!(p.is_dir() && p.join("keep.txt").exists(), "không đụng vào thứ đang nằm ở đường dẫn");
        assert!(!p.with_extension("json.tmp").exists());
        assert!(!d.path().join("tweaks-undo.json.bak").exists());
    }

    #[test]
    fn no_free_bak_name_makes_store_readonly() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("tweaks-undo.json");
        fs::write(&p, "{ not json").unwrap();
        fs::write(d.path().join("tweaks-undo.json.bak"), "old").unwrap();
        for n in 1..=99 {
            fs::write(d.path().join(format!("tweaks-undo.json.bak{n}")), "old").unwrap();
        }
        let (s, warn) = UndoStore::load(&p);
        assert!(warn.unwrap().contains("no free backup name"));
        assert!(s.save().is_err());
        assert_eq!(fs::read_to_string(&p).unwrap(), "{ not json", "file hỏng giữ nguyên, không bị đè");
        assert!(!d.path().join("tweaks-undo.json.bak100").exists());
    }

    #[cfg(windows)]
    #[test]
    fn failed_bak_rename_makes_store_readonly() {
        use std::os::windows::fs::OpenOptionsExt;
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("tweaks-undo.json");
        fs::write(&p, "{ not json").unwrap();
        // Giữ file mở, chỉ cho chia sẻ ĐỌC (FILE_SHARE_READ = 1) ⇒ đọc được nhưng không đổi tên được.
        let hold = fs::OpenOptions::new().read(true).share_mode(1).open(&p).unwrap();
        let (s, warn) = UndoStore::load(&p);
        assert!(warn.unwrap().contains("rename to"));
        assert!(s.save().unwrap_err().contains("read-only"));
        drop(hold);
        assert_eq!(fs::read_to_string(&p).unwrap(), "{ not json");
        assert!(!d.path().join("tweaks-undo.json.bak").exists());
    }

    #[test]
    fn failed_save_leaves_no_tmp_file() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("u.json");
        let (mut s, _) = UndoStore::load(&p);
        s.record_if_absent("t", 0, snap(1));
        // Sau khi nạp, có thứ khác chiếm đường dẫn (một thư mục) ⇒ đổi tên file tạm thất bại.
        fs::create_dir(&p).unwrap();
        let err = s.save().unwrap_err();
        assert!(err.contains("u.json"), "{err}");
        assert!(!p.with_extension("json.tmp").exists(), "không để lại file tạm");
        assert!(p.is_dir());
    }

    #[cfg(windows)]
    fn junction(link: &Path, target: &Path) {
        let out = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap();
        assert!(out.status.success(), "mklink /J: {}", String::from_utf8_lossy(&out.stderr));
    }

    #[cfg(windows)]
    #[test]
    fn junction_as_parent_dir_is_refused_on_load_and_save() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("target");
        fs::create_dir(&target).unwrap();
        let link = d.path().join("WinFreeUp");
        let p = link.join(FILE_NAME);

        // Junction dựng SAU khi nạp: save phải từ chối.
        let (mut s, warn) = UndoStore::load(&p);
        assert!(warn.is_none());
        s.record_if_absent("t", 0, snap(1));
        junction(&link, &target);
        assert!(s.save().unwrap_err().contains("reparse point"));
        assert!(entries_in(&target).is_empty(), "không file nào xuất hiện ở đích");

        // Junction có sẵn khi nạp: kho chỉ-đọc.
        let (s2, warn) = UndoStore::load(&p);
        assert!(warn.unwrap().contains("reparse point"));
        assert!(s2.save().is_err());
        assert!(entries_in(&target).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn junction_at_tmp_path_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("target");
        fs::create_dir(&target).unwrap();
        let p = d.path().join("u.json");
        junction(&p.with_extension("json.tmp"), &target);
        let (mut s, warn) = UndoStore::load(&p);
        assert!(warn.is_none());
        s.record_if_absent("t", 0, snap(1));
        assert!(s.save().unwrap_err().contains("reparse point"));
        assert!(!p.exists());
        assert!(entries_in(&target).is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn junction_at_main_path_is_refused_and_not_renamed() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("target");
        fs::create_dir(&target).unwrap();
        let p = d.path().join("u.json");
        junction(&p, &target);
        let (s, warn) = UndoStore::load(&p);
        assert!(warn.unwrap().contains("reparse point"));
        assert!(s.save().is_err());
        assert!(is_reparse_point(&p), "junction giữ nguyên, không bị đổi tên thành .bak");
        assert!(!d.path().join("u.json.bak").exists());
        assert!(entries_in(&target).is_empty());
    }
}
