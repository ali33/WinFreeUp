//! Nhiệt độ qua ACPI (`MSAcpi_ThermalZoneTemperature`, WMI `root\wmi`) — spec 5.5. Không dùng driver.
//! Nhiều máy không có lớp này, hoặc trả số cố định; `detect::TempTracker` loại các số đó.
use serde::Deserialize;
use wmi::WMIConnection;

use crate::wmiq;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Zone {
    current_temperature: u32,
}

/// Đổi số đo `MSAcpi_ThermalZoneTemperature` (phần mười Kelvin) sang °C.
pub fn decikelvin_to_celsius(v: u32) -> f32 {
    v as f32 / 10.0 - 273.15
}

/// Nhiệt độ cao nhất trong các vùng nhiệt (°C) từ danh sách phần mười Kelvin.
pub fn hottest_celsius(decikelvins: &[u32]) -> Option<f32> {
    decikelvins.iter().copied().filter(|&v| v > 0).map(decikelvin_to_celsius).reduce(f32::max)
}

/// Giữ kết nối WMI trên luồng lấy mẫu để không phải mở lại mỗi lần đọc.
pub struct ThermalReader {
    conn: Result<WMIConnection, String>,
}

impl ThermalReader {
    pub fn new() -> Self {
        ThermalReader { conn: wmiq::connect(wmiq::NS_WMI) }
    }

    /// °C của vùng nóng nhất; lỗi là thông điệp nguyên văn (không có lớp, bị từ chối quyền…).
    pub fn read(&self) -> Result<f32, String> {
        let conn = self.conn.as_ref().map_err(|e| e.clone())?;
        let zones: Vec<Zone> = wmiq::query_on(conn, "SELECT CurrentTemperature FROM MSAcpi_ThermalZoneTemperature")?;
        let raw: Vec<u32> = zones.iter().map(|z| z.current_temperature).collect();
        hottest_celsius(&raw).ok_or_else(|| "MSAcpi_ThermalZoneTemperature: no instances".to_string())
    }
}

impl Default for ThermalReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decikelvin_conversion() {
        assert!((decikelvin_to_celsius(3232) - 50.05).abs() < 0.01);
    }

    #[test]
    fn hottest_zone_wins_and_zero_is_ignored() {
        assert_eq!(hottest_celsius(&[]), None);
        assert_eq!(hottest_celsius(&[0]), None);
        let c = hottest_celsius(&[3032, 3232, 0]).unwrap();
        assert!((c - 50.05).abs() < 0.01);
    }

    #[test]
    fn reading_on_this_machine_is_a_value_or_a_message() {
        // Không Admin thường bị từ chối; máy ảo thường không có lớp — cả hai phải là lỗi có chữ, không panic.
        match ThermalReader::new().read() {
            Ok(c) => assert!(c > -100.0 && c < 200.0),
            Err(e) => assert!(!e.is_empty()),
        }
    }
}
