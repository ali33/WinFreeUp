//! Vòng lấy mẫu của mục Bộ nhớ & Hiệu năng (spec 5): chỉ chạy khi mục đang hiển thị, mỗi 1 giây,
//! giữ 5 phút gần nhất; chi phí của chính mình > 2% CPU ⇒ giãn còn 2 giây.
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use super::apps::{aggregate, AppRow, EssentialRules, PrevTotals, ProcSample};
use super::counters::{memory, net_totals, CpuDiskCounters};
use super::detect::{find_stutters, find_throttle, next_interval_ms, throttled_now, top_apps, AppLoad, Point, TempState, TempTracker, TopApp};
use super::icon::file_description;
use super::netetw::NetTrace;
use super::procs::{own_cpu_100ns, snapshot, PathCache};
use super::thermal::ThermalReader;

pub const HISTORY_MS: u64 = 5 * 60 * 1000;
pub const BASE_INTERVAL_MS: u32 = 1000;
/// Đọc nhiệt độ mỗi 5 giây — truy vấn WMI tốn hơn các bộ đếm khác.
pub const TEMP_EVERY_MS: u64 = 5000;
/// Trần số mẫu giữ lại (5 phút ở nhịp 1 giây, gồm cả hai đầu). Chặn lịch sử phình ra khi đồng hồ hệ thống
/// bị chỉnh lùi (mẫu cũ mang giờ «tương lai» thì cắt theo giờ không bỏ được).
const MAX_FRAMES: usize = (HISTORY_MS / BASE_INTERVAL_MS as u64) as usize + 1;

/// Số liệu toàn máy của một mẫu — điểm trên bốn biểu đồ.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sample {
    /// Giờ Unix (ms) lúc lấy mẫu.
    pub t_ms: u64,
    pub dur_ms: u64,
    pub cpu: f32,
    pub ram_pct: f32,
    pub ram_used: u64,
    pub ram_total: u64,
    pub disk_active: Option<f32>,
    pub net_up_bps: Option<f64>,
    pub net_down_bps: Option<f64>,
    pub cpu_perf: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StutterView {
    pub start_ms: u64,
    pub end_ms: u64,
    pub cpu: bool,
    pub disk: bool,
    pub top_cpu: Vec<TopApp>,
    pub top_disk: Vec<TopApp>,
    /// Trùng lúc CPU đang bị hạ xung.
    pub throttled: bool,
}

/// Payload sự kiện `perf-tick`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PerfTick {
    pub sample: Sample,
    pub apps: Vec<AppRow>,
    pub stutters: Vec<StutterView>,
    /// `None` ⇒ bộ đếm hiệu năng CPU không có: ẩn chỉ báo hạ xung.
    pub throttled: Option<bool>,
    pub perf_error: Option<String>,
    pub disk_error: Option<String>,
    pub temp: TempState,
    /// `None` ⇒ ETW đang chạy (có cột Mạng theo app); có chữ ⇒ lý do nguyên văn, cột Mạng hiện «–».
    pub net_app_error: Option<String>,
    pub interval_ms: u32,
}

struct Frame {
    sample: Sample,
    loads: Vec<AppLoad>,
}

/// 5 phút mẫu gần nhất cùng tải từng app — đủ để tìm cơn giật và 3 app ngốn nhất trong cơn.
#[derive(Default)]
pub struct History {
    frames: VecDeque<Frame>,
}

impl History {
    pub fn push(&mut self, sample: Sample, loads: Vec<AppLoad>) {
        let cutoff = sample.t_ms.saturating_sub(HISTORY_MS);
        self.frames.push_back(Frame { sample, loads });
        while self.frames.front().is_some_and(|f| f.sample.t_ms < cutoff) || self.frames.len() > MAX_FRAMES {
            self.frames.pop_front();
        }
    }

    pub fn samples(&self) -> Vec<Sample> {
        self.frames.iter().map(|f| f.sample.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    fn points(&self) -> Vec<Point> {
        self.frames
            .iter()
            .map(|f| Point {
                t_ms: f.sample.t_ms,
                dur_ms: f.sample.dur_ms,
                cpu: f.sample.cpu,
                disk: f.sample.disk_active.unwrap_or(0.0),
                perf: f.sample.cpu_perf,
            })
            .collect()
    }

    pub fn throttled_now(&self) -> Option<bool> {
        throttled_now(&self.points())
    }

    pub fn stutters(&self) -> Vec<StutterView> {
        let points = self.points();
        let throttle = find_throttle(&points);
        find_stutters(&points)
            .into_iter()
            .map(|s| {
                let frames: Vec<&[AppLoad]> = self.frames.range(s.first..=s.last).map(|f| f.loads.as_slice()).collect();
                StutterView {
                    start_ms: s.start_ms,
                    end_ms: s.end_ms,
                    cpu: s.cpu,
                    disk: s.disk,
                    top_cpu: if s.cpu { top_apps(&frames, true, 3) } else { Vec::new() },
                    top_disk: if s.disk { top_apps(&frames, false, 3) } else { Vec::new() },
                    throttled: throttle.iter().any(|t| t.start_ms < s.end_ms && s.start_ms < t.end_ms),
                }
            })
            .collect()
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Mở phiên ETW mạng theo app. Bản thật là [`NetTrace::start`]; test thay bằng hàm không tạo phiên nào.
pub type OpenNet = fn() -> Result<NetTrace, String>;

/// Mọi nguồn số liệu thật. Nguồn nào không mở/đọc được thì ghi lý do, phần còn lại vẫn chạy (spec 6).
pub struct Sampler {
    counters: Result<CpuDiskCounters, String>,
    net: Option<NetTrace>,
    net_error: Option<String>,
    thermal: ThermalReader,
    temp: TempTracker,
    last_temp: TempState,
    last_temp_ms: u64,
    paths: PathCache,
    descriptions: HashMap<String, String>,
    prev: PrevTotals,
    prev_net: Option<(u64, u64)>,
    last: Instant,
    cpus: u32,
    rules: Arc<EssentialRules>,
    /// Lỗi của từng nguồn trong mẫu vừa lấy (CPU, RAM, mạng tổng, danh sách tiến trình) — nguyên văn.
    warnings: Vec<String>,
}

impl Sampler {
    pub fn open(rules: Arc<EssentialRules>) -> Sampler {
        Self::open_with(rules, NetTrace::start)
    }

    pub fn open_with(rules: Arc<EssentialRules>, open_net: OpenNet) -> Sampler {
        let (net, net_error) = match open_net() {
            Ok(t) => (Some(t), None),
            Err(e) => (None, Some(e)),
        };
        Sampler {
            counters: CpuDiskCounters::open().map_err(|e| e.to_string()),
            net,
            net_error,
            thermal: ThermalReader::new(),
            temp: TempTracker::default(),
            last_temp: TempState::Unavailable { code: "no_sensor".into(), detail: String::new() },
            last_temp_ms: 0,
            paths: PathCache::default(),
            descriptions: HashMap::new(),
            prev: PrevTotals::new(),
            prev_net: net_totals().ok(),
            last: Instant::now(),
            cpus: std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1),
            rules,
            warnings: Vec::new(),
        }
    }

    fn description(&mut self, path: &str) -> String {
        self.descriptions.entry(path.to_string()).or_insert_with(|| file_description(Path::new(path))).clone()
    }

    /// Lỗi từng nguồn của mẫu vừa lấy (xóa sau khi lấy). Tick vẫn được phát; đây là băng hổ phách.
    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    /// Lấy một mẫu. Lỗi chỉ khi không đọc được CPU lẫn danh sách tiến trình.
    pub fn tick(&mut self, interval_ms: u32) -> Result<(PerfTick, Vec<AppLoad>), String> {
        self.warnings.clear();
        let t_ms = now_ms();
        let dt_ms = self.last.elapsed().as_millis().max(1) as u64;
        self.last = Instant::now();
        let reading = self.counters.as_ref().map_err(|e| e.clone()).and_then(|c| c.read().map_err(|e| e.to_string()));
        let (cpu, disk, perf) = match &reading {
            Ok(r) => (r.cpu.unwrap_or(0.0), r.disk_active, r.perf),
            Err(_) => (0.0, None, None),
        };
        let mem = memory().map_err(|e| self.warnings.push(e.to_string())).ok();
        let net = net_totals().map_err(|e| self.warnings.push(e.to_string())).ok();
        let (net_up, net_down) = match (self.prev_net, net) {
            (Some((rx0, tx0)), Some((rx, tx))) => {
                let s = dt_ms as f64 / 1000.0;
                (Some(tx.saturating_sub(tx0) as f64 / s), Some(rx.saturating_sub(rx0) as f64 / s))
            }
            _ => (None, None),
        };
        self.prev_net = net;
        let procs = snapshot().map_err(|e| e.to_string());
        if let (Err(r), Err(p)) = (&reading, &procs) {
            return Err(format!("{r}; {p}"));
        }
        if let Err(e) = &reading {
            self.warnings.push(e.clone());
        }
        if let Err(e) = &procs {
            self.warnings.push(e.clone());
        }
        let per_pid_net = self.net.as_ref().map(|n| n.take());
        let mut samples = Vec::new();
        let mut alive = HashSet::new();
        for p in procs.unwrap_or_default() {
            alive.insert((p.pid, p.create_time));
            let path = self.paths.get(p.pid, p.create_time);
            let description = path.as_deref().map(|x| self.description(x)).unwrap_or_default();
            let net = per_pid_net.as_ref().map(|m| m.get(&p.pid).copied().unwrap_or((0, 0)));
            samples.push(ProcSample {
                pid: p.pid,
                parent_pid: p.parent_pid,
                name: p.name,
                path,
                description,
                create_time: p.create_time,
                private_ws: p.private_ws,
                cpu_100ns: p.cpu_100ns,
                io_bytes: p.io_bytes,
                net,
            });
        }
        self.paths.retain(&alive);
        // Tên thân thiện nhớ theo đường dẫn: bỏ exe không còn chạy để bộ nhớ đệm không phình mãi.
        let live_paths: HashSet<&str> = samples.iter().filter_map(|s| s.path.as_deref()).collect();
        self.descriptions.retain(|k, _| live_paths.contains(k.as_str()));
        let (apps, next) = aggregate(&self.prev, &samples, dt_ms, self.cpus, &self.rules);
        self.prev = next;
        if t_ms.saturating_sub(self.last_temp_ms) >= TEMP_EVERY_MS {
            self.last_temp = self.temp.push(t_ms, self.thermal.read());
            self.last_temp_ms = t_ms;
        }
        let loads = apps
            .iter()
            .map(|a| AppLoad { key: a.key.as_str().into(), name: a.name.as_str().into(), cpu: a.cpu, disk_bps: a.disk_bps })
            .collect();
        let sample = Sample {
            t_ms,
            dur_ms: dt_ms,
            cpu,
            ram_pct: mem.map(|m| m.load_pct as f32).unwrap_or(0.0),
            ram_used: mem.map(|m| m.total.saturating_sub(m.available)).unwrap_or(0),
            ram_total: mem.map(|m| m.total).unwrap_or(0),
            disk_active: disk,
            net_up_bps: net_up,
            net_down_bps: net_down,
            cpu_perf: perf,
        };
        let (perf_error, disk_error) = match (&self.counters, &reading) {
            (Err(e), _) | (Ok(_), Err(e)) => (Some(e.clone()), Some(e.clone())),
            (Ok(c), Ok(_)) => (c.perf_error.clone(), c.disk_error.clone()),
        };
        let tick = PerfTick {
            sample,
            apps,
            stutters: Vec::new(),
            throttled: None,
            perf_error,
            disk_error,
            temp: self.last_temp.clone(),
            net_app_error: self.net_error.clone(),
            interval_ms,
        };
        Ok((tick, loads))
    }

    /// Dừng phiên ETW `WinFreeUp-Net` (nếu đã mở). Bỏ `Sampler` mà không gọi cũng dừng (Drop của NetTrace).
    pub fn close(&mut self) {
        if let Some(n) = self.net.take() {
            n.stop();
        }
    }
}

pub type TickSink = Arc<dyn Fn(&PerfTick) + Send + Sync>;
pub type ErrorSink = Arc<dyn Fn(&str) + Send + Sync>;

type Sinks = Arc<Mutex<Option<(TickSink, ErrorSink)>>>;

/// Một luồng lấy mẫu với cờ dừng RIÊNG — luồng cũ không bao giờ đọc nhầm cờ của luồng mới.
struct Worker {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Luồng lấy mẫu chạy nền. `start` và `stop` giữ cùng một khóa suốt thao tác (kể cả lúc chờ luồng dừng),
/// nên `start → stop → start` từ nhiều luồng gọi vẫn chạy lần lượt, không bao giờ có hai luồng lấy mẫu.
/// `start` khi đang chạy chỉ đổi nơi nhận; `stop` chờ luồng dừng hẳn, sau đó không còn mẫu nào được phát.
pub struct PerfMonitor {
    history: Arc<Mutex<History>>,
    last_apps: Arc<Mutex<Vec<AppRow>>>,
    sinks: Sinks,
    worker: Mutex<Option<Worker>>,
    open_net: OpenNet,
}

impl Default for PerfMonitor {
    fn default() -> Self {
        PerfMonitor {
            history: Arc::default(),
            last_apps: Arc::default(),
            sinks: Arc::default(),
            worker: Mutex::new(None),
            open_net: NetTrace::start,
        }
    }
}

impl PerfMonitor {
    /// Như `default` nhưng mở phiên ETW bằng `open_net` — test dùng để không đụng phiên `WinFreeUp-Net` thật.
    #[cfg(test)]
    fn with_net(open_net: OpenNet) -> Self {
        let mut m = PerfMonitor::default();
        m.open_net = open_net;
        m
    }

    /// Bắt đầu lấy mẫu; trả lịch sử sẵn có (mẫu cũ của lần xem trước, nếu còn trong 5 phút).
    pub fn start(&self, rules: Arc<EssentialRules>, on_tick: TickSink, on_error: ErrorSink) -> Vec<Sample> {
        let mut worker = lock(&self.worker);
        *lock(&self.sinks) = Some((on_tick, on_error));
        let existing = lock(&self.history).samples();
        if worker.as_ref().is_some_and(|w| !w.handle.is_finished()) {
            return existing;
        }
        if let Some(w) = worker.take() {
            // Luồng cũ đã tự kết thúc (vd hoảng) — thu dọn trước khi tạo luồng mới.
            let _ = w.handle.join();
        }
        let stop = Arc::new(AtomicBool::new(false));
        let (history, last_apps, sinks, flag, open_net) =
            (self.history.clone(), self.last_apps.clone(), self.sinks.clone(), stop.clone(), self.open_net);
        let handle = std::thread::spawn(move || run(rules, open_net, &flag, &history, &last_apps, &sinks));
        *worker = Some(Worker { stop, handle });
        existing
    }

    pub fn stop(&self) {
        let mut worker = lock(&self.worker);
        if let Some(w) = worker.take() {
            w.stop.store(true, Ordering::SeqCst);
            let _ = w.handle.join();
        }
        *lock(&self.sinks) = None;
    }

    pub fn is_running(&self) -> bool {
        lock(&self.worker).as_ref().is_some_and(|w| !w.handle.is_finished())
    }

    /// Bảng ứng dụng của mẫu gần nhất (để tìm tiến trình khi người dùng bấm Kết thúc app).
    pub fn last_apps(&self) -> Vec<AppRow> {
        lock(&self.last_apps).clone()
    }
}

impl Drop for PerfMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Thân luồng lấy mẫu. Nhịp tính từ đầu mẫu trước, không cộng dồn thời gian lấy mẫu vào chu kỳ.
fn run(
    rules: Arc<EssentialRules>,
    open_net: OpenNet,
    stop: &AtomicBool,
    history: &Mutex<History>,
    last_apps: &Mutex<Vec<AppRow>>,
    sinks: &Mutex<Option<(TickSink, ErrorSink)>>,
) {
    let current = || if stop.load(Ordering::SeqCst) { None } else { lock(sinks).clone() };
    // Sampler mở phiên ETW; bị bỏ (kể cả khi hoảng) thì Drop của NetTrace dừng phiên.
    let mut sampler = Sampler::open_with(rules, open_net);
    let mut interval = BASE_INTERVAL_MS;
    // Mẫu đầu chỉ để mồi bộ đếm tốc độ. Nó đắt (mở từng tiến trình lấy đường dẫn, đọc tên thân thiện) nên
    // không tính vào chi phí — tính vào thì lần đo đầu luôn vượt 2% và chu kỳ bị giãn vĩnh viễn.
    let _ = sampler.tick(interval);
    let mut own_prev = own_cpu_100ns();
    let mut wall_prev = Instant::now();
    let mut tick_start = Instant::now();
    while !stop.load(Ordering::SeqCst) {
        let wake = tick_start + Duration::from_millis(u64::from(interval));
        while !stop.load(Ordering::SeqCst) && Instant::now() < wake {
            std::thread::sleep(Duration::from_millis(50));
        }
        if stop.load(Ordering::SeqCst) {
            break;
        }
        tick_start = Instant::now();
        let result = sampler.tick(interval);
        let warnings = sampler.take_warnings();
        match result {
            Ok((mut tick, loads)) => {
                {
                    let mut h = lock(history);
                    h.push(tick.sample.clone(), loads);
                    tick.stutters = h.stutters();
                    tick.throttled = h.throttled_now();
                }
                lock(last_apps).clone_from(&tick.apps);
                if let Some((on_tick, on_error)) = current() {
                    on_tick(&tick);
                    for w in &warnings {
                        on_error(w);
                    }
                }
            }
            Err(e) => {
                if let Some((_, on_error)) = current() {
                    on_error(&e);
                }
            }
        }
        let own = own_cpu_100ns();
        let wall_ms = wall_prev.elapsed().as_millis().max(1) as f64;
        let pct = (own.saturating_sub(own_prev) as f64 / 10_000.0) / (wall_ms * f64::from(sampler.cpus)) * 100.0;
        interval = next_interval_ms(interval, pct as f32);
        own_prev = own;
        wall_prev = Instant::now();
    }
    sampler.close();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(t_ms: u64, cpu: f32, disk: f32) -> Sample {
        Sample {
            t_ms,
            dur_ms: 1000,
            cpu,
            ram_pct: 50.0,
            ram_used: 1,
            ram_total: 2,
            disk_active: Some(disk),
            net_up_bps: None,
            net_down_bps: None,
            cpu_perf: Some(100.0),
        }
    }

    fn load(key: &str, cpu: f32, disk: f64) -> AppLoad {
        AppLoad { key: key.into(), name: key.into(), cpu, disk_bps: disk }
    }

    /// Test không mở phiên ETW nào (kể cả `WinFreeUp-Net` của app có thể đang chạy trên máy dev).
    fn no_etw() -> Result<NetTrace, String> {
        Err("ETW tắt trong test".into())
    }

    fn rules() -> Arc<EssentialRules> {
        Arc::new(EssentialRules::new(&crate::util::system_dir().unwrap(), std::process::id()))
    }

    type Ticks = Arc<Mutex<Vec<PerfTick>>>;

    fn collector() -> (Ticks, TickSink) {
        let ticks: Ticks = Arc::default();
        let sink = ticks.clone();
        (ticks, Arc::new(move |t: &PerfTick| sink.lock().unwrap().push(t.clone())))
    }

    fn wait_for(ticks: &Ticks, n: usize) {
        // Mở nguồn (PDH, WMI) + mẫu mồi mất vài giây, lâu hơn khi máy đang bận: chờ tối đa 30 giây.
        let deadline = Instant::now() + Duration::from_secs(30);
        while ticks.lock().unwrap().len() < n && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    #[test]
    fn history_keeps_only_the_last_five_minutes() {
        let mut h = History::default();
        for i in 0..400u64 {
            h.push(sample(1_000_000 + i * 1000, 1.0, 1.0), Vec::new());
        }
        let s = h.samples();
        assert_eq!(s.last().unwrap().t_ms - s.first().unwrap().t_ms, HISTORY_MS);
        assert_eq!(h.len(), 301);
    }

    #[test]
    fn history_stays_bounded_when_the_clock_jumps_back() {
        let mut h = History::default();
        for i in 0..100u64 {
            h.push(sample(10_000_000 + i * 1000, 1.0, 1.0), Vec::new());
        }
        // Đồng hồ bị chỉnh lùi một giờ: mẫu cũ mang giờ «tương lai», cắt theo giờ không bỏ được.
        for i in 0..400u64 {
            h.push(sample(10_000_000 - 3_600_000 + i * 1000, 1.0, 1.0), Vec::new());
        }
        assert_eq!(h.len(), MAX_FRAMES);
    }

    #[test]
    fn stutter_lists_top_apps_for_the_metric_that_spiked_only() {
        let mut h = History::default();
        let t0 = 1_000_000;
        h.push(sample(t0, 10.0, 5.0), vec![load("idle", 1.0, 0.0)]);
        for i in 1..=3 {
            h.push(sample(t0 + i * 1000, 97.0, 5.0), vec![load("game", 80.0, 1e6), load("av", 15.0, 9e6)]);
        }
        h.push(sample(t0 + 4000, 10.0, 5.0), vec![]);
        let s = h.stutters();
        assert_eq!(s.len(), 1);
        assert!(s[0].cpu && !s[0].disk);
        assert_eq!(s[0].top_cpu[0].key, "game");
        assert!(s[0].top_disk.is_empty());
        assert!(!s[0].throttled);
    }

    #[test]
    fn stutter_during_throttling_is_flagged() {
        let mut h = History::default();
        for i in 0..12u64 {
            let mut s = sample(2_000_000 + i * 1000, 95.0, 5.0);
            s.cpu_perf = Some(50.0);
            h.push(s, vec![load("x", 90.0, 0.0)]);
        }
        assert!(h.stutters()[0].throttled);
        assert_eq!(h.throttled_now(), Some(true));
    }

    #[test]
    fn real_monitor_emits_ticks_with_apps_and_stops() {
        let m = PerfMonitor::with_net(no_etw);
        let (ticks, sink) = collector();
        let before = m.start(rules(), sink, Arc::new(|_: &str| {}));
        assert!(before.is_empty());
        assert!(m.is_running());
        wait_for(&ticks, 2);
        m.stop();
        assert!(!m.is_running());
        let got = ticks.lock().unwrap();
        assert!(!got.is_empty(), "ít nhất một mẫu");
        let t = &got[0];
        assert!((0.0..=100.0).contains(&t.sample.cpu));
        assert!(t.sample.ram_total > 0);
        assert!(t.apps.len() > 5);
        assert!(t.apps.iter().any(|a| a.essential), "svchost/System luôn có");
        assert_eq!(t.net_app_error.as_deref(), Some("ETW tắt trong test"), "ETW hỏng ⇒ chỉ cột Mạng theo app mất");
        assert!(!m.last_apps().is_empty());
        let n = got.len();
        drop(got);
        std::thread::sleep(Duration::from_millis(1300));
        assert_eq!(ticks.lock().unwrap().len(), n, "đã dừng thì không còn mẫu");
        // Mở lại: lịch sử của lần xem trước được trả về để vẽ ngay. Có thể hơn một mẫu: mẫu lấy xong đúng
        // lúc `stop` vẫn vào lịch sử, chỉ không được phát nữa.
        let (ticks2, sink2) = collector();
        let history = m.start(rules(), sink2, Arc::new(|_: &str| {})).len();
        assert!((n..=n + 1).contains(&history), "lịch sử {history}, đã phát {n}");
        wait_for(&ticks2, 1);
        m.stop();
        assert!(!ticks2.lock().unwrap().is_empty());
    }

    #[test]
    fn start_stop_start_from_many_threads_never_leaves_two_samplers() {
        let m = Arc::new(PerfMonitor::with_net(no_etw));
        let (ticks, sink) = collector();
        let threads: Vec<_> = (0..3)
            .map(|_| {
                let (m, sink) = (m.clone(), sink.clone());
                std::thread::spawn(move || {
                    for _ in 0..2 {
                        m.start(rules(), sink.clone(), Arc::new(|_: &str| {}));
                        m.stop();
                        m.start(rules(), sink.clone(), Arc::new(|_: &str| {}));
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert!(m.is_running());
        let before = ticks.lock().unwrap().len();
        wait_for(&ticks, before + 3);
        m.stop();
        assert!(!m.is_running());
        let got = ticks.lock().unwrap();
        // Hai luồng lấy mẫu cùng lúc ⇒ hai mẫu gần như trùng giờ. Một luồng ⇒ các mẫu cách nhau ≈ 1 giây.
        for w in got[before..].windows(2) {
            let gap = w[1].sample.t_ms.saturating_sub(w[0].sample.t_ms);
            assert!(gap >= 500, "hai mẫu chỉ cách {gap} ms — có hai luồng lấy mẫu");
        }
        let n = got.len();
        drop(got);
        std::thread::sleep(Duration::from_millis(1300));
        assert_eq!(ticks.lock().unwrap().len(), n, "đã dừng thì không còn mẫu");
    }

    #[test]
    fn a_second_start_while_running_only_moves_the_sink() {
        let m = PerfMonitor::with_net(no_etw);
        let (old, old_sink) = collector();
        m.start(rules(), old_sink, Arc::new(|_: &str| {}));
        let (new, new_sink) = collector();
        m.start(rules(), new_sink, Arc::new(|_: &str| {}));
        wait_for(&new, 2);
        m.stop();
        assert!(old.lock().unwrap().is_empty(), "nơi nhận cũ không còn nhận mẫu");
        assert!(new.lock().unwrap().len() >= 2);
    }
}
