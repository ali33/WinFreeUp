//! Lõi WinFreeUp: quét, dọn, luật an toàn, nhật ký. Không phụ thuộc Tauri, không chứa chữ hiển thị.

pub mod cleaners;
pub mod engine;
pub mod env;
pub mod error;
pub mod fsclean;
pub mod log;
pub mod safety;
pub mod service;
pub mod sys_windows;
pub mod types;

#[cfg(test)]
pub(crate) mod testutil;

pub use env::{Env, SystemOps};
pub use error::{CoreError, Result};
pub use types::*;
