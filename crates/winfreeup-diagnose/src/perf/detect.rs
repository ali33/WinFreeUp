//! Hàm thuần trên chuỗi mẫu (spec 5.3–5.5): cơn giật, hạ xung, nhiệt độ đáng tin, giãn chu kỳ lấy mẫu.
use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;

/// Cơn giật: CPU hoặc Hoạt động đĩa vượt ngưỡng này…
pub const STUTTER_PCT: f32 = 90.0;
/// …trong ít nhất chừng này mẫu liên tiếp.
pub const STUTTER_MIN_SAMPLES: usize = 3;
/// Hai cơn cách nhau dưới 2 giây thì gộp.
pub const MERGE_GAP_MS: u64 = 2_000;
/// Hạ xung: tải CPU > 80% và hiệu năng < 70% liên tục ≥ 10 giây.
pub const THROTTLE_LOAD_PCT: f32 = 80.0;
pub const THROTTLE_PERF_PCT: f32 = 70.0;
pub const THROTTLE_MIN_MS: u64 = 10_000;
/// Nhiệt độ hợp lệ 20–110 °C; đứng yên suốt 60 giây ⇒ cảm biến giả.
pub const TEMP_MIN_C: f32 = 20.0;
pub const TEMP_MAX_C: f32 = 110.0;
pub const TEMP_STUCK_MS: u64 = 60_000;
/// Chi phí của chính WinFreeUp vượt 2% CPU ⇒ giãn chu kỳ còn 2 giây.
pub const SELF_CPU_BUDGET_PCT: f32 = 2.0;
pub const SLOW_INTERVAL_MS: u32 = 2_000;

/// Một mẫu: thời điểm kết thúc `t_ms`, mẫu đại diện cho `dur_ms` trước đó.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub t_ms: u64,
    pub dur_ms: u64,
    pub cpu: f32,
    pub disk: f32,
    /// % Processor Performance; `None` khi bộ đếm không có.
    pub perf: Option<f32>,
}

impl Point {
    fn start_ms(&self) -> u64 {
        self.t_ms.saturating_sub(self.dur_ms)
    }
}

/// Hai mẫu kề nhau có liền mạch không (không có khoảng dừng lấy mẫu ở giữa).
/// Đồng hồ lùi (`cur.t_ms < prev.t_ms`) ⇒ đứt đoạn.
fn contiguous(prev: &Point, cur: &Point) -> bool {
    cur.t_ms >= prev.t_ms && cur.t_ms - prev.t_ms <= cur.dur_ms + cur.dur_ms / 2
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Span {
    pub start_ms: u64,
    pub end_ms: u64,
    /// Chỉ số mẫu đầu/cuối (gồm cả hai) trong chuỗi đã đưa vào.
    pub first: usize,
    pub last: usize,
    pub cpu: bool,
    pub disk: bool,
}

/// Các đoạn liên tiếp thỏa `hot`, dài ít nhất `min_samples` mẫu.
fn runs(points: &[Point], min_samples: usize, hot: impl Fn(&Point) -> bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    for i in 0..=points.len() {
        let continues = i < points.len()
            && hot(&points[i])
            && start.is_none_or(|_| contiguous(&points[i - 1], &points[i]));
        match (start, continues) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                if i - s >= min_samples {
                    out.push((s, i - 1));
                }
                start = if i < points.len() && hot(&points[i]) {
                    Some(i)
                } else {
                    None
                };
            }
            _ => {}
        }
    }
    out
}

/// Spec 5.3: cơn giật = CPU hoặc Đĩa > 90% trong ≥ 3 mẫu liên tiếp; hai cơn cách < 2 giây gộp làm một.
pub fn find_stutters(points: &[Point]) -> Vec<Span> {
    let hot = |p: &Point| p.cpu > STUTTER_PCT || p.disk > STUTTER_PCT;
    let mut out: Vec<Span> = Vec::new();
    for (a, b) in runs(points, STUTTER_MIN_SAMPLES, hot) {
        let slice = &points[a..=b];
        let span = Span {
            start_ms: points[a].start_ms(),
            end_ms: points[b].t_ms,
            first: a,
            last: b,
            cpu: slice.iter().any(|p| p.cpu > STUTTER_PCT),
            disk: slice.iter().any(|p| p.disk > STUTTER_PCT),
        };
        match out.last_mut() {
            // Đồng hồ lùi (cơn sau bắt đầu trước khi cơn trước kết thúc) ⇒ không gộp.
            Some(prev)
                if span.start_ms >= prev.end_ms && span.start_ms - prev.end_ms < MERGE_GAP_MS =>
            {
                prev.end_ms = span.end_ms;
                prev.last = span.last;
                prev.cpu |= span.cpu;
                prev.disk |= span.disk;
            }
            _ => out.push(span),
        }
    }
    out
}

/// Spec 5.4: các đoạn hạ xung (tải > 80% và hiệu năng < 70% liên tục ≥ 10 giây).
pub fn find_throttle(points: &[Point]) -> Vec<Span> {
    let hot =
        |p: &Point| p.cpu > THROTTLE_LOAD_PCT && p.perf.is_some_and(|v| v < THROTTLE_PERF_PCT);
    runs(points, 1, hot)
        .into_iter()
        .map(|(a, b)| Span {
            start_ms: points[a].start_ms(),
            end_ms: points[b].t_ms,
            first: a,
            last: b,
            cpu: true,
            disk: false,
        })
        .filter(|s| s.end_ms.saturating_sub(s.start_ms) >= THROTTLE_MIN_MS)
        .collect()
}

/// Trạng thái hạ xung ở mẫu mới nhất: `None` khi bộ đếm hiệu năng không có (ẩn chỉ báo).
pub fn throttled_now(points: &[Point]) -> Option<bool> {
    let last = points.last()?;
    last.perf?;
    Some(
        find_throttle(points)
            .last()
            .is_some_and(|s| s.last == points.len() - 1),
    )
}

/// Tải của một app trong một mẫu (dùng để chọn 3 app ngốn nhất trong cơn).
#[derive(Debug, Clone, PartialEq)]
pub struct AppLoad {
    pub key: Arc<str>,
    pub name: Arc<str>,
    pub cpu: f32,
    pub disk_bps: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopApp {
    pub key: String,
    pub name: String,
    /// Trung bình trong cơn: % CPU hoặc byte/giây đĩa.
    pub avg: f64,
}

/// 3 app ngốn nhất trong các khung mẫu của cơn, theo trung bình (khung vắng app tính 0).
pub fn top_apps(frames: &[&[AppLoad]], by_cpu: bool, n: usize) -> Vec<TopApp> {
    let mut sum: HashMap<&str, (f64, &str)> = HashMap::new();
    for f in frames {
        for a in f.iter() {
            let v = if by_cpu { f64::from(a.cpu) } else { a.disk_bps };
            let e = sum.entry(&a.key).or_insert((0.0, &a.name));
            e.0 += v;
        }
    }
    let count = frames.len().max(1) as f64;
    let mut v: Vec<TopApp> = sum
        .into_iter()
        .filter(|(_, (s, _))| *s > 0.0)
        .map(|(k, (s, name))| TopApp {
            key: k.to_string(),
            name: name.to_string(),
            avg: s / count,
        })
        .collect();
    v.sort_by(|a, b| b.avg.total_cmp(&a.avg).then_with(|| a.key.cmp(&b.key)));
    v.truncate(n);
    v
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TempState {
    Ok {
        celsius: f32,
    },
    /// `code`: `no_sensor` (không có lớp WMI / không đọc được — `detail` nguyên văn), `out_of_range`, `stuck`.
    Unavailable {
        code: String,
        detail: String,
    },
}

/// Spec 5.5: loại số đo ngoài 20–110 °C và cảm biến đứng yên suốt 60 giây.
#[derive(Debug, Default)]
pub struct TempTracker {
    last: Option<f32>,
    same_since: u64,
}

impl TempTracker {
    pub fn push(&mut self, t_ms: u64, reading: std::result::Result<f32, String>) -> TempState {
        let c = match reading {
            Err(detail) => {
                self.last = None;
                return TempState::Unavailable {
                    code: "no_sensor".into(),
                    detail,
                };
            }
            Ok(c) => c,
        };
        if !(TEMP_MIN_C..=TEMP_MAX_C).contains(&c) {
            self.last = None;
            return TempState::Unavailable {
                code: "out_of_range".into(),
                detail: format!("{c:.1}"),
            };
        }
        match self.last {
            Some(prev) if (prev - c).abs() < 0.05 => {}
            _ => {
                self.last = Some(c);
                self.same_since = t_ms;
            }
        }
        if t_ms.saturating_sub(self.same_since) >= TEMP_STUCK_MS {
            TempState::Unavailable {
                code: "stuck".into(),
                detail: format!("{c:.1}"),
            }
        } else {
            TempState::Ok { celsius: c }
        }
    }
}

/// Spec 5: chi phí của chính mình vượt 2% CPU ⇒ giãn chu kỳ còn 2 giây (không tự rút ngắn lại trong phiên).
pub fn next_interval_ms(current_ms: u32, own_cpu_pct: f32) -> u32 {
    if own_cpu_pct > SELF_CPU_BUDGET_PCT {
        SLOW_INTERVAL_MS.max(current_ms)
    } else {
        current_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Chuỗi mẫu 1 giây/mẫu: `c` = CPU nóng, `d` = đĩa nóng, `.` = bình thường.
    fn series(pattern: &str) -> Vec<Point> {
        pattern
            .chars()
            .enumerate()
            .map(|(i, ch)| Point {
                t_ms: (i as u64 + 1) * 1000,
                dur_ms: 1000,
                cpu: if ch == 'c' { 95.0 } else { 10.0 },
                disk: if ch == 'd' { 99.0 } else { 5.0 },
                perf: Some(100.0),
            })
            .collect()
    }

    #[test]
    fn exactly_three_seconds_is_a_stutter_two_is_not() {
        let s = find_stutters(&series("..ccc.."));
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].start_ms, s[0].end_ms), (2000, 5000));
        assert!(s[0].cpu && !s[0].disk);
        assert!(find_stutters(&series("..cc...")).is_empty());
    }

    #[test]
    fn exactly_ninety_percent_is_not_hot() {
        let mut p = series("ccc");
        for x in &mut p {
            x.cpu = 90.0;
        }
        assert!(find_stutters(&p).is_empty());
    }

    #[test]
    fn cpu_or_disk_counts_and_both_flags_are_kept() {
        let s = find_stutters(&series("cdc"));
        assert_eq!(s.len(), 1);
        assert!(s[0].cpu && s[0].disk);
    }

    #[test]
    fn two_stutters_one_second_apart_merge() {
        let s = find_stutters(&series("ccc.ddd"));
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].first, s[0].last), (0, 6));
        assert!(s[0].cpu && s[0].disk);
    }

    #[test]
    fn exactly_two_seconds_apart_stay_separate_and_three_too() {
        assert_eq!(find_stutters(&series("ccc..ccc")).len(), 2);
        assert_eq!(find_stutters(&series("ccc...ccc")).len(), 2);
    }

    #[test]
    fn a_gap_in_sampling_breaks_the_run() {
        let mut p = series("cccc");
        // Rời mục 30 giây rồi quay lại: mẫu thứ 3 không liền với mẫu thứ 2.
        p[2].t_ms += 30_000;
        p[3].t_ms += 30_000;
        assert!(find_stutters(&p).is_empty());
    }

    #[test]
    fn slower_two_second_sampling_still_needs_three_samples() {
        let p: Vec<Point> = (0..3)
            .map(|i| Point {
                t_ms: (i + 1) * 2000,
                dur_ms: 2000,
                cpu: 99.0,
                disk: 0.0,
                perf: None,
            })
            .collect();
        assert_eq!(find_stutters(&p).len(), 1);
        assert_eq!(find_stutters(&p[..2]).len(), 0);
    }

    fn throttle_series(n: usize, load: f32, perf: Option<f32>) -> Vec<Point> {
        (0..n)
            .map(|i| Point {
                t_ms: (i as u64 + 1) * 1000,
                dur_ms: 1000,
                cpu: load,
                disk: 0.0,
                perf,
            })
            .collect()
    }

    #[test]
    fn throttle_needs_ten_full_seconds() {
        assert_eq!(
            find_throttle(&throttle_series(10, 95.0, Some(50.0))).len(),
            1
        );
        assert!(find_throttle(&throttle_series(9, 95.0, Some(50.0))).is_empty());
        assert_eq!(
            throttled_now(&throttle_series(10, 95.0, Some(50.0))),
            Some(true)
        );
        assert_eq!(
            throttled_now(&throttle_series(9, 95.0, Some(50.0))),
            Some(false)
        );
    }

    #[test]
    fn throttle_boundaries_and_missing_counter() {
        assert!(
            find_throttle(&throttle_series(12, 80.0, Some(50.0))).is_empty(),
            "tải đúng 80% chưa tính"
        );
        assert!(
            find_throttle(&throttle_series(12, 95.0, Some(70.0))).is_empty(),
            "hiệu năng đúng 70% chưa tính"
        );
        assert_eq!(
            throttled_now(&throttle_series(12, 95.0, None)),
            None,
            "không có bộ đếm ⇒ ẩn chỉ báo"
        );
        assert_eq!(throttled_now(&[]), None);
    }

    #[test]
    fn top_three_apps_by_the_metric_that_spiked() {
        let a = |k: &str, cpu: f32, disk: f64| AppLoad {
            key: k.into(),
            name: k.to_uppercase().into(),
            cpu,
            disk_bps: disk,
        };
        let f1 = vec![
            a("chrome", 60.0, 0.0),
            a("defender", 30.0, 9e6),
            a("x", 1.0, 0.0),
            a("y", 2.0, 0.0),
        ];
        let f2 = vec![
            a("chrome", 40.0, 0.0),
            a("defender", 50.0, 7e6),
            a("y", 2.0, 0.0),
        ];
        let frames: Vec<&[AppLoad]> = vec![&f1, &f2];
        let cpu = top_apps(&frames, true, 3);
        assert_eq!(
            cpu.iter().map(|t| t.key.as_str()).collect::<Vec<_>>(),
            vec!["chrome", "defender", "y"]
        );
        assert_eq!(cpu[0].avg, 50.0);
        assert_eq!(cpu[0].name, "CHROME");
        let disk = top_apps(&frames, false, 3);
        assert_eq!(disk.len(), 1, "app không có đĩa thì không liệt kê");
        assert_eq!(disk[0].avg, 8e6);
    }

    #[test]
    fn temperature_is_trusted_only_in_range_and_while_moving() {
        let mut t = TempTracker::default();
        assert_eq!(t.push(0, Ok(55.0)), TempState::Ok { celsius: 55.0 });
        assert_eq!(t.push(59_000, Ok(55.0)), TempState::Ok { celsius: 55.0 });
        assert_eq!(
            t.push(60_000, Ok(55.0)),
            TempState::Unavailable {
                code: "stuck".into(),
                detail: "55.0".into()
            }
        );
        assert_eq!(t.push(61_000, Ok(56.0)), TempState::Ok { celsius: 56.0 });
        assert!(
            matches!(t.push(62_000, Ok(19.9)), TempState::Unavailable { ref code, .. } if code == "out_of_range")
        );
        assert!(
            matches!(t.push(63_000, Ok(110.1)), TempState::Unavailable { ref code, .. } if code == "out_of_range")
        );
        assert_eq!(t.push(64_000, Ok(110.0)), TempState::Ok { celsius: 110.0 });
        assert_eq!(
            t.push(65_000, Err("Invalid class".into())),
            TempState::Unavailable {
                code: "no_sensor".into(),
                detail: "Invalid class".into()
            }
        );
    }

    #[test]
    fn sampling_slows_down_only_when_over_budget() {
        assert_eq!(next_interval_ms(1000, 1.5), 1000);
        assert_eq!(next_interval_ms(1000, 2.0), 1000);
        assert_eq!(next_interval_ms(1000, 2.1), 2000);
        assert_eq!(next_interval_ms(2000, 0.1), 2000);
    }

    #[test]
    fn spans_serialize_for_the_ui() {
        let v = serde_json::to_value(TempState::Ok { celsius: 50.0 }).unwrap();
        assert_eq!(v, serde_json::json!({"state": "ok", "celsius": 50.0}));
    }

    #[test]
    fn unavailable_and_span_serialize_for_the_ui() {
        let v = serde_json::to_value(TempState::Unavailable {
            code: "stuck".into(),
            detail: "55.0".into(),
        })
        .unwrap();
        assert_eq!(
            v,
            serde_json::json!({"state": "unavailable", "code": "stuck", "detail": "55.0"})
        );
        let s = find_stutters(&series("..ccc.."));
        let v = serde_json::to_value(&s[0]).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"start_ms": 2000, "end_ms": 5000, "first": 2, "last": 4, "cpu": true, "disk": false})
        );
    }

    #[test]
    fn clock_going_backwards_breaks_the_run_without_panic() {
        let mut p = series("cccccc");
        // Đồng hồ lùi 3 giây giữa chuỗi: không được coi là liền mạch.
        for x in &mut p[3..] {
            x.t_ms -= 3_000;
        }
        let s = find_stutters(&p);
        assert_eq!(s.len(), 2, "{s:?}");
        assert_eq!((s[0].last, s[1].first), (2, 3));
        let mut q = series("ccc.");
        q[1].t_ms = 500;
        assert!(find_stutters(&q).is_empty(), "mẫu lùi giờ làm đứt đoạn");
        // 12 mẫu, lùi 5 giây ở mẫu thứ 7 ⇒ hai đoạn 6 giây, không đoạn nào đủ 10 giây.
        let mut t = throttle_series(12, 95.0, Some(50.0));
        for x in &mut t[6..] {
            x.t_ms -= 5_000;
        }
        assert!(find_throttle(&t).is_empty());
        assert_eq!(throttled_now(&t), Some(false));
    }

    #[test]
    fn temperature_boundary_and_stuck_clock_restarts_after_error() {
        let mut t = TempTracker::default();
        assert_eq!(t.push(0, Ok(20.0)), TempState::Ok { celsius: 20.0 });
        let mut t = TempTracker::default();
        t.push(0, Ok(55.0));
        t.push(30_000, Err("x".into()));
        assert_eq!(t.push(31_000, Ok(55.0)), TempState::Ok { celsius: 55.0 });
        assert_eq!(t.push(90_000, Ok(55.0)), TempState::Ok { celsius: 55.0 });
        assert!(
            matches!(t.push(91_000, Ok(55.0)), TempState::Unavailable { ref code, .. } if code == "stuck")
        );
    }

    #[test]
    fn throttle_at_two_second_sampling() {
        let p = |n: u64| -> Vec<Point> {
            (0..n)
                .map(|i| Point {
                    t_ms: (i + 1) * 2000,
                    dur_ms: 2000,
                    cpu: 95.0,
                    disk: 0.0,
                    perf: Some(50.0),
                })
                .collect()
        };
        assert_eq!(find_throttle(&p(5)).len(), 1);
        assert!(find_throttle(&p(4)).is_empty());
    }

    #[test]
    fn missing_perf_in_the_middle_breaks_throttle() {
        let mut p = throttle_series(12, 95.0, Some(50.0));
        p[5].perf = None;
        assert!(find_throttle(&p).is_empty());
    }
}
