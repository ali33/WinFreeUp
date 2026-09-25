//! Ảnh chụp giá trị cũ để hoàn tác (spec mục 3.3): `%LOCALAPPDATA%\WinFreeUp\tweaks-undo.json`.
//! Chỉ ghi lần đầu; ghi file tạm rồi đổi tên; file hỏng ⇒ đổi thành `.bak`, không xoá.
use std::collections::BTreeMap;
use std::fs;
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

pub struct UndoStore {
    path: PathBuf,
    file: UndoFile,
}

/// `%LOCALAPPDATA%\WinFreeUp\tweaks-undo.json`.
pub fn default_path(local_appdata: &Path) -> PathBuf {
    local_appdata.join("WinFreeUp").join(FILE_NAME)
}

/// Tên `.bak` chưa dùng: `x.json.bak`, rồi `x.json.bak1`, `x.json.bak2`…
fn free_backup_name(path: &Path) -> PathBuf {
    let base = format!("{}.bak", path.display());
    let mut candidate = PathBuf::from(&base);
    let mut n = 1;
    while candidate.exists() {
        candidate = PathBuf::from(format!("{base}{n}"));
        n += 1;
    }
    candidate
}

impl UndoStore {
    /// Không có file ⇒ kho rỗng. File hỏng ⇒ đổi tên thành `.bak`, kho rỗng, và trả thông điệp
    /// nguyên văn để giao diện hiện băng hổ phách `undo_corrupt`.
    pub fn load(path: &Path) -> (UndoStore, Option<String>) {
        let empty = || UndoStore { path: path.to_path_buf(), file: UndoFile { version: VERSION, ..Default::default() } };
        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (empty(), None),
            Err(e) => return (empty(), Some(format!("{}: {e}", path.display()))),
        };
        match serde_json::from_str::<UndoFile>(&text) {
            Ok(file) if file.version == VERSION => (UndoStore { path: path.to_path_buf(), file }, None),
            other => {
                let detail = match other {
                    Ok(f) => format!("unsupported version {}", f.version),
                    Err(e) => e.to_string(),
                };
                let bak = free_backup_name(path);
                let moved = fs::rename(path, &bak).map(|_| bak.display().to_string()).unwrap_or_else(|e| format!("rename failed: {e}"));
                (empty(), Some(format!("{}: {detail} ⇒ {moved}", path.display())))
            }
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

    /// Ghi file tạm cạnh file thật rồi đổi tên (thay thế nguyên tử trên cùng ổ).
    pub fn save(&self) -> Result<(), String> {
        let dir = self.path.parent().ok_or_else(|| format!("{}: no parent", self.path.display()))?;
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let tmp = self.path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(&self.file).map_err(|e| e.to_string())?;
        fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        fs::rename(&tmp, &self.path).map_err(|e| format!("{}: {e}", self.path.display()))
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
}
