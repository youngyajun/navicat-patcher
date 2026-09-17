//! 平台能力层：原生对话框、剪贴板、进程启动、路径与时间工具。
//!
//! 全部直接调用 Win32 API（经 rfd / arboard / std），零运行时依赖——
//! 对应 Electron 版 main/index.ts 中的 ipcMain.handle 系统能力部分。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 当前本地时间 HH:MM:SS（日志时间戳，对应前端 new Date() 格式化）
pub fn now_hms() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

/// 返回 dir 下的 libcc.dll 完整路径，不存在返回 None
pub fn libcc_path_in(dir: &str) -> Option<String> {
    let p = Path::new(dir).join("libcc.dll");
    if p.is_file() {
        Some(p.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// 取路径的父目录（对应 node:path 的 dirname）
pub fn dirname(p: &str) -> String {
    let parent = Path::new(p)
        .parent()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    // 对齐 node:path 行为：无父目录段时返回 "." 而非空串
    if parent.is_empty() { ".".to_string() } else { parent }
}

/// 启动 Navicat（对应 Java ProcessBuilder / Electron spawn detached）。
///
/// Windows 上 spawn 的子进程拥有独立生命周期，不随本工具退出
/// （等价 Node 的 detached + unref）。
pub fn launch_navicat(install_dir: &str) -> Result<String, String> {
    let exe = Path::new(install_dir).join("navicat.exe");
    if !exe.is_file() {
        return Err(format!(
            "在安装目录下未找到 navicat.exe:\n{install_dir}\n请确认目录是否正确。"
        ));
    }
    let exe_str = exe.display().to_string();
    Command::new(&exe)
        .current_dir(install_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 Navicat 失败: {e}"))?;
    Ok(format!("已启动: {exe_str}"))
}

// ── 原生对话框（rfd：Win32 IFileDialog COM API）──
// ⚠️ blocking 版本必须在非主线程调用（本项目的调用方均为后台线程）。

/// 选择目录对话框，取消返回 None（对应 dialog:select-directory）
pub fn pick_folder_blocking(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).pick_folder()
}

/// 选择 DLL 文件对话框，取消返回 None（对应 dialog:select-dll）
pub fn pick_dll_blocking(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("DLL 文件", &["dll"])
        .add_filter("所有文件", &["*"])
        .pick_file()
}

/// 写入剪贴板（arboard：Win32 API）
pub fn set_clipboard(clipboard: &mut Option<arboard::Clipboard>, text: &str) -> bool {
    match clipboard {
        Some(c) => c.set_text(text.to_string()).is_ok(),
        None => false,
    }
}
