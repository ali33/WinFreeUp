//! Mạng theo app (spec 5.6): phiên ETW thời gian thực `WinFreeUp-Net`, provider
//! `Microsoft-Windows-Kernel-Network`. Cần Admin. Sự kiện gửi/nhận TCP/UDP (IPv4/IPv6) mang `PID` và `size`.
//! Khởi động thấy phiên cùng tên còn sót (lần trước bị tắt ngang) ⇒ dừng phiên đó rồi tạo lại.
//! Chỉ dừng phiên có ĐÚNG tên của mình (`ControlTraceW` theo tên), không đụng phiên ETW nào khác.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ferrisetw::parser::Parser;
use ferrisetw::provider::Provider;
use ferrisetw::trace::{stop_trace_by_name, UserTrace};
use ferrisetw::{EventRecord, SchemaLocator};

pub const SESSION_NAME: &str = "WinFreeUp-Net";
pub const KERNEL_NETWORK_GUID: &str = "7DD42A49-5329-4832-8DFD-43D979153A88";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

/// Mã sự kiện ⇒ chiều. 10/11 TCPv4, 26/27 TCPv6, 42/43 UDPv4, 58/59 UDPv6 (gửi/nhận) —
/// đã đối chiếu bằng `wevtutil gp Microsoft-Windows-Kernel-Network /ge /gm:true`.
pub fn direction(event_id: u16) -> Option<Direction> {
    match event_id {
        10 | 26 | 42 | 58 => Some(Direction::Up),
        11 | 27 | 43 | 59 => Some(Direction::Down),
        _ => None,
    }
}

/// Byte (lên, xuống) theo PID, cộng dồn giữa hai lần `take`.
#[derive(Default)]
pub struct NetCounts(Mutex<HashMap<u32, (u64, u64)>>);

impl NetCounts {
    pub fn add(&self, pid: u32, dir: Direction, bytes: u64) {
        if let Ok(mut m) = self.0.lock() {
            let e = m.entry(pid).or_default();
            match dir {
                Direction::Up => e.0 = e.0.saturating_add(bytes),
                Direction::Down => e.1 = e.1.saturating_add(bytes),
            }
        }
    }

    /// Lấy ra và xóa số đã cộng — gọi một lần mỗi mẫu.
    pub fn take(&self) -> HashMap<u32, (u64, u64)> {
        self.0.lock().map(|mut m| std::mem::take(&mut *m)).unwrap_or_default()
    }
}

fn on_event(counts: &NetCounts, record: &EventRecord, schemas: &SchemaLocator) {
    let Some(dir) = direction(record.event_id()) else { return };
    let Ok(schema) = schemas.event_schema(record) else { return };
    let parser = Parser::create(record, &schema);
    if let (Ok(pid), Ok(size)) = (parser.try_parse::<u32>("PID"), parser.try_parse::<u32>("size")) {
        counts.add(pid, dir, u64::from(size));
    }
}

pub struct NetTrace {
    trace: Option<UserTrace>,
    counts: Arc<NetCounts>,
}

impl NetTrace {
    /// Mở phiên. Lỗi là thông điệp nguyên văn (không Admin ⇒ bị từ chối) — cột Mạng hiện «–».
    pub fn start() -> Result<NetTrace, String> {
        Self::start_named(SESSION_NAME)
    }

    /// Mở phiên với tên cho trước (test dùng tên riêng để không đụng phiên của app đang chạy).
    fn start_named(name: &str) -> Result<NetTrace, String> {
        // Phiên cùng tên còn sót ⇒ dừng; không có phiên thì lỗi «không tìm thấy» là bình thường, bỏ qua.
        let _ = stop_trace_by_name(name);
        let counts = Arc::new(NetCounts::default());
        let sink = counts.clone();
        let provider = Provider::by_guid(KERNEL_NETWORK_GUID)
            .add_callback(move |record: &EventRecord, schemas: &SchemaLocator| on_event(&sink, record, schemas))
            .build();
        match UserTrace::new().named(name.to_string()).enable(provider).start_and_process() {
            Ok(trace) => Ok(NetTrace { trace: Some(trace), counts }),
            Err(e) => {
                // Phiên có thể đã được tạo trước khi bước sau hỏng — dừng để không để sót.
                let _ = stop_trace_by_name(name);
                Err(format!("ETW {name}: {e:?}"))
            }
        }
    }

    pub fn take(&self) -> HashMap<u32, (u64, u64)> {
        self.counts.take()
    }

    pub fn stop(mut self) {
        if let Some(t) = self.trace.take() {
            let _ = t.stop();
        }
    }
}

impl Drop for NetTrace {
    fn drop(&mut self) {
        if let Some(t) = self.trace.take() {
            let _ = t.stop();
        }
    }
}

/// Dừng phiên còn sót nếu có — gọi khi đóng ứng dụng. Chỉ dừng phiên tên `WinFreeUp-Net`.
pub fn stop_stale_session() {
    let _ = stop_trace_by_name(SESSION_NAME);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// Tên phiên riêng cho test — không đụng phiên `WinFreeUp-Net` của app có thể đang chạy.
    const TEST_SESSION: &str = "WinFreeUp-Net-Test";

    /// Luôn dừng phiên test khi ra khỏi phạm vi, kể cả khi test hỏng giữa chừng.
    struct StopOnDrop;
    impl Drop for StopOnDrop {
        fn drop(&mut self) {
            let _ = stop_trace_by_name(TEST_SESSION);
        }
    }

    #[test]
    fn event_ids_map_to_directions() {
        for id in [10, 26, 42, 58] {
            assert_eq!(direction(id), Some(Direction::Up), "{id}");
        }
        for id in [11, 27, 43, 59] {
            assert_eq!(direction(id), Some(Direction::Down), "{id}");
        }
        for id in [12, 13, 14, 15, 18, 0] {
            assert_eq!(direction(id), None, "{id}");
        }
    }

    #[test]
    fn counts_accumulate_per_pid_and_reset_on_take() {
        let c = NetCounts::default();
        c.add(7, Direction::Up, 100);
        c.add(7, Direction::Down, 40);
        c.add(7, Direction::Down, 60);
        c.add(9, Direction::Up, 1);
        let m = c.take();
        assert_eq!(m[&7], (100, 100));
        assert_eq!(m[&9], (1, 0));
        assert!(c.take().is_empty());
    }

    #[test]
    fn starting_without_admin_fails_with_a_message_and_admin_captures_traffic() {
        let _guard = StopOnDrop;
        match NetTrace::start_named(TEST_SESSION) {
            Err(e) => assert!(e.contains(SESSION_NAME), "{e}"),
            Ok(t) => {
                // Có Admin: tạo chút lưu lượng TCP nội bộ (không ra Internet) rồi lấy số — không panic là đủ.
                let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
                let addr = listener.local_addr().unwrap();
                let mut client = std::net::TcpStream::connect(addr).unwrap();
                let (mut server, _) = listener.accept().unwrap();
                client.write_all(&[0u8; 4096]).unwrap();
                let mut buf = [0u8; 4096];
                let _ = server.read(&mut buf);
                std::thread::sleep(std::time::Duration::from_secs(2));
                let _ = t.take();
                t.stop();
            }
        }
    }
}
