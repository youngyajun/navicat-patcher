//! 核心算法模块：与 Tauri 版 / Electron 版 / Java 版逐行对齐，
//! 不依赖任何 UI 框架，可被任意前端复用。

pub mod byte_utils;
pub mod patcher;
pub mod pe_file;
pub mod rsa;

/// 统一错误类型（Send + Sync 以便跨线程传递到 UI）
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
