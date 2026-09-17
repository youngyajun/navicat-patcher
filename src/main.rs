// 隐藏 release 构建的控制台窗口，DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Navicat 17.3.x Patcher — egui 纯原生版本入口。
//!
//! 窗口尺寸 920×820（最小 880×720），平铺式布局，所有步骤始终展开。
//! 现代浅色风：浅灰蓝画布 + 纯白卡片 + 品牌蓝主色。

mod app;
mod core;
mod platform;
mod theme;

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([920.0, 820.0])
        .with_min_inner_size([880.0, 720.0]);
    if let Some(icon) = theme::load_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Navicat Patcher",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(app::PatcherApp::new(cc)))
        }),
    )
}
