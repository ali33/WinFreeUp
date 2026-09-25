//! Tinh chỉnh Windows: gỡ app cài sẵn và tắt quảng cáo/thu thập dữ liệu.
//! Mọi thao tác chạm hệ thống đi qua trait `TweakOps`; lõi không chứa chữ hiển thị.

pub mod blocklist;
pub mod catalog;
pub mod engine;
pub mod model;
pub mod ops;
pub mod state;
pub mod sys_windows;
pub mod undo;

#[cfg(test)]
pub(crate) mod fake;

pub use model::{Group, Level, Op, RegType, RegValue, Restart, Risk, StartType, Tweak, WindowsReq};
pub use ops::{PackageInfo, RegData, SystemInfo, TweakOps};
