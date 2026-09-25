//! Lời gọi Windows thật cho phần Tinh chỉnh. Mỗi mảng một file; `real::RealTweakOps` chỉ chuyển tiếp.
#![cfg(windows)]

pub mod appx;
pub mod info;
pub mod real;
pub mod registry;
pub mod services;
pub mod shell;
pub mod tasks;
