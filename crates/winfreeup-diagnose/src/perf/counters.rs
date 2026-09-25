//! Số liệu toàn máy: CPU, Hoạt động đĩa, % hiệu năng CPU (PDH), RAM (GlobalMemoryStatusEx),
//! tổng mạng (GetIfTable2). Bộ đếm PDH thêm bằng tên TIẾNG ANH (`PdhAddEnglishCounterW`) để chạy được
//! trên Windows tiếng Việt, nơi tên bộ đếm hiển thị đã được dịch.
use serde::Serialize;
use windows_sys::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows_sys::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue, PDH_CSTATUS_NEW_DATA,
    PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
};
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use winfreeup_core::{CoreError, Result};

use crate::util::wide;

/// Không cắt giá trị ở 100 (tiện ích và hiệu năng CPU vượt 100% khi turbo).
const PDH_FMT_NOCAP100: u32 = 0x8000;
const IF_TYPE_SOFTWARE_LOOPBACK: u32 = 24;
const IF_OPER_STATUS_UP: i32 = 1;

pub const CPU_COUNTERS: [&str; 2] = [r"\Processor Information(_Total)\% Processor Utility", r"\Processor(_Total)\% Processor Time"];
pub const DISK_IDLE_COUNTER: &str = r"\PhysicalDisk(_Total)\% Idle Time";
pub const PERF_COUNTER: &str = r"\Processor Information(_Total)\% Processor Performance";

fn pdh_err(what: &str, code: u32) -> CoreError {
    CoreError::System(format!("PDH {what}: 0x{code:08X}"))
}

pub struct Pdh {
    query: PDH_HQUERY,
}

/// Một bộ đếm trong truy vấn `Pdh` (bọc con trỏ thô để hàm công khai không nhận con trỏ).
#[derive(Debug, Clone, Copy)]
pub struct Counter(PDH_HCOUNTER);

// SAFETY: PDH_HQUERY là con trỏ thô tới handle PDH, không gắn với luồng tạo ra nó; `Pdh` không `Sync`
// nên chỉ một luồng dùng truy vấn tại một thời điểm (luồng lấy mẫu).
unsafe impl Send for Pdh {}

impl Pdh {
    pub fn open() -> Result<Pdh> {
        let mut q: PDH_HQUERY = std::ptr::null_mut();
        // SAFETY: nguồn dữ liệu null = bộ đếm thời gian thực; `q` là biến cục bộ hợp lệ để PDH ghi handle vào.
        let rc = unsafe { windows_sys::Win32::System::Performance::PdhOpenQueryW(std::ptr::null(), 0, &mut q) };
        if rc != 0 {
            return Err(pdh_err("open", rc));
        }
        Ok(Pdh { query: q })
    }

    pub fn add(&self, path: &str) -> Result<Counter> {
        let w = wide(path);
        let mut c: PDH_HCOUNTER = std::ptr::null_mut();
        // SAFETY: `self.query` là handle mở bởi PdhOpenQueryW, sống tới Drop; `w` là chuỗi UTF-16 kết thúc NUL
        // (util::wide) còn sống suốt lời gọi; `c` là biến cục bộ hợp lệ để ghi handle bộ đếm.
        let rc = unsafe { PdhAddEnglishCounterW(self.query, w.as_ptr(), 0, &mut c) };
        if rc != 0 {
            return Err(pdh_err(path, rc));
        }
        Ok(Counter(c))
    }

    pub fn collect(&self) -> Result<()> {
        // SAFETY: `self.query` là handle hợp lệ (mở ở `open`, chỉ đóng ở Drop).
        let rc = unsafe { PdhCollectQueryData(self.query) };
        if rc != 0 {
            return Err(pdh_err("collect", rc));
        }
        Ok(())
    }

    /// Giá trị đã định dạng; `None` khi chưa đủ hai lần thu (bộ đếm tốc độ) hoặc dữ liệu không hợp lệ.
    pub fn value(&self, c: Counter) -> Option<f64> {
        let mut v = PDH_FMT_COUNTERVALUE::default();
        // SAFETY: `c.0` là handle bộ đếm thêm vào truy vấn này (còn sống vì truy vấn chưa đóng); lpdwType null được phép;
        // `v` là biến cục bộ đủ kích thước PDH_FMT_COUNTERVALUE.
        let rc = unsafe { PdhGetFormattedCounterValue(c.0, PDH_FMT_DOUBLE | PDH_FMT_NOCAP100, std::ptr::null_mut(), &mut v) };
        if rc != 0 || !(v.CStatus == PDH_CSTATUS_VALID_DATA || v.CStatus == PDH_CSTATUS_NEW_DATA) {
            return None;
        }
        // SAFETY: đã yêu cầu PDH_FMT_DOUBLE và lời gọi thành công với dữ liệu hợp lệ ⇒ trường đang dùng là doubleValue.
        Some(unsafe { v.Anonymous.doubleValue })
    }
}

impl Drop for Pdh {
    fn drop(&mut self) {
        // SAFETY: đóng đúng một lần handle mở ở `open`; không còn dùng sau Drop (Counter chỉ đọc qua &Pdh).
        unsafe { PdhCloseQuery(self.query) };
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuDiskReading {
    pub cpu: Option<f32>,
    pub disk_active: Option<f32>,
    pub perf: Option<f32>,
}

/// Ba bộ đếm của mục Hiệu năng. CPU bắt buộc; đĩa và % hiệu năng thiếu thì `None` (spec 5.4: ẩn chỉ báo).
pub struct CpuDiskCounters {
    pdh: Pdh,
    cpu: Counter,
    disk_idle: Option<Counter>,
    perf: Option<Counter>,
    /// Lý do bộ đếm hiệu năng CPU không có (nguyên văn), để ghi «Không đo được».
    pub perf_error: Option<String>,
    pub disk_error: Option<String>,
}

// SAFETY: chỉ chứa `Pdh` (Send, xem trên) và handle bộ đếm thuộc chính truy vấn đó; không `Sync`.
unsafe impl Send for CpuDiskCounters {}

impl CpuDiskCounters {
    pub fn open() -> Result<Self> {
        let pdh = Pdh::open()?;
        let cpu = pdh.add(CPU_COUNTERS[0]).or_else(|_| pdh.add(CPU_COUNTERS[1]))?;
        let (disk_idle, disk_error) = match pdh.add(DISK_IDLE_COUNTER) {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(e.to_string())),
        };
        let (perf, perf_error) = match pdh.add(PERF_COUNTER) {
            Ok(c) => (Some(c), None),
            Err(e) => (None, Some(e.to_string())),
        };
        pdh.collect()?;
        Ok(CpuDiskCounters { pdh, cpu, disk_idle, perf, perf_error, disk_error })
    }

    pub fn read(&self) -> Result<CpuDiskReading> {
        self.pdh.collect()?;
        Ok(CpuDiskReading {
            cpu: self.pdh.value(self.cpu).map(|v| v.clamp(0.0, 100.0) as f32),
            disk_active: self.disk_idle.and_then(|c| self.pdh.value(c)).map(|idle| (100.0 - idle).clamp(0.0, 100.0) as f32),
            perf: self.perf.and_then(|c| self.pdh.value(c)).map(|v| v.max(0.0) as f32),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MemoryStatus {
    /// % RAM đang dùng (Windows tự tính).
    pub load_pct: u32,
    pub total: u64,
    pub available: u64,
    /// Commit charge đang dùng và giới hạn (RAM + pagefile).
    pub commit_used: u64,
    pub commit_limit: u64,
}

pub fn memory() -> Result<MemoryStatus> {
    let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
    // SAFETY: `m` là biến cục bộ với dwLength đã đặt đúng kích thước như API yêu cầu.
    if unsafe { GlobalMemoryStatusEx(&mut m) } == 0 {
        return Err(CoreError::System(format!("GlobalMemoryStatusEx: {}", std::io::Error::last_os_error())));
    }
    Ok(MemoryStatus {
        load_pct: m.dwMemoryLoad,
        total: m.ullTotalPhys,
        available: m.ullAvailPhys,
        commit_used: m.ullTotalPageFile.saturating_sub(m.ullAvailPageFile),
        commit_limit: m.ullTotalPageFile,
    })
}

/// Tổng byte (nhận, gửi) từ lúc khởi động qua các card mạng thật đang bật.
/// Bỏ loopback và card ảo (Hyper-V, VPN…) để lưu lượng không bị đếm hai lần.
pub fn net_totals() -> Result<(u64, u64)> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    // SAFETY: `table` là biến cục bộ để API ghi con trỏ tới bảng do nó cấp phát (giải phóng bằng FreeMibTable bên dưới).
    let rc = unsafe { GetIfTable2(&mut table) };
    if rc != 0 {
        return Err(CoreError::System(format!("GetIfTable2: {}", std::io::Error::from_raw_os_error(rc as i32))));
    }
    let (mut rx, mut tx) = (0u64, 0u64);
    // SAFETY: GetIfTable2 thành công ⇒ `table` trỏ tới MIB_IF_TABLE2 hợp lệ có NumEntries dòng liền nhau bắt đầu ở
    // `Table`; lát cắt chỉ dùng trước FreeMibTable, và bảng được giải phóng đúng một lần.
    unsafe {
        let n = (*table).NumEntries as usize;
        let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), n);
        for r in rows {
            let hardware = r.InterfaceAndOperStatusFlags._bitfield & 1 != 0;
            if hardware && r.Type != IF_TYPE_SOFTWARE_LOOPBACK && r.OperStatus == IF_OPER_STATUS_UP {
                // Tổng tích lũy 64-bit của nhiều card: chặn trần thay vì tràn (bản debug sẽ panic khi tràn).
                rx = rx.saturating_add(r.InOctets);
                tx = tx.saturating_add(r.OutOctets);
            }
        }
        FreeMibTable(table.cast());
    }
    Ok((rx, tx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_and_disk_counters_give_percentages_after_two_reads() {
        let c = CpuDiskCounters::open().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let r = c.read().unwrap();
        let cpu = r.cpu.expect("CPU luôn có");
        assert!((0.0..=100.0).contains(&cpu), "{cpu}");
        if let Some(d) = r.disk_active {
            assert!((0.0..=100.0).contains(&d), "{d}");
        } else {
            assert!(c.disk_error.is_some());
        }
        if let Some(p) = r.perf {
            assert!(p > 0.0 && p < 400.0, "{p}");
        }
    }

    #[test]
    fn unknown_counter_is_a_clean_error() {
        let pdh = Pdh::open().unwrap();
        let e = pdh.add(r"\Khong Co(_Total)\% Gi Ca").unwrap_err().to_string();
        assert!(e.starts_with("PDH "), "{e}");
    }

    #[test]
    fn memory_numbers_are_consistent() {
        let m = memory().unwrap();
        assert!(m.load_pct <= 100);
        assert!(m.available <= m.total);
        assert!(m.commit_used <= m.commit_limit && m.commit_limit >= m.total / 2);
    }

    #[test]
    fn network_totals_do_not_go_backwards() {
        let (rx1, tx1) = net_totals().unwrap();
        let (rx2, tx2) = net_totals().unwrap();
        assert!(rx2 >= rx1 && tx2 >= tx1);
    }
}
