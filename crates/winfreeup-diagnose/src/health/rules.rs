//! Luật khám nhanh (spec 4) — hàm thuần, không gọi hệ thống, test được từng ngưỡng.
//! Nguồn số liệu nào không đọc được ⇒ mức `unknown` kèm lý do nguyên văn, KHÔNG BAO GIỜ là `ok`.
use serde::Serialize;

use crate::perf::counters::MemoryStatus;
use crate::perf::detect::{find_throttle, Point, TEMP_MAX_C, TEMP_MIN_C};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Critical,
    Warn,
    Ok,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Finding {
    pub id: &'static str,
    pub level: Level,
    /// Con số cho câu giải thích: % trống, % RAM, số app, số ngày, °C.
    pub value: Option<f64>,
    /// Lý do «Không đo được» (nguyên văn), hoặc tên ổ đĩa có vấn đề.
    pub detail: Option<String>,
}

pub type Measured<T> = Result<T, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskHealth {
    pub name: String,
    /// `MSFT_PhysicalDisk.HealthStatus`: 0 Healthy, 1 Warning, 2 Unhealthy, 5 Unknown.
    pub health_status: u16,
    /// SMART `PredictFailure`; `None` khi ổ không hỗ trợ / không đọc được.
    pub predict_failure: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Hdd,
    Ssd,
    Scm,
    Unspecified,
}

impl MediaType {
    /// `MSFT_PhysicalDisk.MediaType`: 3 HDD, 4 SSD, 5 SCM, còn lại chưa rõ.
    pub fn from_code(code: u16) -> MediaType {
        match code {
            3 => MediaType::Hdd,
            4 => MediaType::Ssd,
            5 => MediaType::Scm,
            _ => MediaType::Unspecified,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerState {
    pub on_ac: bool,
    /// Tiết kiệm pin đang bật hoặc gói nguồn đang là «Power saver».
    pub saver: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HealthMetrics {
    /// (byte trống, tổng) của ổ hệ thống.
    pub system_free: Measured<(u64, u64)>,
    pub disks: Measured<Vec<DiskHealth>>,
    pub system_media: Measured<MediaType>,
    pub memory: Measured<MemoryStatus>,
    pub startup_enabled: Measured<u32>,
    pub uptime_secs: Measured<u64>,
    pub power: Measured<PowerState>,
    pub cpu_temp_c: Measured<f32>,
}

pub const DISK_WARN_FREE_PCT: f64 = 15.0;
pub const DISK_CRIT_FREE_PCT: f64 = 5.0;
pub const RAM_WARN_PCT: u32 = 85;
pub const COMMIT_CRIT_RATIO: f64 = 0.90;
/// Giao diện (`overview/OverviewView.tsx`) lặp lại hằng này — đổi thì đổi cả hai.
pub const STARTUP_WARN_COUNT: u32 = 8;
pub const UPTIME_WARN_SECS: u64 = 7 * 24 * 3600;
pub const CPU_HOT_C: f32 = 90.0;

fn finding(id: &'static str, level: Level, value: Option<f64>) -> Finding {
    Finding {
        id,
        level,
        value,
        detail: None,
    }
}

fn unknown(id: &'static str, reason: &str) -> Finding {
    Finding {
        id,
        level: Level::Unknown,
        value: None,
        detail: Some(reason.to_string()),
    }
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

pub fn disk_full(m: &Measured<(u64, u64)>) -> Finding {
    match m {
        Err(e) => unknown("disk_full", e),
        Ok((_, 0)) => unknown("disk_full", "total size is 0"),
        Ok((free, total)) if free > total => {
            unknown("disk_full", &format!("free {free} > total {total}"))
        }
        Ok((free, total)) => {
            let pct = *free as f64 * 100.0 / *total as f64;
            let level = if pct < DISK_CRIT_FREE_PCT {
                Level::Critical
            } else if pct < DISK_WARN_FREE_PCT {
                Level::Warn
            } else {
                Level::Ok
            };
            finding("disk_full", level, Some(round1(pct)))
        }
    }
}

pub fn disk_health(m: &Measured<Vec<DiskHealth>>) -> Finding {
    let disks = match m {
        Err(e) => return unknown("disk_health", e),
        Ok(d) if d.is_empty() => return unknown("disk_health", "no physical disks reported"),
        Ok(d) => d,
    };
    if let Some(bad) = disks
        .iter()
        .find(|d| matches!(d.health_status, 1 | 2) || d.predict_failure == Some(true))
    {
        return Finding {
            id: "disk_health",
            level: Level::Critical,
            value: None,
            detail: Some(bad.name.clone()),
        };
    }
    // Ổ nào không có HealthStatus tốt lẫn SMART đọc được thì chưa đo được — cả dòng không được là 🟢.
    if let Some(unmeasured) = disks
        .iter()
        .find(|d| d.health_status != 0 && d.predict_failure.is_none())
    {
        return unknown(
            "disk_health",
            &format!(
                "HealthStatus {} ({})",
                unmeasured.health_status, unmeasured.name
            ),
        );
    }
    finding("disk_health", Level::Ok, None)
}

pub fn system_hdd(m: &Measured<MediaType>) -> Finding {
    match m {
        Err(e) => unknown("system_hdd", e),
        Ok(MediaType::Hdd) => finding("system_hdd", Level::Warn, None),
        Ok(MediaType::Ssd | MediaType::Scm) => finding("system_hdd", Level::Ok, None),
        Ok(MediaType::Unspecified) => unknown("system_hdd", "MediaType Unspecified"),
    }
}

pub fn ram_pressure(m: &Measured<MemoryStatus>) -> Finding {
    match m {
        Err(e) => unknown("ram_pressure", e),
        Ok(mem) if mem.commit_limit == 0 => unknown("ram_pressure", "commit limit is 0"),
        Ok(mem) => {
            let commit = mem.commit_used as f64 / mem.commit_limit as f64;
            if commit > COMMIT_CRIT_RATIO {
                finding(
                    "ram_pressure",
                    Level::Critical,
                    Some(round1(commit * 100.0)),
                )
            } else if mem.load_pct > RAM_WARN_PCT {
                finding("ram_pressure", Level::Warn, Some(f64::from(mem.load_pct)))
            } else {
                finding("ram_pressure", Level::Ok, Some(f64::from(mem.load_pct)))
            }
        }
    }
}

pub fn startup_apps(m: &Measured<u32>) -> Finding {
    match m {
        Err(e) => unknown("startup_apps", e),
        Ok(n) => finding(
            "startup_apps",
            if *n > STARTUP_WARN_COUNT {
                Level::Warn
            } else {
                Level::Ok
            },
            Some(f64::from(*n)),
        ),
    }
}

pub fn uptime(m: &Measured<u64>) -> Finding {
    match m {
        Err(e) => unknown("uptime", e),
        Ok(s) => finding(
            "uptime",
            if *s > UPTIME_WARN_SECS {
                Level::Warn
            } else {
                Level::Ok
            },
            Some(round1(*s as f64 / 86_400.0)),
        ),
    }
}

pub fn power_saver(m: &Measured<PowerState>) -> Finding {
    match m {
        Err(e) => unknown("power_saver", e),
        Ok(p) => finding(
            "power_saver",
            if p.on_ac && p.saver {
                Level::Warn
            } else {
                Level::Ok
            },
            None,
        ),
    }
}

pub fn cpu_hot(m: &Measured<f32>) -> Finding {
    match m {
        Err(e) => unknown("cpu_hot", e),
        Ok(c) if !(TEMP_MIN_C..=TEMP_MAX_C).contains(c) => {
            unknown("cpu_hot", &format!("out of range: {c:.1} °C"))
        }
        Ok(c) => finding(
            "cpu_hot",
            if *c > CPU_HOT_C {
                Level::Warn
            } else {
                Level::Ok
            },
            Some(round1(f64::from(*c))),
        ),
    }
}

/// Spec 4 `cpu_throttle`: lấy mẫu 10 giây lúc khám, áp luật mục 5.4.
pub fn cpu_throttle(points: &Measured<Vec<Point>>) -> Finding {
    match points {
        Err(e) => unknown("cpu_throttle", e),
        Ok(p) if p.is_empty() => unknown("cpu_throttle", "no samples"),
        Ok(p) if p.iter().any(|x| x.perf.is_none()) => {
            unknown("cpu_throttle", "% Processor Performance counter is missing")
        }
        Ok(p) => finding(
            "cpu_throttle",
            if find_throttle(p).is_empty() {
                Level::Ok
            } else {
                Level::Warn
            },
            None,
        ),
    }
}

/// Tám dòng khám nhanh, đúng thứ tự bảng spec mục 4 (dòng `cpu_throttle` đến riêng sau 10 giây).
pub fn evaluate(m: &HealthMetrics) -> Vec<Finding> {
    vec![
        disk_full(&m.system_free),
        disk_health(&m.disks),
        system_hdd(&m.system_media),
        ram_pressure(&m.memory),
        startup_apps(&m.startup_enabled),
        uptime(&m.uptime_secs),
        power_saver(&m.power),
        cpu_hot(&m.cpu_temp_c),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1 << 30;

    fn mem(load: u32, used: u64, limit: u64) -> Measured<MemoryStatus> {
        Ok(MemoryStatus {
            load_pct: load,
            total: 16 * GB,
            available: 4 * GB,
            commit_used: used,
            commit_limit: limit,
        })
    }

    #[test]
    fn disk_full_thresholds_and_exact_boundaries() {
        assert_eq!(
            disk_full(&Ok((15, 100))).level,
            Level::Ok,
            "đúng 15% chưa cảnh báo"
        );
        assert_eq!(disk_full(&Ok((149, 1000))).level, Level::Warn);
        assert_eq!(
            disk_full(&Ok((5, 100))).level,
            Level::Warn,
            "đúng 5% chưa nghiêm trọng"
        );
        assert_eq!(disk_full(&Ok((49, 1000))).level, Level::Critical);
        assert_eq!(disk_full(&Ok((49, 1000))).value, Some(4.9));
        assert_eq!(disk_full(&Ok((0, 0))).level, Level::Unknown);
        assert_eq!(
            disk_full(&Ok((200, 100))).level,
            Level::Unknown,
            "trống > tổng là số hỏng, không phải 🟢"
        );
    }

    #[test]
    fn disk_health_from_status_and_smart() {
        let d = |s: u16, p: Option<bool>| DiskHealth {
            name: "Samsung".into(),
            health_status: s,
            predict_failure: p,
        };
        assert_eq!(disk_health(&Ok(vec![d(0, Some(false))])).level, Level::Ok);
        assert_eq!(
            disk_health(&Ok(vec![d(0, None)])).level,
            Level::Ok,
            "NVMe không có SMART cũ vẫn ổn nếu HealthStatus tốt"
        );
        let bad = disk_health(&Ok(vec![d(0, None), d(1, None)]));
        assert_eq!(
            (bad.level, bad.detail.as_deref()),
            (Level::Critical, Some("Samsung"))
        );
        assert_eq!(disk_health(&Ok(vec![d(2, None)])).level, Level::Critical);
        assert_eq!(
            disk_health(&Ok(vec![d(0, Some(true))])).level,
            Level::Critical,
            "SMART báo sắp hỏng"
        );
        assert_eq!(
            disk_health(&Ok(vec![d(5, None)])).level,
            Level::Unknown,
            "HealthStatus Unknown không phải 🔴"
        );
        assert_eq!(disk_health(&Ok(vec![])).level, Level::Unknown);
    }

    #[test]
    fn one_unmeasured_disk_makes_the_row_unknown_not_ok() {
        let d = |n: &str, s: u16, p: Option<bool>| DiskHealth {
            name: n.into(),
            health_status: s,
            predict_failure: p,
        };
        let f = disk_health(&Ok(vec![d("Samsung", 0, None), d("WD", 5, None)]));
        assert_eq!(f.level, Level::Unknown);
        assert!(
            f.detail.as_deref().is_some_and(|s| s.contains("WD")),
            "{:?}",
            f.detail
        );
        assert_eq!(
            disk_health(&Ok(vec![d("WD", 5, Some(false))])).level,
            Level::Ok,
            "SMART đọc được và ổn"
        );
        assert_eq!(
            disk_health(&Ok(vec![d("WD", 7, None)])).level,
            Level::Unknown,
            "mã lạ không phải 🟢"
        );
        let crit = disk_health(&Ok(vec![d("WD", 5, None), d("Samsung", 2, None)]));
        assert_eq!(
            (crit.level, crit.detail.as_deref()),
            (Level::Critical, Some("Samsung")),
            "🔴 thắng dòng không đo được"
        );
    }

    #[test]
    fn system_drive_media_type() {
        assert_eq!(system_hdd(&Ok(MediaType::Hdd)).level, Level::Warn);
        assert_eq!(system_hdd(&Ok(MediaType::Ssd)).level, Level::Ok);
        assert_eq!(
            system_hdd(&Ok(MediaType::Unspecified)).level,
            Level::Unknown,
            "máy ảo thường không rõ loại ổ"
        );
        assert_eq!(MediaType::from_code(3), MediaType::Hdd);
        assert_eq!(MediaType::from_code(0), MediaType::Unspecified);
    }

    #[test]
    fn ram_thresholds_and_boundaries() {
        assert_eq!(
            ram_pressure(&mem(85, 10, 100)).level,
            Level::Ok,
            "đúng 85% chưa cảnh báo"
        );
        assert_eq!(ram_pressure(&mem(86, 10, 100)).level, Level::Warn);
        assert_eq!(
            ram_pressure(&mem(50, 90, 100)).level,
            Level::Ok,
            "commit đúng 90% chưa nghiêm trọng"
        );
        let crit = ram_pressure(&mem(50, 91, 100));
        assert_eq!((crit.level, crit.value), (Level::Critical, Some(91.0)));
        assert_eq!(
            ram_pressure(&mem(50, 10, 0)).level,
            Level::Unknown,
            "không có giới hạn commit thì không xét được 🔴"
        );
    }

    #[test]
    fn startup_uptime_power_temperature() {
        assert_eq!(
            STARTUP_WARN_COUNT, 8,
            "giao diện (OverviewView.tsx) lặp hằng này = 8"
        );
        assert_eq!(startup_apps(&Ok(8)).level, Level::Ok);
        assert_eq!(startup_apps(&Ok(9)).level, Level::Warn);
        assert_eq!(
            uptime(&Ok(UPTIME_WARN_SECS)).level,
            Level::Ok,
            "đúng 7 ngày chưa cảnh báo"
        );
        let up = uptime(&Ok(UPTIME_WARN_SECS + 86_400));
        assert_eq!((up.level, up.value), (Level::Warn, Some(8.0)));
        assert_eq!(
            power_saver(&Ok(PowerState {
                on_ac: true,
                saver: true
            }))
            .level,
            Level::Warn
        );
        assert_eq!(
            power_saver(&Ok(PowerState {
                on_ac: false,
                saver: true
            }))
            .level,
            Level::Ok,
            "đang chạy pin thì tiết kiệm là đúng"
        );
        assert_eq!(cpu_hot(&Ok(90.0)).level, Level::Ok);
        assert_eq!(cpu_hot(&Ok(90.5)).level, Level::Warn);
        assert_eq!(
            cpu_hot(&Ok(150.0)).level,
            Level::Unknown,
            "số ngoài 20–110 °C là cảm biến hỏng"
        );
        assert_eq!(cpu_hot(&Ok(f32::NAN)).level, Level::Unknown);
    }

    #[test]
    fn every_missing_source_is_unknown_with_its_reason_never_ok() {
        fn e<T>() -> Measured<T> {
            Err("Access denied".to_string())
        }
        let m = HealthMetrics {
            system_free: e(),
            disks: e(),
            system_media: e(),
            memory: e(),
            startup_enabled: e(),
            uptime_secs: e(),
            power: e(),
            cpu_temp_c: e(),
        };
        let all = evaluate(&m);
        assert_eq!(
            all.iter().map(|f| f.id).collect::<Vec<_>>(),
            vec![
                "disk_full",
                "disk_health",
                "system_hdd",
                "ram_pressure",
                "startup_apps",
                "uptime",
                "power_saver",
                "cpu_hot"
            ]
        );
        for f in all {
            assert_eq!(f.level, Level::Unknown, "{}", f.id);
            assert_eq!(f.detail.as_deref(), Some("Access denied"));
        }
        assert_eq!(
            cpu_throttle(&Err("PDH open: 0xC0000BB8".into())).level,
            Level::Unknown
        );
    }

    #[test]
    fn throttle_finding_from_ten_second_sampling() {
        let pts = |perf: Option<f32>| -> Measured<Vec<Point>> {
            Ok((0..10)
                .map(|i| Point {
                    t_ms: (i + 1) * 1000,
                    dur_ms: 1000,
                    cpu: 95.0,
                    disk: 0.0,
                    perf,
                })
                .collect())
        };
        assert_eq!(cpu_throttle(&pts(Some(50.0))).level, Level::Warn);
        assert_eq!(cpu_throttle(&pts(Some(99.0))).level, Level::Ok);
        assert_eq!(cpu_throttle(&pts(None)).level, Level::Unknown);
        assert_eq!(cpu_throttle(&Ok(vec![])).level, Level::Unknown);
    }

    #[test]
    fn finding_serializes_for_the_ui() {
        let v = serde_json::to_value(disk_full(&Ok((10, 100)))).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"id": "disk_full", "level": "warn", "value": 10.0, "detail": null})
        );
    }
}
