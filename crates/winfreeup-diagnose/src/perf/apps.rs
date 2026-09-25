//! Gộp tiến trình thành "app" theo đường dẫn exe (spec 5.2) và luật chặn kết thúc tiến trình thiết yếu.
use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

/// Tên (không phân biệt hoa thường, bỏ `.exe`) của tiến trình thiết yếu — spec 5.2.
pub const ESSENTIAL_NAMES: [&str; 11] =
    ["system", "registry", "smss", "csrss", "wininit", "winlogon", "services", "lsass", "dwm", "svchost", "fontdrvhost"];

pub struct EssentialRules {
    /// `%WINDIR%\System32`, chữ thường.
    system32: String,
    self_pid: u32,
}

impl EssentialRules {
    pub fn new(system32: &Path, self_pid: u32) -> Self {
        EssentialRules { system32: system32.to_string_lossy().trim_end_matches('\\').to_lowercase(), self_pid }
    }

    /// Chặn khi: là chính WinFreeUp hoặc tiến trình con của nó (WebView2); hoặc tên nằm trong danh sách
    /// **và** exe nằm ngay trong System32 (hoặc không đọc được đường dẫn — chặn cho chắc).
    /// Exe trùng tên đặt ở chỗ khác (vd `C:\Temp\svchost.exe`) thì vẫn cho kết thúc.
    pub fn is_essential(&self, pid: u32, parent_pid: u32, name: &str, path: Option<&str>) -> bool {
        if pid == self.self_pid || parent_pid == self.self_pid || pid == 4 || pid == 0 {
            return true;
        }
        let lower = name.to_lowercase();
        let stem = lower.strip_suffix(".exe").unwrap_or(&lower);
        if !ESSENTIAL_NAMES.contains(&stem) {
            return false;
        }
        match path {
            None => true,
            Some(p) => {
                let p = p.to_lowercase();
                Path::new(&p).parent().is_some_and(|d| d.to_string_lossy().trim_end_matches('\\') == self.system32)
            }
        }
    }
}

/// Một tiến trình trong mẫu hiện tại, đã kèm đường dẫn và tên thân thiện.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcSample {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub path: Option<String>,
    /// Tên từ `FileDescription` của exe; rỗng ⇒ dùng tên file.
    pub description: String,
    pub create_time: i64,
    pub private_ws: u64,
    pub cpu_100ns: u64,
    pub io_bytes: u64,
    /// Byte mạng (lên, xuống) trong chu kỳ vừa rồi — từ ETW; `None` khi ETW không chạy.
    pub net: Option<(u64, u64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ProcRow {
    pub pid: u32,
    pub create_time: i64,
    pub name: String,
    pub ram: u64,
    pub cpu: f32,
    pub disk_bps: f64,
    pub net_up_bps: Option<f64>,
    pub net_down_bps: Option<f64>,
    pub essential: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppRow {
    /// Khóa gộp: đường dẫn exe chữ thường, hoặc `name:<tên>` khi không đọc được đường dẫn.
    pub key: String,
    pub name: String,
    pub path: Option<String>,
    pub ram: u64,
    pub cpu: f32,
    pub disk_bps: f64,
    pub net_up_bps: Option<f64>,
    pub net_down_bps: Option<f64>,
    /// Có ít nhất một tiến trình thiết yếu ⇒ không cho kết thúc cả app.
    pub essential: bool,
    pub procs: Vec<ProcRow>,
}

pub fn app_key(name: &str, path: Option<&str>) -> String {
    match path {
        Some(p) => p.to_lowercase(),
        None => format!("name:{}", name.to_lowercase()),
    }
}

fn display_name(p: &ProcSample) -> String {
    if !p.description.trim().is_empty() {
        return p.description.trim().to_string();
    }
    match &p.path {
        Some(path) => Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| p.name.clone()),
        None => p.name.clone(),
    }
}

/// Số liệu cộng dồn của lần trước, theo (pid, giờ tạo): (thời gian CPU, byte vào/ra).
pub type PrevTotals = HashMap<(u32, i64), (u64, u64)>;

/// Tính một mẫu bảng ứng dụng. Tiến trình mới xuất hiện (chưa có số lần trước) tính 0 cho CPU/đĩa.
/// Kết quả sắp theo RAM giảm dần (mặc định của spec); `next` trả về số cộng dồn cho lần sau.
pub fn aggregate(prev: &PrevTotals, cur: &[ProcSample], dt_ms: u64, cpus: u32, rules: &EssentialRules) -> (Vec<AppRow>, PrevTotals) {
    let dt_s = (dt_ms.max(1)) as f64 / 1000.0;
    let cpu_capacity = dt_ms.max(1) as f64 * 10_000.0 * f64::from(cpus.max(1));
    let mut next = PrevTotals::with_capacity(cur.len());
    let mut apps: HashMap<String, AppRow> = HashMap::new();
    for p in cur {
        next.insert((p.pid, p.create_time), (p.cpu_100ns, p.io_bytes));
        let (cpu, disk_bps) = match prev.get(&(p.pid, p.create_time)) {
            Some(&(c0, io0)) => (
                (p.cpu_100ns.saturating_sub(c0) as f64 / cpu_capacity * 100.0).min(100.0) as f32,
                p.io_bytes.saturating_sub(io0) as f64 / dt_s,
            ),
            None => (0.0, 0.0),
        };
        let (up, down) = match p.net {
            Some((u, d)) => (Some(u as f64 / dt_s), Some(d as f64 / dt_s)),
            None => (None, None),
        };
        let essential = rules.is_essential(p.pid, p.parent_pid, &p.name, p.path.as_deref());
        let key = app_key(&p.name, p.path.as_deref());
        let row = apps.entry(key.clone()).or_insert_with(|| AppRow {
            key,
            name: display_name(p),
            path: p.path.clone(),
            ram: 0,
            cpu: 0.0,
            disk_bps: 0.0,
            net_up_bps: up.map(|_| 0.0),
            net_down_bps: down.map(|_| 0.0),
            essential: false,
            procs: Vec::new(),
        });
        row.ram += p.private_ws;
        row.cpu += cpu;
        row.disk_bps += disk_bps;
        row.net_up_bps = row.net_up_bps.zip(up).map(|(a, b)| a + b);
        row.net_down_bps = row.net_down_bps.zip(down).map(|(a, b)| a + b);
        row.essential |= essential;
        row.procs.push(ProcRow {
            pid: p.pid,
            create_time: p.create_time,
            name: p.name.clone(),
            ram: p.private_ws,
            cpu,
            disk_bps,
            net_up_bps: up,
            net_down_bps: down,
            essential,
        });
    }
    let mut rows: Vec<AppRow> = apps.into_values().collect();
    for r in &mut rows {
        r.cpu = r.cpu.min(100.0);
        r.procs.sort_by(|a, b| b.ram.cmp(&a.ram));
    }
    rows.sort_by(|a, b| b.ram.cmp(&a.ram).then_with(|| a.key.cmp(&b.key)));
    (rows, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> EssentialRules {
        EssentialRules::new(Path::new(r"C:\Windows\System32"), 999)
    }

    #[test]
    fn essential_by_name_and_system32_location() {
        let r = rules();
        assert!(r.is_essential(700, 600, "svchost.exe", Some(r"C:\Windows\System32\svchost.exe")));
        assert!(r.is_essential(700, 600, "LSASS.EXE", Some(r"c:\windows\system32\lsass.exe")));
        assert!(r.is_essential(88, 4, "Registry", None), "tiến trình nhân không có đường dẫn");
        assert!(r.is_essential(4, 0, "System", None));
        assert!(r.is_essential(710, 600, "csrss.exe", None), "không đọc được đường dẫn ⇒ chặn cho chắc");
    }

    #[test]
    fn same_name_outside_system32_can_be_ended() {
        let r = rules();
        assert!(!r.is_essential(701, 600, "svchost.exe", Some(r"C:\Temp\svchost.exe")));
        assert!(!r.is_essential(702, 600, "dwm.exe", Some(r"C:\Windows\System32\sub\dwm.exe")), "phải nằm ngay trong System32");
        assert!(!r.is_essential(703, 600, "chrome.exe", Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe")));
    }

    #[test]
    fn winfreeup_itself_and_its_webview_are_protected() {
        let r = rules();
        assert!(r.is_essential(999, 1, "WinFreeUp.exe", Some(r"D:\WinFreeUp.exe")));
        assert!(r.is_essential(1234, 999, "msedgewebview2.exe", Some(r"C:\Program Files (x86)\Microsoft\EdgeWebView\msedgewebview2.exe")));
    }

    fn sample(pid: u32, name: &str, path: Option<&str>, ram: u64, cpu: u64, io: u64) -> ProcSample {
        ProcSample {
            pid,
            parent_pid: 1,
            name: name.into(),
            path: path.map(Into::into),
            description: String::new(),
            create_time: 10,
            private_ws: ram,
            cpu_100ns: cpu,
            io_bytes: io,
            net: None,
        }
    }

    #[test]
    fn processes_group_by_exe_path_and_rates_come_from_deltas() {
        let chrome = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
        let mut prev = PrevTotals::new();
        prev.insert((1, 10), (0, 0));
        prev.insert((2, 10), (0, 1_000_000));
        let mut a = sample(1, "chrome.exe", Some(chrome), 300, 5_000_000, 0);
        a.description = "Google Chrome".into();
        let b = sample(2, "chrome.exe", Some(&chrome.to_uppercase()), 200, 5_000_000, 3_000_000);
        let c = sample(3, "notepad.exe", Some(r"C:\Windows\notepad.exe"), 900, 1, 1);
        // 1 giây, 2 lõi ⇒ năng lực 2e7 × 100 ns; mỗi tiến trình chrome dùng 5e6 ⇒ 25%.
        let (rows, next) = aggregate(&prev, &[a, b, c], 1000, 2, &rules());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "notepad", "sắp theo RAM giảm dần; không có FileDescription thì dùng tên file");
        assert_eq!(rows[0].cpu, 0.0, "tiến trình mới chưa có số lần trước");
        let c_row = &rows[1];
        assert_eq!(c_row.name, "Google Chrome");
        assert_eq!(c_row.ram, 500);
        assert_eq!(c_row.cpu, 50.0);
        assert_eq!(c_row.disk_bps, 2_000_000.0);
        assert_eq!(c_row.procs.len(), 2);
        assert_eq!(c_row.procs[0].pid, 1);
        assert_eq!(next.get(&(2, 10)), Some(&(5_000_000, 3_000_000)));
    }

    #[test]
    fn a_reused_pid_is_not_mistaken_for_the_old_process() {
        let mut prev = PrevTotals::new();
        prev.insert((5, 1), (9_000_000, 9_000_000));
        let mut p = sample(5, "x.exe", Some(r"C:\x.exe"), 1, 100, 100);
        p.create_time = 2;
        let (rows, _) = aggregate(&prev, &[p], 1000, 1, &rules());
        assert_eq!(rows[0].cpu, 0.0);
        assert_eq!(rows[0].disk_bps, 0.0);
    }

    #[test]
    fn network_columns_stay_empty_without_etw_and_sum_with_it() {
        let (rows, _) = aggregate(&PrevTotals::new(), &[sample(1, "a.exe", None, 1, 0, 0)], 1000, 1, &rules());
        assert_eq!(rows[0].net_down_bps, None);
        assert_eq!(rows[0].key, "name:a.exe");
        let mut p1 = sample(1, "a.exe", Some(r"C:\a.exe"), 1, 0, 0);
        let mut p2 = sample(2, "a.exe", Some(r"C:\a.exe"), 1, 0, 0);
        p1.net = Some((1000, 4000));
        p2.net = Some((0, 2000));
        let (rows, _) = aggregate(&PrevTotals::new(), &[p1, p2], 2000, 1, &rules());
        assert_eq!(rows[0].net_up_bps, Some(500.0));
        assert_eq!(rows[0].net_down_bps, Some(3000.0));
    }

    #[test]
    fn one_essential_process_locks_the_whole_app() {
        let s32 = r"C:\Windows\System32\svchost.exe";
        let (rows, _) = aggregate(&PrevTotals::new(), &[sample(1, "svchost.exe", Some(s32), 1, 0, 0)], 1000, 1, &rules());
        assert!(rows[0].essential && rows[0].procs[0].essential);
    }
}
