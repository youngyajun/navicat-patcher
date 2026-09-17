//! egui 主界面：平铺四步向导 + 终端风日志 + Toast + 错误弹窗（现代浅色风）。
//!
//! 流程与状态机（对应 JavaFX AppUI.java）：
//!  - Step 1: 选择 Navicat 安装目录并自动备份 libcc.dll
//!  - Step 2: 生成 2048 位 RSA 密钥对（公钥/私钥左右分栏展示）
//!  - Step 3: 应用补丁
//!  - Step 4: 离线激活（复制密钥 → 启动 Navicat → 输入请求码 → 生成/复制激活码）
//!
//! 视觉设计（现代 · 统一 · 平铺不折叠）：
//!  - 浅灰蓝画布 + 纯白卡片 + 浅灰描边，卡片柔和阴影，圆角 8px；
//!  - 所有步骤始终展开，不做折叠/收起，步骤间用连接线串联；
//!  - 步骤徽标为圆角方形：蓝实心=进行中 / 绿描边=已完成 / 灰=未解锁；
//!  - 头部四段进度块 + "x / 4" 计数；日志为浅色终端，始终可见；
//!  - 按钮两种：主操作品牌蓝实心 / 次操作白底浅灰边（悬停变蓝）；
//!  - 仅浮层（弹窗/Toast）使用柔和阴影。
//!
//! 线程模型（对应 Electron 的异步 IPC）：
//!  - UI 线程只做绘制与事件分发，所有耗时操作（RSA 生成、PE 读写、
//!    原生对话框）在 std::thread 后台线程执行；
//!  - 后台线程通过 mpsc 通道回传 UiEvent，update() 每帧非阻塞收取；
//!  - 核心服务的日志回调即通道的 Log 事件，等价原来的 onLog 订阅。

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{
    Align, Align2, Button, Color32, FontId, Layout, RichText, Sense, TextEdit, TextStyle,
};

use crate::core::patcher::PatcherService;
use crate::platform;
use crate::theme::*;

/// 产品密钥（显示格式）—— 与各版本 PRODUCT_KEY_FORMATTED 一致
const PRODUCT_KEY: &str = "NAVM-IKCH-CWNI-HS3Q";
/// Toast 显示时长（对应 Vue 版 1600ms）
const TOAST_MS: u64 = 1600;
/// 表单标签固定宽度（保证各行输入框左对齐）
const LABEL_W: f32 = 64.0;
/// 密钥/PEM 文本框的最小行数
const KEY_ROWS: usize = 3;

/// 后台线程 → UI 线程事件
enum UiEvent {
    /// 核心服务日志
    Log(String),
    /// 目录选择对话框结果
    FolderPicked(Option<PathBuf>),
    /// DLL 选择对话框结果
    DllPicked(Option<PathBuf>),
    /// Step 1: 备份结果（备份路径）
    Backup(Result<String, String>),
    /// Step 2: 密钥对结果（公钥 PEM，私钥 PEM）
    Keys(Result<(String, String), String>),
    /// Step 3: 补丁结果（patchFileOffset, offsetFromRip）
    Patch(Result<(i64, i64), String>),
    /// Step 4: 激活码结果（Base64）
    Activation(Result<String, String>),
    /// 启动 Navicat 结果（日志消息）
    Launch(Result<String, String>),
}

/// 状态行：彩色圆点 + 文字（无胶囊底）
#[derive(Clone)]
struct Status(String, Color32);

impl Status {
    fn idle(text: &str) -> Self {
        Status(text.into(), DIM)
    }
    fn busy(text: &str) -> Self {
        Status(text.into(), WARN)
    }
    fn ok(text: String) -> Self {
        Status(text, OK)
    }
    fn err(text: &str) -> Self {
        Status(text.into(), ERR)
    }
}

/// 步骤状态：进行中（蓝）/ 已完成（绿）/ 未解锁（灰）
#[derive(Clone, Copy)]
enum StepState {
    Active,
    Done,
    Locked,
}

impl StepState {
    /// 步骤标题颜色
    fn title_color(self) -> Color32 {
        match self {
            StepState::Locked => DIM,
            _ => TEXT,
        }
    }
}

pub struct PatcherApp {
    service: Arc<PatcherService>,
    tx: Sender<UiEvent>,
    rx: Receiver<UiEvent>,
    /// egui Context：后台线程完成后 request_repaint 唤醒 UI
    ctx: egui::Context,
    clipboard: Option<arboard::Clipboard>,
    logo: Option<egui::TextureHandle>,

    // ── Step 1 状态 ──
    install_dir: String,
    dll_path: String,
    backup_path: String,
    backing_up: bool,
    step1_status: Status,
    step1_done: bool,
    picking_folder: bool,
    picking_dll: bool,

    // ── Step 2 状态 ──
    public_key_pem: String,
    private_key_pem: String,
    generating_keys: bool,
    step2_status: Status,
    step2_done: bool,

    // ── Step 3 状态 ──
    patching: bool,
    step3_status: Status,
    step3_done: bool,

    // ── Step 4 状态 ──
    request_code: String,
    username: String,
    organization: String,
    activation_code: String,
    generating_activation: bool,

    // ── 日志区 ──
    log_lines: Vec<String>,

    // ── Toast / 错误弹窗 ──
    toast: Option<(String, Instant)>,
    error_modal: Option<(String, String)>,
}

impl PatcherApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        // 核心服务日志回调：注入为通道 Log 事件（对应 webContents.send('patch:log')）
        let log_tx = tx.clone();
        let logger: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |line: &str| {
            let _ = log_tx.send(UiEvent::Log(line.to_string()));
        });
        let service = Arc::new(PatcherService::new(logger));

        let mut app = Self {
            service,
            tx,
            rx,
            ctx: cc.egui_ctx.clone(),
            clipboard: arboard::Clipboard::new().ok(),
            logo: load_logo(&cc.egui_ctx),

            install_dir: String::new(),
            dll_path: String::new(),
            backup_path: String::new(),
            backing_up: false,
            step1_status: Status::idle("待操作"),
            step1_done: false,
            picking_folder: false,
            picking_dll: false,

            public_key_pem: String::new(),
            private_key_pem: String::new(),
            generating_keys: false,
            step2_status: Status::idle("待操作"),
            step2_done: false,

            patching: false,
            step3_status: Status::idle("待操作"),
            step3_done: false,

            request_code: String::new(),
            username: String::new(),
            organization: String::new(),
            activation_code: String::new(),
            generating_activation: false,

            log_lines: Vec::new(),

            toast: None,
            error_modal: None,
        };
        app.log("=== Navicat 激活补丁工具已启动 ===");
        app.log("提示: 请先断网，然后选择 Navicat 安装目录开始操作。");
        app
    }

    // ═══════════════════════════════════════════════════════════
    //  通用工具
    // ═══════════════════════════════════════════════════════════

    fn log(&mut self, msg: &str) {
        self.log_lines.push(format!("[{}] {msg}", platform::now_hms()));
    }

    /// 是否有后台操作进行中（决定是否持续重绘以收取事件）
    fn busy(&self) -> bool {
        self.backing_up
            || self.generating_keys
            || self.patching
            || self.generating_activation
            || self.picking_folder
            || self.picking_dll
    }

    /// 对应 Java String.format("0x%X")：按无符号大写十六进制显示（i64→u32 回绕同 JS >>>0）
    fn hex(n: i64) -> String {
        format!("0x{:X}", n as u32)
    }

    /// Step 4 是否已完成（激活码已成功生成）
    fn step4_done(&self) -> bool {
        !self.activation_code.is_empty()
            && self.activation_code != "正在生成激活码..."
            && self.activation_code != "生成失败！"
    }

    /// 错误弹窗（对应 Java showError 的两个重载）
    fn show_error(&mut self, header_or_message: &str, err: Option<&str>) {
        match err {
            Some(e) => {
                self.log(&format!("错误: {e}"));
                self.error_modal = Some((header_or_message.to_string(), e.to_string()));
            }
            None => {
                self.error_modal = Some(("错误".to_string(), header_or_message.to_string()));
            }
        }
    }

    /// Toast（对应 showToast：顶部中央短暂提示后消失）
    fn show_toast(&mut self, message: &str) {
        self.toast = Some((
            message.to_string(),
            Instant::now() + Duration::from_millis(TOAST_MS),
        ));
    }

    /// 在后台线程执行闭包，结果经 mpsc 通道回传（对应 Electron 的 async ipcMain.handle）
    fn spawn_bg<T, F>(&self, make_event: F)
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
        UiEvent: From<T>,
    {
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let event = make_event().into();
            let _ = tx.send(event);
            ctx.request_repaint();
        });
    }

    // ═══════════════════════════════════════════════════════════
    //  事件处理（后台线程结果 → UI 状态机）
    // ═══════════════════════════════════════════════════════════

    fn on_event(&mut self, ev: UiEvent) {
        match ev {
            UiEvent::Log(line) => self.log(&line),

            // ── Step 1：目录选择 ──
            UiEvent::FolderPicked(None) => self.picking_folder = false,
            UiEvent::FolderPicked(Some(dir)) => {
                self.picking_folder = false;
                let dir_s = dir.display().to_string();
                self.install_dir = dir_s.clone();
                self.log(&format!("已选择 Navicat 安装目录: {dir_s}"));
                match platform::libcc_path_in(&dir_s) {
                    Some(libcc) => {
                        self.log(&format!("已找到 libcc.dll: {libcc}"));
                        self.dll_path = libcc.clone();
                        self.auto_backup(&libcc);
                    }
                    None => {
                        self.log("在安装目录下未找到 libcc.dll，请手动选择。");
                        self.step1_status =
                            Status::busy("未找到 libcc.dll，请手动选择");
                        self.step1_done = false;
                    }
                }
            }

            // ── Step 1：手动选择 DLL ──
            UiEvent::DllPicked(None) => self.picking_dll = false,
            UiEvent::DllPicked(Some(picked)) => {
                self.picking_dll = false;
                let dll_s = picked.display().to_string();
                self.log(&format!("已选择 libcc.dll: {dll_s}"));
                self.dll_path = dll_s.clone();
                // libcc.dll 所在目录即为安装目录，自动回填
                let dir = platform::dirname(&dll_s);
                self.log(&format!("已自动回填安装目录: {dir}"));
                self.install_dir = dir;
                self.auto_backup(&dll_s);
            }

            // ── Step 1：备份结果 ──
            UiEvent::Backup(Ok(backup)) => {
                self.backing_up = false;
                self.backup_path = backup.clone();
                self.step1_status = Status::ok("已备份".into());
                self.step1_done = true;
                self.log(&format!("Step 1 完成: libcc.dll 已备份 → {backup}"));
            }
            UiEvent::Backup(Err(e)) => {
                self.backing_up = false;
                self.step1_status = Status::err("备份失败".into());
                self.show_error("自动备份失败", Some(&e));
            }

            // ── Step 2：密钥对结果 ──
            UiEvent::Keys(Ok((pub_pem, priv_pem))) => {
                self.generating_keys = false;
                self.public_key_pem = pub_pem;
                self.private_key_pem = priv_pem;
                self.step2_status = Status::ok("已生成".into());
                self.step2_done = true;
                self.log("Step 2 完成: RSA 密钥对已生成");
            }
            UiEvent::Keys(Err(e)) => {
                self.generating_keys = false;
                self.step2_status = Status::err("生成失败".into());
                self.show_error("密钥生成失败", Some(&e));
            }

            // ── Step 3：补丁结果 ──
            UiEvent::Patch(Ok((offset, rip))) => {
                self.patching = false;
                self.step3_status = Status::ok(format!(
                    "已应用（偏移 {}，RIP {}）",
                    Self::hex(offset),
                    Self::hex(rip)
                ));
                self.step3_done = true;
                self.log("Step 3 完成: 补丁已应用");
            }
            UiEvent::Patch(Err(e)) => {
                self.patching = false;
                self.step3_status = Status::err("应用失败".into());
                self.show_error("补丁应用失败", Some(&e));
            }

            // ── Step 4：激活码结果 ──
            UiEvent::Activation(Ok(code)) => {
                self.generating_activation = false;
                self.activation_code = code;
                self.log("Step 4 完成: 激活码已生成");
                self.log("请复制激活码到 Navicat 激活窗口完成激活。");
            }
            UiEvent::Activation(Err(e)) => {
                self.generating_activation = false;
                self.activation_code = "生成失败！".to_string();
                self.show_error("激活码生成失败", Some(&e));
            }

            // ── 启动 Navicat ──
            UiEvent::Launch(Ok(msg)) => self.log(&msg),
            UiEvent::Launch(Err(e)) => {
                self.log(&format!("启动 Navicat 失败: {e}"));
                self.show_error("启动 Navicat 失败", Some(&e));
            }
        }
    }

    // ═══════════════════════════════════════════════════════════
    //  动作（按钮点击 → 后台线程）
    // ═══════════════════════════════════════════════════════════

    fn browse_install_dir(&mut self) {
        self.picking_folder = true;
        self.spawn_bg(|| {
            UiEvent::FolderPicked(platform::pick_folder_blocking("选择 Navicat 安装目录"))
        });
    }

    fn browse_dll_file(&mut self) {
        self.picking_dll = true;
        self.spawn_bg(|| UiEvent::DllPicked(platform::pick_dll_blocking("选择 libcc.dll")));
    }

    fn auto_backup(&mut self, dll: &str) {
        self.step1_status = Status::busy("正在备份...");
        self.backing_up = true;
        let svc = self.service.clone();
        let dll = dll.to_string();
        self.spawn_bg(move || {
            let r = svc.backup_file(&dll).map_err(|e| e.to_string());
            UiEvent::Backup(r)
        });
    }

    fn do_generate_key(&mut self) {
        self.generating_keys = true;
        self.step2_status = Status::busy("正在生成...");
        let svc = self.service.clone();
        self.spawn_bg(move || {
            let r = svc
                .generate_key_pair()
                .map(|i| (i.public_key_pem, i.private_key_pem))
                .map_err(|e| e.to_string());
            UiEvent::Keys(r)
        });
    }

    fn do_apply_patch(&mut self) {
        if self.backup_path.is_empty() {
            self.show_error("请先完成 Step 1 备份", None);
            return;
        }
        let output = self.dll_path.trim().to_string();
        if output.is_empty() {
            self.show_error("未指定 libcc.dll 路径", None);
            return;
        }
        self.patching = true;
        self.step3_status = Status::busy("正在应用补丁...");
        let svc = self.service.clone();
        let backup = self.backup_path.clone();
        self.spawn_bg(move || {
            let r = svc
                .apply_patch(&backup, &output)
                .map(|i| (i.patch_file_offset, i.offset_from_rip))
                .map_err(|e| e.to_string());
            UiEvent::Patch(r)
        });
    }

    fn copy_product_key(&mut self) {
        if platform::set_clipboard(&mut self.clipboard, PRODUCT_KEY) {
            self.show_toast("密钥复制成功");
            self.log("产品密钥已复制到剪贴板");
        }
    }

    fn launch_navicat(&mut self) {
        let dir = self.install_dir.trim().to_string();
        if dir.is_empty() {
            self.show_error("请先在 Step 1 中选择 Navicat 安装目录", None);
            return;
        }
        self.spawn_bg(move || UiEvent::Launch(platform::launch_navicat(&dir)));
    }

    fn do_generate_activation(&mut self) {
        let request = self.request_code.trim().to_string();
        let username = self.username.trim().to_string();
        let organization = self.organization.trim().to_string();

        if request.is_empty() {
            self.show_error("请输入请求码", None);
            return;
        }
        if username.is_empty() {
            self.show_error("请输入用户名", None);
            return;
        }
        if organization.is_empty() {
            self.show_error("请输入组织名", None);
            return;
        }

        self.generating_activation = true;
        self.activation_code = "正在生成激活码...".to_string();
        let svc = self.service.clone();
        self.spawn_bg(move || {
            let r = svc
                .generate_activation_code(&request, &username, &organization)
                .map_err(|e| e.to_string());
            UiEvent::Activation(r)
        });
    }

    fn copy_activation_code(&mut self) {
        let code = self.activation_code.clone();
        if code.is_empty() || code == "正在生成激活码..." || code == "生成失败！" {
            self.show_error("暂无激活码可复制", None);
            return;
        }
        if platform::set_clipboard(&mut self.clipboard, &code) {
            self.show_toast("激活码复制成功");
            self.log("激活码已复制到剪贴板");
        }
    }

    // ═══════════════════════════════════════════════════════════
    //  绘制
    // ═══════════════════════════════════════════════════════════

    fn paint(&mut self, ctx: &egui::Context) {
        // ── 顶部标题栏：logo + 标题 + 版本徽章 + 断网警示 + 四段进度块 ──
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::default()
                    .fill(APP_BG)
                    .inner_margin(egui::Margin::symmetric(18.0, 12.0)),
            )
            .show(ctx, |ui| {
                // 顶部行：logo + 标题 + 版本徽章 …… 右侧断网警示徽章
                ui.horizontal(|ui| {
                    if let Some(tex) = &self.logo {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(26.0, 26.0), Sense::hover());
                        let img = egui::Rect::from_center_size(
                            rect.center(),
                            egui::vec2(26.0, 26.0),
                        );
                        ui.painter().image(
                            tex.id(),
                            img,
                            egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            ),
                            Color32::WHITE,
                        );
                    }
                    ui.add_space(10.0);
                    ui.label(RichText::new("Navicat 激活补丁工具").strong().size(16.0));
                    ui.add_space(8.0);
                    // 版本徽章
                    egui::Frame::default()
                        .fill(CARD_SUBTLE)
                        .stroke(egui::Stroke::new(1.0_f32, BORDER))
                        .rounding(9.0)
                        .inner_margin(egui::Margin::symmetric(8.0, 2.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new("v17.3.x").color(DIM).size(10.5));
                        });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // 断网警示徽章：浅橙底 + 橙字
                        egui::Frame::default()
                            .fill(Color32::from_rgb(0xFF, 0xF4, 0xE2))
                            .stroke(egui::Stroke::new(
                                1.0_f32,
                                Color32::from_rgb(0xFC, 0xE2, 0xB4),
                            ))
                            .rounding(9.0)
                            .inner_margin(egui::Margin::symmetric(10.0, 3.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("⚠ 请先断网，并关闭 Navicat 进程")
                                        .color(WARN)
                                        .size(11.0),
                                );
                            });
                    });
                });
                ui.add_space(13.0);
                self.progress_blocks(ui);
                ui.add_space(6.0);
                // 轻量分隔线
                let rect = ui.min_rect();
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.left(), rect.bottom() - 1.0),
                        egui::pos2(rect.right(), rect.bottom() - 1.0),
                    ],
                    egui::Stroke::new(1.0_f32, BORDER),
                );
            });

        // ── 底部日志面板：始终展开，拖动上边缘可调整高度 ──
        let max_log_h = (ctx.screen_rect().height() * 0.4).max(80.0);
        egui::TopBottomPanel::bottom("log-pane")
            .frame(
                egui::Frame::default()
                    .fill(APP_BG)
                    .inner_margin(egui::Margin::symmetric(16.0, 6.0)),
            )
            .resizable(true)
            .default_height(100.0)
            .height_range(40.0..=max_log_h)
            .show(ctx, |ui| self.log_pane(ui));

        // ── 中央：平铺四步卡片（始终展开，ScrollArea 兜底）──
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(APP_BG)
                    .inner_margin(egui::Margin::symmetric(16.0, 8.0)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("steps-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.step1(ui);
                        ui.add_space(6.0);
                        self.step2(ui);
                        ui.add_space(6.0);
                        self.step3(ui);
                        ui.add_space(6.0);
                        self.step4(ui);
                        ui.add_space(4.0);
                    });
            });

        // ── 覆盖层：Toast 与错误弹窗 ──
        self.paint_toast(ctx);
        self.paint_modal(ctx);
    }

    /// 头部四段进度块：已完成=品牌蓝 / 未完成=浅灰轨道，右侧 "x / 4"
    fn progress_blocks(&mut self, ui: &mut egui::Ui) {
        let steps = [
            self.step1_done,
            self.step2_done,
            self.step3_done,
            self.step4_done(),
        ];
        let done = steps.iter().filter(|s| **s).count();

        ui.horizontal(|ui| {
            // 右侧：完成计数
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(format!("{done} / 4")).color(DIM).size(11.0));
                ui.add_space(10.0);
            });
            // 四段进度滑块（留出计数空间）
            let total = (ui.available_width() - 44.0).max(60.0);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(total, 8.0), Sense::hover());
            let gap = 6.0;
            let seg_w = (rect.width() - gap * 3.0) / 4.0;
            let h = 7.0;
            let y = rect.min.y + (rect.height() - h) * 0.5;
            for i in 0..4 {
                let x0 = rect.min.x + i as f32 * (seg_w + gap);
                let seg = egui::Rect::from_min_max(
                    egui::pos2(x0, y),
                    egui::pos2(x0 + seg_w, y + h),
                );
                // 圆角轨道 + 已完成填充
                ui.painter().rect_filled(seg, h * 0.5, TRACK);
                if steps[i] {
                    ui.painter().rect_filled(seg, h * 0.5, ACCENT);
                }
            }
        });
    }

    /// 步骤卡片容器：白色圆角卡片 + 浅灰细描边 + 柔和阴影
    fn step_frame(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
        egui::Frame::default()
            .fill(CARD)
            .rounding(10.0)
            .stroke(egui::Stroke::new(1.0_f32, BORDER))
            .shadow(hard_shadow())
            .inner_margin(egui::Margin { left: 16.0, right: 16.0, top: 14.0, bottom: 14.0 })
            .show(ui, add);
    }

    /// 输入框容器实现：关闭 TextEdit 自带边框，由外层 Frame 统一自绘：
    ///  - 可编辑：白底 + 灰蓝描边
    ///  - 只读（readonly）：浅灰蓝底 + 灰蓝描边，与可编辑态区分
    ///  - 聚焦：描边加粗并转为品牌蓝
    ///  - 传入 `outer_height` 时高度锁定，内容超出则在框内滚动
    ///
    /// 返回 (内部输入框响应, 外框实际高度)。
    fn field_impl<'a>(
        ui: &mut egui::Ui,
        edit: TextEdit<'a>,
        readonly: bool,
        fill_width: bool,
        outer_height: Option<f32>,
        id_salt: &str,
    ) -> (egui::Response, f32) {
        let radius: egui::Rounding = 8.0.into();
        let pad = egui::Margin::symmetric(8.0, 5.0);
        let fill = if readonly { CARD_SUBTLE } else { CARD };

        let outer = egui::Frame::default()
            .fill(fill)
            .stroke(egui::Stroke::new(1.0_f32, FIELD_BORDER))
            .rounding(radius)
            .inner_margin(pad)
            .show(ui, |ui| {
                let mut edit = edit.frame(false).margin(egui::Margin::ZERO);
                if fill_width {
                    edit = edit.desired_width(ui.available_width());
                }
                match outer_height {
                    // 锁定高度：可视区裁到 inner_h，内容超出部分在框内滚动
                    Some(h) => {
                        let row_h = ui.text_style_height(&TextStyle::Monospace);
                        let inner_h = (h - pad.sum().y).max(row_h);
                        let size = egui::vec2(ui.available_width(), inner_h);
                        ui.allocate_ui(size, |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt(id_salt)
                                .auto_shrink([false, true])
                                .show(ui, |ui| ui.add(edit.desired_width(ui.available_width())))
                                .inner
                        })
                        .inner
                    }
                    None => ui.add(edit),
                }
            });

        let inner = outer.inner;
        let outer_h = outer.response.rect.height();
        if inner.has_focus() {
            ui.painter().rect_stroke(
                outer.response.rect.expand(1.0),
                radius,
                egui::Stroke::new(1.5_f32, ACCENT),
            );
        }
        (inner, outer_h)
    }

    /// 表单标签：固定宽度，保证各行输入框左对齐
    fn field_label(ui: &mut egui::Ui, text: &str) {
        ui.add_sized(
            [LABEL_W, 18.0],
            egui::Label::new(RichText::new(text).color(DIM).size(12.0)),
        );
    }

    /// 统一输入框容器：清晰可见的边框 + 圆角，聚焦时描边转品牌蓝。
    ///
    /// 背景：egui 0.29 中"可编辑" TextEdit 的边框取 `ui.style().interact(..).bg_stroke`，
    /// 在纯白卡片上实际渲染不可见（只有 `interactive(false)` 的只读框才有边框）。
    /// 因此关闭 TextEdit 自带边框，由外层 Frame 统一自绘：
    ///  - 可编辑：白底 + 灰蓝描边
    ///  - 只读（readonly）：浅灰蓝底 + 灰蓝描边，与可编辑态区分
    ///  - 聚焦：描边加粗并转为品牌蓝
    ///
    /// `fill_width=true` 时输入框撑满可用宽度（在 Frame 内部测量，已扣除内边距）。
    /// 输入框容器（自适应高度）：内容多高，框就多高。
    fn field<'a>(
        ui: &mut egui::Ui,
        edit: TextEdit<'a>,
        readonly: bool,
        fill_width: bool,
    ) -> egui::Response {
        Self::field_impl(ui, edit, readonly, fill_width, None, "field").0
    }

    /// 输入框容器（锁定高度）：外框高度固定为 `outer_height`，超出部分在框内滚动。
    /// 用于左右分栏需等高、且内容可能很长的场景（如私钥 PEM 对齐公钥高度）。
    fn field_sized<'a>(
        ui: &mut egui::Ui,
        edit: TextEdit<'a>,
        readonly: bool,
        outer_height: f32,
        id_salt: &str,
    ) -> egui::Response {
        Self::field_impl(ui, edit, readonly, true, Some(outer_height), id_salt).0
    }

    /// 步骤徽标：26px 圆角方形（蓝实心=进行中/绿=已完成/灰=未解锁）
    /// 进行中时外圈有一抹品牌浅色光晕，提示当前所处步骤。
    fn step_badge(ui: &mut egui::Ui, num: &str, state: StepState) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), Sense::hover());
        let (fill, edge, fg) = match state {
            StepState::Active => (ACCENT, ACCENT, CREAM_TEXT),
            StepState::Done => (OK, OK, CREAM_TEXT),
            StepState::Locked => (CARD_SUBTLE, BORDER, PLACEHOLDER),
        };
        if matches!(state, StepState::Active) {
            ui.painter().rect_filled(rect.expand(4.0), 10.0, ACCENT_TINT);
        }
        ui.painter().rect_filled(rect, 8.0, fill);
        ui.painter()
            .rect_stroke(rect, 8.0, egui::Stroke::new(1.0_f32, edge));
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            num,
            FontId::proportional(13.0),
            fg,
        );
    }

    /// 状态胶囊：浅色底 + 语义色文字（进行/完成/失败/待操作）
    fn status_pill(ui: &mut egui::Ui, status: &Status) {
        egui::Frame::default()
            .fill(status.1.gamma_multiply(0.12))
            .rounding(7.0)
            .inner_margin(egui::Margin { left: 9.0, right: 9.0, top: 2.0, bottom: 2.0 })
            .show(ui, |ui| {
                ui.label(RichText::new(&status.0).color(status.1).size(11.0));
            });
    }

    /// 卡片头（单行）：[方形徽标] 标题 …… [状态胶囊]
    /// 所有步骤始终展开，无折叠箭头。
    fn step_header(
        ui: &mut egui::Ui,
        num: &str,
        title: &str,
        state: StepState,
        status: &Status,
    ) {
        ui.horizontal(|ui| {
            Self::step_badge(ui, num, state);
            ui.add_space(10.0);
            ui.label(RichText::new(title).strong().size(13.5).color(state.title_color()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // 状态常显（提示"正在备份…"等后台进度），以胶囊呈现
                Self::status_pill(ui, status);
            });
        });
    }

    /// 动作按钮（30px 高、8px 圆角），两种视觉层级：
    ///  - primary   ：品牌蓝实心 + 白色文字 —— 每步的唯一主操作（生成密钥 / 应用补丁 / 生成激活码 / 复制激活码）
    ///  - secondary ：白底 + 浅灰蓝描边 + 深灰文字，悬停转浅蓝底蓝字 —— 浏览 / 复制密钥 / 启动等次操作
    /// 禁用态统一为浅灰底 + 灰字，明确"当前不可点"。
    ///
    /// 关键：egui 0.29 的 Button 以 `weak_bg_fill` 作为背景填充（不是 `bg_fill`），
    /// 只想改颜色却漏掉该字段，按钮就会退回默认灰底 —— 这正是此前"按钮没有颜色"的原因。
    /// 因此三态的 bg_fill / weak_bg_fill 都显式覆盖，并在 add 后立即恢复样式避免泄漏。
    fn action_button(ui: &mut egui::Ui, label: &str, enabled: bool, primary: bool) -> egui::Response {
        let radius: egui::Rounding = 8.0.into();
        let button = Button::new(RichText::new(label).strong().size(12.5))
            .min_size(egui::vec2(0.0, 30.0))
            .rounding(radius);

        // 样式在本次 add 后恢复，避免污染后续控件
        let saved = ui.style().visuals.clone();

        // ── 禁用态：低饱和浅灰，与可用态形成清晰对比 ──
        // 同时覆盖全部四个状态，确保无论 egui 选中哪一个都呈现"不可点"外观。
        if !enabled || !ui.is_enabled() {
            {
                let v = &mut ui.visuals_mut().widgets;
                for s in [
                    &mut v.noninteractive,
                    &mut v.inactive,
                    &mut v.hovered,
                    &mut v.active,
                ] {
                    s.bg_fill = BTN_DISABLED_BG;
                    s.weak_bg_fill = BTN_DISABLED_BG;
                    s.bg_stroke = egui::Stroke::new(1.0_f32, BTN_DISABLED_BORDER);
                    s.fg_stroke = egui::Stroke::new(1.0_f32, BTN_DISABLED_TEXT);
                    s.rounding = radius;
                    s.expansion = 0.0;
                }
            }
            let resp = ui.add_enabled(false, button);
            ui.style_mut().visuals = saved;
            return resp;
        }

        // ── 启用态：静止 / 悬停 / 按下 三态配色 ──
        let (rest_bg, rest_edge, rest_fg) = if primary {
            (ACCENT, ACCENT, CREAM_TEXT)
        } else {
            (CARD, FIELD_BORDER, TEXT)
        };
        let (hover_bg, hover_edge, hover_fg) = if primary {
            (ACCENT_HOVER, ACCENT_HOVER, CREAM_TEXT)
        } else {
            (ACCENT_TINT, ACCENT, ACCENT_PRESS)
        };
        let (press_bg, press_edge, press_fg) = if primary {
            (ACCENT_PRESS, ACCENT_PRESS, CREAM_TEXT)
        } else {
            (ACCENT_TINT_DEEP, ACCENT_PRESS, ACCENT_PRESS)
        };

        {
            let v = &mut ui.visuals_mut().widgets;
            // (状态, 底色, 描边, 文字, 悬停/按下时的外扩量)
            let states: [(&mut egui::style::WidgetVisuals, Color32, Color32, Color32, f32); 3] = [
                (&mut v.inactive, rest_bg, rest_edge, rest_fg, 0.0),
                (&mut v.hovered, hover_bg, hover_edge, hover_fg, 0.5),
                (&mut v.active, press_bg, press_edge, press_fg, 0.5),
            ];
            for (s, bg, edge, fg, expansion) in states {
                s.bg_fill = bg;
                s.weak_bg_fill = bg;
                s.bg_stroke = egui::Stroke::new(1.0_f32, edge);
                s.fg_stroke = egui::Stroke::new(1.0_f32, fg);
                s.rounding = radius;
                s.expansion = expansion;
            }
        }
        let response = ui.add(button);
        ui.style_mut().visuals = saved;
        response
    }

    /// 不做锁定：所有步骤的控件与按钮始终可用（层级由按钮主/次样式表达）。
    fn lock_if(_ui: &mut egui::Ui, _unlocked: bool) {}

    // ── Step 1 ──
    fn step1(&mut self, ui: &mut egui::Ui) {
        let state = if self.step1_done {
            StepState::Done
        } else {
            StepState::Active
        };
        let status = self.step1_status.clone();
        Self::step_frame(ui, |ui| {
            Self::step_header(ui, "1", "选择目录并备份", state, &status);
            ui.add_space(8.0);

            Self::lock_if(ui, true); // Step 1 始终解锁

            // 安装目录行：[标签][输入框 填充][选择目录]
            let mut pick_dir = false;
            ui.horizontal(|ui| {
                Self::field_label(ui, "安装目录");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let can = !self.backing_up && !self.picking_folder;
                    if Self::action_button(ui, "选择目录", can, false).clicked() {
                        pick_dir = true;
                    }
                    Self::field(
                        ui,
                        TextEdit::singleline(&mut self.install_dir)
                            .hint_text("Navicat 安装目录"),
                        false,
                        true,
                    );
                });
            });
            if pick_dir {
                self.browse_install_dir();
            }

            ui.add_space(5.0);

            // DLL 路径行：[标签][输入框 填充][手动选择]
            let mut pick_dll = false;
            ui.horizontal(|ui| {
                Self::field_label(ui, "DLL 路径");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let can = !self.backing_up && !self.picking_dll;
                    if Self::action_button(ui, "手动选择", can, false).clicked() {
                        pick_dll = true;
                    }
                    Self::field(
                        ui,
                        TextEdit::singleline(&mut self.dll_path)
                            .hint_text("选择目录后自动填充，也可手动选择"),
                        false,
                        true,
                    );
                });
            });
            if pick_dll {
                self.browse_dll_file();
            }
        });
    }

    // ── Step 2 ──
    fn step2(&mut self, ui: &mut egui::Ui) {
        let state = if self.step2_done {
            StepState::Done
        } else if self.step1_done {
            StepState::Active
        } else {
            StepState::Locked
        };
        // 未解锁时头部状态显示"未解锁"，否则显示实际状态
        let status = if self.step1_done {
            self.step2_status.clone()
        } else {
            Status::idle("未解锁")
        };
        Self::step_frame(ui, |ui| {
            Self::step_header(ui, "2", "生成密钥对", state, &status);
            ui.add_space(8.0);

            Self::lock_if(ui, self.step1_done);

            if Self::action_button(ui, "生成 2048 位 RSA 密钥对", !self.generating_keys, true)
                .clicked()
            {
                self.do_generate_key();
            }
            ui.add_space(6.0);

            // 公钥/私钥左右分栏：私钥框高度对齐公钥，超出部分在框内滚动
            ui.columns(2, |cols| {
                let (left, right) = cols.split_at_mut(1);
                let height = Self::key_col(&mut left[0], "公钥 (PEM)", &mut self.public_key_pem);
                Self::key_col_sized(&mut right[0], "私钥 (PEM)", &mut self.private_key_pem, height);
            });
        });
    }

    /// 密钥文本框构造：只读等宽多行（生成后显示，最少 3 行）
    fn key_edit(buf: &mut String) -> TextEdit<'_> {
        TextEdit::multiline(buf)
            .desired_rows(KEY_ROWS)
            .font(TextStyle::Monospace)
            .interactive(false)
            .hint_text("生成后显示")
    }

    /// 密钥栏：小标题 + 只读等宽多行框，返回框体实际高度（供右栏对齐）
    fn key_col(ui: &mut egui::Ui, title: &str, buf: &mut String) -> f32 {
        ui.label(RichText::new(title).color(DIM).size(11.5));
        Self::field_impl(ui, Self::key_edit(buf), true, true, None, title).1
    }

    /// 密钥栏（等高版）：高度锁定为 `height`，内容超出部分在框内滚动
    fn key_col_sized(ui: &mut egui::Ui, title: &str, buf: &mut String, height: f32) {
        ui.label(RichText::new(title).color(DIM).size(11.5));
        Self::field_sized(ui, Self::key_edit(buf), true, height, title);
    }

    // ── Step 3 ──
    fn step3(&mut self, ui: &mut egui::Ui) {
        let state = if self.step3_done {
            StepState::Done
        } else if self.step2_done {
            StepState::Active
        } else {
            StepState::Locked
        };
        let status = if self.step2_done {
            self.step3_status.clone()
        } else {
            Status::idle("未解锁")
        };
        Self::step_frame(ui, |ui| {
            Self::step_header(ui, "3", "应用补丁", state, &status);
            ui.add_space(8.0);

            Self::lock_if(ui, self.step2_done);

            if Self::action_button(ui, "应用补丁到 libcc.dll", !self.patching, true).clicked() {
                self.do_apply_patch();
            }
        });
    }

    // ── Step 4 ──
    fn step4(&mut self, ui: &mut egui::Ui) {
        let step4_ready = !self.request_code.trim().is_empty()
            && !self.username.trim().is_empty()
            && !self.organization.trim().is_empty();
        let activation_ready = self.step4_done();

        // 头部状态即时派生
        let status = if self.step4_done() {
            Status::ok("激活码已生成".into())
        } else if self.generating_activation {
            Status::busy("正在生成...")
        } else if self.activation_code == "生成失败！" {
            Status::err("生成失败".into())
        } else if self.step3_done {
            Status::idle("待操作")
        } else {
            Status::idle("未解锁")
        };
        let state = if self.step4_done() {
            StepState::Done
        } else if self.step3_done {
            StepState::Active
        } else {
            StepState::Locked
        };

        Self::step_frame(ui, |ui| {
            Self::step_header(ui, "4", "离线激活", state, &status);
            ui.add_space(8.0);

            Self::lock_if(ui, self.step3_done);

            // 警示行：红字断网提示
            ui.label(RichText::new("⚠ 请断网后操作").color(ERR).size(11.5));
            ui.add_space(6.0);

            // 密钥行：[标签][密钥 代码块] …… 右侧 [复制密钥][启动 Navicat]
            ui.horizontal(|ui| {
                Self::field_label(ui, "产品密钥");
                egui::Frame::default()
                    .fill(CARD_SUBTLE)
                    .stroke(egui::Stroke::new(1.0_f32, BORDER))
                    .rounding(6.0)
                    .inner_margin(egui::Margin::symmetric(8.0, 3.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(PRODUCT_KEY).monospace().strong().size(12.5),
                        );
                    });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if Self::action_button(ui, "启动 Navicat", true, false).clicked() {
                        self.launch_navicat();
                    }
                    if Self::action_button(ui, "复制密钥", true, false).clicked() {
                        self.copy_product_key();
                    }
                });
            });
            ui.add_space(6.0);

            // 请求码：占位提示即说明
            Self::field(
                ui,
                TextEdit::multiline(&mut self.request_code)
                    .desired_rows(3)
                    .font(TextStyle::Monospace)
                    .hint_text("粘贴 Navicat 离线激活窗口中的请求码"),
                false,
                true,
            );
            ui.add_space(5.0);

            // 用户名 / 组织名
            ui.horizontal(|ui| {
                Self::field_label(ui, "用户名");
                let w = ui.available_width() * 0.35;
                Self::field(
                    ui,
                    TextEdit::singleline(&mut self.username)
                        .desired_width(w)
                        .hint_text("必填"),
                    false,
                    false,
                );
                ui.label(RichText::new("组织名").color(DIM).size(12.0));
                Self::field(
                    ui,
                    TextEdit::singleline(&mut self.organization).hint_text("必填"),
                    false,
                    true,
                );
            });
            ui.add_space(6.0);

            // 生成激活码 + 提示
            ui.horizontal(|ui| {
                let can = step4_ready && !self.generating_activation;
                if Self::action_button(ui, "生成激活码", can, true).clicked() {
                    self.do_generate_activation();
                }
                ui.add_space(5.0);
                ui.label(
                    RichText::new(if step4_ready {
                        "点击生成"
                    } else {
                        "请先填写请求码、用户名和组织名"
                    })
                    .color(if step4_ready { OK } else { PLACEHOLDER })
                    .size(11.5),
                );
            });
            ui.add_space(5.0);

            // 激活码：[标签][只读框 填充][复制激活码]
            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                if Self::action_button(ui, "复制激活码", activation_ready, true).clicked() {
                    self.copy_activation_code();
                }
                // 右到左布局中标签最后放置：必须预留其宽度（LABEL_W）+ 行间距 + 容器左右内边距，
                // 否则框按剩余全宽填充会把标签挤出卡片可视区，只露出最后一个"码"字
                let box_w = (ui.available_width() - LABEL_W - 24.0).max(120.0);
                Self::field(
                    ui,
                    TextEdit::multiline(&mut self.activation_code)
                        .desired_rows(3)
                        .desired_width(box_w)
                        .font(TextStyle::Monospace)
                        .interactive(false)
                        .hint_text("激活码生成后显示"),
                    true,
                    false,
                );
                Self::field_label(ui, "激活码");
            });
        });
    }

    // ── 日志面板：浅色终端（浅灰底 + 中灰等宽字），始终展开 ──
    fn log_pane(&mut self, ui: &mut egui::Ui) {
        egui::Frame::default()
            .fill(LOG_BG)
            .rounding(8.0)
            .stroke(egui::Stroke::new(1.0_f32, LOG_BORDER))
            .inner_margin(egui::Margin::same(10.0))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    // 头部行：终端圆点标题 · 提示
                    ui.horizontal(|ui| {
                        ui.add_space(2.0);
                        let (d, _) = ui.allocate_exact_size(egui::vec2(7.0, 7.0), Sense::hover());
                        ui.painter().circle_filled(d.center(), 3.5, ACCENT);
                        ui.add_space(6.0);
                        ui.label(RichText::new("运行日志").color(LOG_TEXT).strong().size(12.0));
                        ui.label(
                            RichText::new("· 拖动上边缘可调整高度")
                                .color(LOG_DIM)
                                .size(10.5),
                        );
                    });

                    ui.add_space(5.0);
                    // 内容区：等宽字体 + 自动滚动
                    egui::ScrollArea::vertical()
                        .id_salt("log-scroll")
                        .stick_to_bottom(true)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.style_mut().override_font_id = Some(FontId::monospace(11.0));
                            ui.style_mut().visuals.override_text_color = Some(LOG_TEXT);
                            ui.set_min_width(ui.available_width());
                            for line in &self.log_lines {
                                ui.label(line.clone());
                            }
                        });
                });
            });
    }

    // ── Toast：顶部中央白色纸片（浅描边 + 柔和阴影）──
    fn paint_toast(&mut self, ctx: &egui::Context) {
        let expired = matches!(&self.toast, Some((_, until)) if *until <= Instant::now());
        if expired {
            self.toast = None;
        }
        if let Some((msg, _)) = &self.toast {
            let msg = msg.clone();
            egui::Area::new(egui::Id::new("toast"))
                .anchor(Align2::CENTER_TOP, egui::vec2(0.0, 56.0))
                .order(egui::Order::Foreground)
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::default()
                        .fill(TOAST_BG)
                        .rounding(10.0)
                        .stroke(egui::Stroke::new(1.0_f32, BORDER))
                        .inner_margin(egui::Margin::symmetric(14.0, 6.0))
                        .shadow(hard_shadow())
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("✓").color(TOAST_OK).strong().size(12.0));
                                ui.add_space(5.0);
                                ui.label(
                                    RichText::new(msg).color(TOAST_TEXT).strong().size(12.0),
                                );
                            });
                        });
                });
        }
    }

    // ── 错误弹窗：遮罩 + 居中白色卡片（浅描边 + 柔和阴影）──
    fn paint_modal(&mut self, ctx: &egui::Context) {
        let Some((header, message)) = self.error_modal.clone() else {
            return;
        };

        // 全屏遮罩：拦截底层输入并压暗背景
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("modal-mask"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(ctx, |ui| {
                ui.allocate_rect(screen, Sense::click());
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(60));
            });

        // 居中错误卡片
        let mut close = false;
        egui::Window::new("error-modal")
            .id(egui::Id::new("error-modal"))
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .frame(
                egui::Frame::default()
                    .fill(CARD)
                    .rounding(10.0)
                    .stroke(egui::Stroke::new(1.0_f32, BORDER))
                    .shadow(hard_shadow())
                    .inner_margin(egui::Margin::same(16.0)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(400.0);
                ui.set_max_width(560.0);

                // 标题行：红色标题 + 右侧关闭按钮
                ui.horizontal(|ui| {
                    ui.label(RichText::new(&header).color(ERR).size(14.5).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(
                                Button::new(RichText::new("✕").color(DIM).size(13.0))
                                    .fill(Color32::TRANSPARENT)
                                    .stroke(egui::Stroke::NONE),
                            )
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                ui.add_space(7.0);
                // 分隔线
                let rect = ui.min_rect();
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.left(), rect.bottom()),
                        egui::pos2(rect.right(), rect.bottom()),
                    ],
                    egui::Stroke::new(1.0_f32, BORDER),
                );
                ui.add_space(7.0);

                // 消息区（可选中复制，限高滚动）
                egui::ScrollArea::vertical()
                    .id_salt("modal-msg")
                    .max_height(240.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&message).color(TEXT).size(12.5))
                                .selectable(true)
                                .wrap(),
                        );
                    });
                ui.add_space(12.0);

                // 底部：右对齐主按钮
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if Self::action_button(ui, "确定", true, true).clicked() {
                        close = true;
                    }
                });
            });
        if close {
            self.error_modal = None;
        }
    }
}

impl eframe::App for PatcherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 非阻塞收取后台事件
        while let Ok(ev) = self.rx.try_recv() {
            self.on_event(ev);
        }
        // 有后台操作时保持重绘，确保事件被及时处理
        if self.busy() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        self.paint(ctx);
    }
}
