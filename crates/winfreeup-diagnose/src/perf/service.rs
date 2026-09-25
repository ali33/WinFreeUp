//! Tầng dịch vụ của mục Bộ nhớ & Hiệu năng: bật/tắt lấy mẫu, kết thúc app có chặn tiến trình thiết yếu,
//! mở vị trí file, biểu tượng app (nhớ đệm). Lỗi dạng chuỗi: `essential`, `unknown_app`, `bad_path`, hoặc
//! thông điệp hệ thống nguyên văn.
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use winfreeup_core::CoreError;

use super::apps::{AppRow, EssentialRules};
use super::icon::{icon_data_url, is_local_file};
use super::monitor::{ErrorSink, PerfMonitor, Sample, TickSink};
use super::netetw::stop_stale_session;
use crate::actlog::ActionLog;

/// Trần số biểu tượng nhớ đệm — đường dẫn do giao diện gửi, không để bộ nhớ đệm phình không giới hạn.
const MAX_ICONS: usize = 1024;

/// Kết thúc một tiến trình cụ thể — bản thật gọi `procs::kill`, test dùng bản giả.
pub trait Killer: Send + Sync {
    fn kill(&self, pid: u32, create_time: i64) -> Result<(), String>;
}

/// `procs::kill`: kiểm lại giờ tạo và luật thiết yếu (cả cây con cháu WinFreeUp) trên cùng handle.
pub struct RealKiller;

impl Killer for RealKiller {
    fn kill(&self, pid: u32, create_time: i64) -> Result<(), String> {
        super::procs::kill(pid, create_time).map_err(|e| match e {
            // Giữ nguyên mã/thông điệp (vd `essential`), không thêm tiền tố.
            CoreError::System(s) => s,
            other => other.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KillResult {
    pub killed: u32,
    /// Thông điệp nguyên văn cho từng tiến trình không kết thúc được.
    pub errors: Vec<String>,
    pub log_error: Option<String>,
}

pub struct PerfService {
    monitor: PerfMonitor,
    rules: Arc<EssentialRules>,
    killer: Box<dyn Killer>,
    log: Arc<ActionLog>,
    icons: Mutex<HashMap<String, Option<String>>>,
}

impl PerfService {
    pub fn new(rules: Arc<EssentialRules>, killer: Box<dyn Killer>, log: Arc<ActionLog>) -> Self {
        PerfService { monitor: PerfMonitor::default(), rules, killer, log, icons: Mutex::new(HashMap::new()) }
    }

    /// Bản chạy thật: luật thiết yếu dựng từ System32 lấy qua API (không qua biến môi trường), kết thúc
    /// tiến trình bằng [`RealKiller`].
    pub fn from_system(log: Arc<ActionLog>) -> Result<Self, String> {
        let system32 = crate::util::system_dir()?;
        let rules = Arc::new(EssentialRules::new(&system32, std::process::id()));
        Ok(Self::new(rules, Box::new(RealKiller), log))
    }

    /// Mục Bộ nhớ & Hiệu năng vừa hiện ra. Trả lịch sử còn giữ để vẽ ngay.
    pub fn start(&self, on_tick: TickSink, on_error: ErrorSink) -> Vec<Sample> {
        self.monitor.start(self.rules.clone(), on_tick, on_error)
    }

    /// Rời mục ⇒ dừng lấy mẫu (luồng lấy mẫu dừng phiên ETW của nó trước khi kết thúc).
    pub fn stop(&self) {
        self.monitor.stop();
    }

    /// Đóng ứng dụng: dừng lấy mẫu và phiên ETW `WinFreeUp-Net` còn sót.
    pub fn shutdown(&self) {
        self.monitor.stop();
        stop_stale_session();
    }

    /// Kết thúc mọi tiến trình của một app (theo khóa của mẫu gần nhất). Hai lớp chặn: luật thiết yếu theo
    /// bảng app ở đây, rồi `Killer` thật kiểm lại trên tiến trình sống.
    pub fn kill_app(&self, key: &str) -> Result<KillResult, String> {
        let apps = self.monitor.last_apps();
        self.kill_in(&apps, key)
    }

    fn kill_in(&self, apps: &[AppRow], key: &str) -> Result<KillResult, String> {
        let app = apps.iter().find(|a| a.key == key).ok_or("unknown_app")?;
        if app.essential || app.procs.iter().any(|p| p.essential) {
            return Err("essential".into());
        }
        let mut result = KillResult { killed: 0, errors: Vec::new(), log_error: None };
        let path = app.path.as_deref().unwrap_or(&app.key);
        for p in &app.procs {
            let (line, outcome) = match self.killer.kill(p.pid, p.create_time) {
                Ok(()) => (format!("APP_KILL pid={} {path}", p.pid), Ok(())),
                Err(e) => (format!("APP_KILL_FAILED pid={} {path} -- {e}", p.pid), Err(e)),
            };
            match outcome {
                Ok(()) => result.killed += 1,
                Err(e) => result.errors.push(e),
            }
            if let Err(e) = self.log.line(&line) {
                result.log_error = Some(e);
            }
        }
        Ok(result)
    }

    /// Mở Explorer chọn sẵn file. Chỉ nhận file có thật trên ổ cục bộ: đường mạng (`\\máy\…`) sẽ làm
    /// Explorer — chạy quyền Admin — kết nối SMB ra ngoài.
    pub fn reveal(&self, path: &str) -> Result<(), String> {
        let p = Path::new(path);
        if !is_local_file(p) || !p.exists() {
            return Err("bad_path".into());
        }
        crate::util::reveal_in_explorer(p)
    }

    pub fn icon(&self, path: &str) -> Option<String> {
        if let Some(v) = self.icons.lock().ok().and_then(|m| m.get(path).cloned()) {
            return v;
        }
        let v = icon_data_url(Path::new(path));
        if let Ok(mut m) = self.icons.lock() {
            if m.len() >= MAX_ICONS {
                m.clear();
            }
            m.insert(path.to_string(), v.clone());
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perf::apps::ProcRow;

    #[derive(Default)]
    struct FakeKiller {
        fail_pid: Option<u32>,
        calls: Arc<Mutex<Vec<u32>>>,
    }

    impl Killer for FakeKiller {
        fn kill(&self, pid: u32, _: i64) -> Result<(), String> {
            self.calls.lock().unwrap().push(pid);
            if Some(pid) == self.fail_pid {
                Err("Access is denied. (os error 5)".into())
            } else {
                Ok(())
            }
        }
    }

    fn proc_row(pid: u32, essential: bool) -> ProcRow {
        ProcRow { pid, create_time: 1, name: "x.exe".into(), ram: 1, cpu: 0.0, disk_bps: 0.0, net_up_bps: None, net_down_bps: None, essential }
    }

    fn app(key: &str, procs: Vec<ProcRow>) -> AppRow {
        AppRow {
            key: key.into(),
            name: key.into(),
            path: Some(format!(r"C:\Apps\{key}")),
            ram: 1,
            cpu: 0.0,
            disk_bps: 0.0,
            net_up_bps: None,
            net_down_bps: None,
            essential: procs.iter().any(|p| p.essential),
            procs,
        }
    }

    fn service(killer: FakeKiller) -> (tempfile::TempDir, PerfService) {
        let t = tempfile::tempdir().unwrap();
        let rules = Arc::new(EssentialRules::new(&crate::util::system_dir().unwrap(), 1));
        let s = PerfService::new(rules, Box::new(killer), Arc::new(ActionLog::new(t.path().join("logs"))));
        (t, s)
    }

    fn log_text(t: &tempfile::TempDir) -> String {
        let log = std::fs::read_dir(t.path().join("logs")).unwrap().next().unwrap().unwrap().path();
        std::fs::read_to_string(log).unwrap()
    }

    #[test]
    fn kill_app_ends_every_process_and_logs() {
        let (t, s) = service(FakeKiller::default());
        let apps = vec![app("game.exe", vec![proc_row(10, false), proc_row(11, false)])];
        let r = s.kill_in(&apps, "game.exe").unwrap();
        assert_eq!(r, KillResult { killed: 2, errors: vec![], log_error: None });
        let text = log_text(&t);
        assert!(text.contains(r"APP_KILL pid=10 C:\Apps\game.exe"));
        assert!(text.contains(r"APP_KILL pid=11 C:\Apps\game.exe"));
    }

    #[test]
    fn essential_apps_are_refused_without_touching_any_process() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let (_t, s) = service(FakeKiller { calls: calls.clone(), ..Default::default() });
        // Chỉ MỘT tiến trình thiết yếu cũng chặn cả app (cờ của app bị giao diện cũ báo sai vẫn không lọt).
        let mut row = app("svchost", vec![proc_row(20, false), proc_row(21, true)]);
        row.essential = false;
        assert_eq!(s.kill_in(&[row], "svchost").unwrap_err(), "essential");
        assert_eq!(s.kill_in(&[], "khong-co").unwrap_err(), "unknown_app");
        assert!(calls.lock().unwrap().is_empty(), "không tiến trình nào bị đụng");
    }

    #[test]
    fn partial_failure_reports_the_verbatim_error_and_logs_it() {
        let (t, s) = service(FakeKiller { fail_pid: Some(31), ..Default::default() });
        let apps = vec![app("a", vec![proc_row(30, false), proc_row(31, false)])];
        let r = s.kill_in(&apps, "a").unwrap();
        assert_eq!(r.killed, 1);
        assert_eq!(r.errors, vec!["Access is denied. (os error 5)".to_string()]);
        assert!(log_text(&t).contains(r"APP_KILL_FAILED pid=31 C:\Apps\a -- Access is denied. (os error 5)"));
    }

    #[test]
    fn kill_app_without_any_sample_is_unknown() {
        let (_t, s) = service(FakeKiller::default());
        assert_eq!(s.kill_app("game.exe").unwrap_err(), "unknown_app");
    }

    #[test]
    fn real_killer_is_a_second_layer_that_returns_the_bare_essential_code() {
        use std::os::windows::process::CommandExt;
        // Tiến trình con do chính test tạo là con cháu của «WinFreeUp» (tiến trình test) ⇒ thiết yếu với
        // `procs::kill`. Không đụng tiến trình thật nào khác.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut child = std::process::Command::new(crate::util::system_dir().unwrap().join("ping.exe"))
            .args(["-n", "30", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .unwrap();
        let pid = child.id();
        let r = crate::perf::procs::snapshot()
            .map_err(|e| e.to_string())
            .and_then(|all| all.into_iter().find(|p| p.pid == pid).ok_or_else(|| "không thấy tiến trình con".to_string()))
            .and_then(|info| RealKiller.kill(pid, info.create_time).map(|_| "đã kết thúc".to_string()));
        let still_alive = child.try_wait().unwrap().is_none();
        let _ = child.kill();
        let _ = child.wait();
        assert_eq!(r.unwrap_err(), "essential");
        assert!(still_alive, "lớp thứ hai từ chối thì tiến trình còn sống");
    }

    #[test]
    fn reveal_refuses_network_and_missing_paths_without_starting_explorer() {
        let (_t, s) = service(FakeKiller::default());
        assert_eq!(s.reveal(r"\\attacker\share\x.exe").unwrap_err(), "bad_path");
        assert_eq!(s.reveal(r"C:\khong-co\x.exe").unwrap_err(), "bad_path");
        assert_eq!(s.reveal("relative.exe").unwrap_err(), "bad_path");
    }

    #[test]
    fn icons_are_cached_including_misses() {
        let (_t, s) = service(FakeKiller::default());
        assert!(s.icon(r"C:\khong-co\x.exe").is_none());
        assert!(s.icons.lock().unwrap().contains_key(r"C:\khong-co\x.exe"));
    }

    #[test]
    fn from_system_builds_rules_without_environment_variables() {
        let t = tempfile::tempdir().unwrap();
        let s = PerfService::from_system(Arc::new(ActionLog::new(t.path().join("logs")))).unwrap();
        assert!(!s.monitor.is_running(), "chưa hiển thị thì chưa lấy mẫu");
    }
}
