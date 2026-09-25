//! Ảnh chụp mọi tiến trình trong MỘT lời gọi `NtQuerySystemInformation(SystemProcessInformation)` —
//! cách Task Manager làm: có RAM riêng (private working set), thời gian CPU, byte vào/ra, không phải mở
//! từng tiến trình. Đường dẫn exe mới cần mở tiến trình (quyền tối thiểu), nên được nhớ theo (pid, giờ tạo).
use std::collections::{HashMap, HashSet};

use windows_sys::Wdk::System::SystemInformation::{NtQuerySystemInformation, SystemProcessInformation};
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, TerminateProcess, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};
use winfreeup_core::{CoreError, Result};

use super::apps::{EssentialRules, ProcTree};

const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

#[repr(C)]
struct UnicodeString {
    length: u16,
    maximum_length: u16,
    buffer: *const u16,
}

/// Bố cục đầy đủ của SYSTEM_PROCESS_INFORMATION (x64). Bản tài liệu công khai giấu các trường này dưới
/// tên `Reserved…` nhưng vị trí không đổi từ Windows 7; test `layout_offsets_match_the_documented_ones`
/// giữ lại hai mốc công khai (`ImageName` 0x38, `UniqueProcessId` 0x50).
#[repr(C)]
struct SpiEntry {
    next_entry_offset: u32,
    number_of_threads: u32,
    working_set_private_size: i64,
    hard_fault_count: u32,
    number_of_threads_high_watermark: u32,
    cycle_time: u64,
    create_time: i64,
    user_time: i64,
    kernel_time: i64,
    image_name: UnicodeString,
    base_priority: i32,
    unique_process_id: usize,
    inherited_from_unique_process_id: usize,
    handle_count: u32,
    session_id: u32,
    unique_process_key: usize,
    peak_virtual_size: usize,
    virtual_size: usize,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
    private_page_count: usize,
    read_operation_count: i64,
    write_operation_count: i64,
    other_operation_count: i64,
    read_transfer_count: i64,
    write_transfer_count: i64,
    other_transfer_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcInfo {
    pub pid: u32,
    pub parent_pid: u32,
    /// Tên ảnh (vd `chrome.exe`; `System`, `Registry` với tiến trình nhân).
    pub name: String,
    /// FILETIME lúc tạo — cùng pid để phân biệt tiến trình đã chết và pid bị dùng lại.
    pub create_time: i64,
    /// Byte RAM riêng (private working set) — cột RAM của spec.
    pub private_ws: u64,
    /// Tổng thời gian CPU (user + kernel), đơn vị 100 ns.
    pub cpu_100ns: u64,
    /// Tổng byte đọc + ghi (file, thiết bị) từ lúc tiến trình chạy.
    pub io_bytes: u64,
}

/// Mọi tiến trình đang chạy, bỏ tiến trình Idle (pid 0).
pub fn snapshot() -> Result<Vec<ProcInfo>> {
    let mut buf: Vec<u64> = vec![0; 256 * 1024];
    let mut tries = 0;
    loop {
        let mut needed = 0u32;
        // SAFETY: buf có đúng buf.len() * 8 byte khả ghi, căn 8 byte.
        let status = unsafe {
            NtQuerySystemInformation(SystemProcessInformation, buf.as_mut_ptr().cast(), (buf.len() * 8) as u32, &mut needed)
        };
        if status == STATUS_INFO_LENGTH_MISMATCH && tries < 8 {
            tries += 1;
            buf.resize((needed as usize / 8) + 16 * 1024, 0);
            continue;
        }
        if status < 0 {
            return Err(CoreError::System(format!("NtQuerySystemInformation failed: 0x{:08X}", status as u32)));
        }
        break;
    }
    parse_records(&buf)
}

/// Đọc chuỗi bản ghi SYSTEM_PROCESS_INFORMATION trong `buf`, bỏ Idle (pid 0). Bản ghi nào nằm vắt ra ngoài
/// bộ đệm (hoặc lệch căn) ⇒ lỗi, không cắt im lặng rồi trả thiếu tiến trình.
fn parse_records(buf: &[u64]) -> Result<Vec<ProcInfo>> {
    let base = buf.as_ptr() as *const u8;
    let end = buf.len() * 8;
    let mut out = Vec::with_capacity(512);
    let mut off = 0usize;
    loop {
        if !off.is_multiple_of(8) || off.checked_add(std::mem::size_of::<SpiEntry>()).is_none_or(|e| e > end) {
            return Err(CoreError::System(format!("SystemProcessInformation: record at {off} runs past {end} bytes")));
        }
        // SAFETY: bản ghi nằm trọn trong buf và căn 8 byte (đã kiểm ở trên); mọi mẫu bit đều hợp lệ cho SpiEntry.
        let e = unsafe { &*(base.add(off) as *const SpiEntry) };
        let pid = e.unique_process_id as u32;
        if pid != 0 {
            let chars = e.image_name.length as usize / 2;
            let start = e.image_name.buffer as usize;
            let inside = start >= base as usize && start + chars * 2 <= base as usize + end;
            let name = if e.image_name.buffer.is_null() || !inside {
                String::new()
            } else {
                // SAFETY: chuỗi tên nằm trong buf (đã kiểm ở trên), dài `chars` ký tự UTF-16.
                String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(e.image_name.buffer, chars) })
            };
            out.push(ProcInfo {
                pid,
                parent_pid: e.inherited_from_unique_process_id as u32,
                name: if pid == 4 && name.is_empty() { "System".into() } else { name },
                create_time: e.create_time,
                private_ws: e.working_set_private_size.max(0) as u64,
                cpu_100ns: e.user_time.saturating_add(e.kernel_time).max(0) as u64,
                io_bytes: e.read_transfer_count.saturating_add(e.write_transfer_count).max(0) as u64,
            });
        }
        if e.next_entry_offset == 0 {
            return Ok(out);
        }
        off += e.next_entry_offset as usize;
    }
}

struct Handle(HANDLE);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: handle do OpenProcess trả về (khác null), đóng đúng một lần.
        unsafe { CloseHandle(self.0) };
    }
}

fn open(pid: u32, access: u32) -> Option<Handle> {
    // SAFETY: gọi API không có con trỏ; kết quả null được xử lý.
    let h = unsafe { OpenProcess(access, 0, pid) };
    (!h.is_null()).then_some(Handle(h))
}

fn creation_time(h: &Handle) -> Option<i64> {
    let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: h là handle tiến trình còn mở; bốn FILETIME là biến cục bộ khả ghi.
    let ok = unsafe { GetProcessTimes(h.0, &mut c, &mut e, &mut k, &mut u) };
    (ok != 0).then_some(((c.dwHighDateTime as i64) << 32) | c.dwLowDateTime as i64)
}

/// Đường dẫn exe của tiến trình mà handle trỏ tới (handle cần `PROCESS_QUERY_LIMITED_INFORMATION`).
fn image_path(h: &Handle) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: buf có len phần tử khả ghi; API ghi lại độ dài thật (không gồm NUL) vào len.
    let ok = unsafe { QueryFullProcessImageNameW(h.0, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) };
    (ok != 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

/// Đường dẫn exe đầy đủ; `None` khi không mở được (tiến trình nhân, đã thoát…).
pub fn exe_path(pid: u32) -> Option<String> {
    image_path(&open(pid, PROCESS_QUERY_LIMITED_INFORMATION)?)
}

/// Nhớ đường dẫn exe theo (pid, giờ tạo) — không mở lại tiến trình mỗi giây.
#[derive(Default)]
pub struct PathCache {
    map: HashMap<(u32, i64), Option<String>>,
}

impl PathCache {
    pub fn get(&mut self, pid: u32, create_time: i64) -> Option<String> {
        self.map.entry((pid, create_time)).or_insert_with(|| exe_path(pid)).clone()
    }

    /// Bỏ các tiến trình đã thoát.
    pub fn retain(&mut self, alive: &HashSet<(u32, i64)>) {
        self.map.retain(|k, _| alive.contains(k));
    }
}

/// Kết thúc một tiến trình, chỉ khi nó vẫn là đúng tiến trình đã thấy (pid không bị dùng lại) và không
/// thiết yếu theo luật mặc định: System32 thật (qua API) và chính WinFreeUp (pid hiện tại).
pub fn kill(pid: u32, create_time: i64) -> Result<()> {
    let system32 = crate::util::system_dir().map_err(CoreError::System)?;
    kill_with(pid, create_time, &EssentialRules::new(&system32, std::process::id()))
}

/// Như [`kill`] với luật thiết yếu tùy chọn (gồm mọi con cháu của WinFreeUp, lấy từ ảnh chụp mới). Mọi bước kiểm và lệnh kết thúc dùng CÙNG MỘT handle: khi
/// handle còn mở, hệ điều hành không cấp lại pid đó, nên giờ tạo, đường dẫn và tên đọc được đều là của
/// đúng tiến trình sắp bị kết thúc. Luật thiết yếu được kiểm lại tại đây (không tin bảng app đã cũ).
/// Lỗi `essential` là mã, không phải chữ hiển thị.
pub fn kill_with(pid: u32, create_time: i64, rules: &EssentialRules) -> Result<()> {
    let h = open(pid, PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION)
        .ok_or_else(|| CoreError::System(format!("OpenProcess({pid}): {}", std::io::Error::last_os_error())))?;
    if creation_time(&h) != Some(create_time) {
        return Err(CoreError::System(format!("process {pid} has already exited")));
    }
    // Handle đang giữ ⇒ bản ghi cùng (pid, giờ tạo) trong ảnh chụp chính là tiến trình này.
    let all = snapshot()?;
    let info = all
        .iter()
        .find(|p| p.pid == pid && p.create_time == create_time)
        .ok_or_else(|| CoreError::System(format!("process {pid} has already exited")))?;
    let tree: ProcTree = all.iter().map(|p| (p.pid, (p.parent_pid, p.create_time))).collect();
    let path = image_path(&h);
    if rules.is_essential_in(pid, info.parent_pid, create_time, &info.name, path.as_deref(), &tree) {
        return Err(CoreError::System("essential".into()));
    }
    // SAFETY: h là handle có quyền PROCESS_TERMINATE, còn mở.
    if unsafe { TerminateProcess(h.0, 1) } == 0 {
        return Err(CoreError::System(format!("TerminateProcess({pid}): {}", std::io::Error::last_os_error())));
    }
    Ok(())
}

/// Tổng thời gian CPU của chính WinFreeUp (100 ns) — để giữ chi phí dưới 2%.
pub fn own_cpu_100ns() -> u64 {
    let (mut c, mut e, mut k, mut u) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
    // SAFETY: GetCurrentProcess là pseudo-handle luôn hợp lệ; bốn FILETIME là biến cục bộ khả ghi.
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) };
    if ok == 0 {
        return 0;
    }
    let ft = |f: FILETIME| ((f.dwHighDateTime as u64) << 32) | f.dwLowDateTime as u64;
    ft(k) + ft(u)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::windows::process::CommandExt;

    #[test]
    fn layout_offsets_match_the_documented_ones() {
        assert_eq!(std::mem::offset_of!(SpiEntry, image_name), 0x38);
        assert_eq!(std::mem::offset_of!(SpiEntry, unique_process_id), 0x50);
        assert_eq!(std::mem::offset_of!(SpiEntry, read_transfer_count), 0xE8);
        assert_eq!(std::mem::size_of::<SpiEntry>(), 0x100);
    }

    #[test]
    fn snapshot_contains_this_test_process_with_sane_numbers() {
        let me = std::process::id();
        let all = snapshot().unwrap();
        assert!(all.len() > 10);
        assert!(all.iter().any(|p| p.pid == 4 && p.name == "System"));
        let p = all.iter().find(|p| p.pid == me).expect("tiến trình test");
        let exe = std::env::current_exe().unwrap();
        assert!(p.name.eq_ignore_ascii_case(&exe.file_name().unwrap().to_string_lossy()));
        assert!(p.private_ws > 0);
        assert!(p.create_time > 0);
        assert_eq!(exe_path(me).unwrap().to_lowercase(), exe.display().to_string().to_lowercase());
    }

    #[test]
    fn io_counter_grows_when_we_write() {
        let me = std::process::id();
        let before = snapshot().unwrap().into_iter().find(|p| p.pid == me).unwrap().io_bytes;
        let t = tempfile::tempdir().unwrap();
        std::fs::write(t.path().join("x.bin"), vec![7u8; 2_000_000]).unwrap();
        let after = snapshot().unwrap().into_iter().find(|p| p.pid == me).unwrap().io_bytes;
        assert!(after >= before + 2_000_000, "{before} -> {after}");
    }

    /// Test hỏng giữa chừng vẫn dọn tiến trình con do chính nó tạo.
    struct KillOnDrop(std::process::Child);

    impl Drop for KillOnDrop {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn records_running_past_the_buffer_are_an_error() {
        let rec = std::mem::size_of::<SpiEntry>();
        let mut buf = vec![0u64; rec / 8 + 4];
        // Bản ghi 1 hợp lệ, trỏ sang bản ghi 2 nằm vắt ra ngoài bộ đệm.
        buf[0] = 64; // next_entry_offset = 64, number_of_threads = 0
        assert!(parse_records(&buf).is_err());
        buf[0] = 0;
        assert!(parse_records(&buf).unwrap().is_empty(), "pid 0 (Idle) bị bỏ");
        assert!(parse_records(&buf[..rec / 8 - 1]).is_err(), "bộ đệm nhỏ hơn một bản ghi");
    }

    #[test]
    fn kill_ends_our_own_child_and_refuses_a_stale_create_time() {
        let mut child = KillOnDrop(
            std::process::Command::new("ping")
                .args(["-n", "60", "127.0.0.1"])
                .creation_flags(0x0800_0000)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
        let pid = child.0.id();
        let info = snapshot().unwrap().into_iter().find(|p| p.pid == pid).unwrap();
        assert!(kill(pid, info.create_time + 1).is_err(), "giờ tạo khác ⇒ không phải tiến trình đã thấy");
        // Luật mặc định coi tiến trình con của chính mình (như WebView2 của WinFreeUp) là thiết yếu.
        let refused = kill(pid, info.create_time).unwrap_err();
        assert_eq!(refused.to_string(), "essential");
        assert!(child.0.try_wait().unwrap().is_none(), "bị từ chối thì tiến trình phải còn sống");
        // Luật không coi test là "chính WinFreeUp" ⇒ kết thúc được tiến trình con do test tạo.
        let rules = EssentialRules::new(&crate::util::system_dir().unwrap(), u32::MAX);
        kill_with(pid, info.create_time, &rules).unwrap();
        let status = child.0.wait().unwrap();
        assert!(!status.success());
    }

    #[test]
    fn own_cpu_time_is_counted() {
        let a = own_cpu_100ns();
        let mut x = 0u64;
        for i in 0..20_000_000u64 {
            x = x.wrapping_add(i * i);
        }
        std::hint::black_box(x);
        assert!(own_cpu_100ns() > a);
    }

    #[test]
    fn path_cache_remembers_and_forgets() {
        let mut c = PathCache::default();
        let me = std::process::id();
        assert!(c.get(me, 1).is_some());
        c.retain(&HashSet::new());
        assert!(c.map.is_empty());
    }
}
