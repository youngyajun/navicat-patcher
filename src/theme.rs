//! 主题：现代浅色设计系统。
//!
//! 设计语言（干净 · 现代 · 轻盈）：
//!  - 浅灰蓝画布 + 纯白卡片，1px 浅灰描边 + 柔和投影，圆角 8-10px；
//!  - 单一品牌蓝主色：主按钮 / 当前步骤徽标 / 进度块；
//!  - 步骤徽标为圆角方形：蓝=进行中 / 绿描边=已完成 / 灰=未解锁；
//!  - 日志为浅灰白终端：浅灰底 + 中灰等宽字；
//!  - 浮层（弹窗/Toast）使用柔和阴影营造层级；
//!  - 字体运行时加载系统字体（Segoe UI / Consolas / 微软雅黑），exe 不内嵌字库。

use egui::{Color32, Context, TextureHandle, Visuals};

// ── 表面 ──
/// 应用画布底色（柔和浅灰蓝）
pub const APP_BG: Color32 = Color32::from_rgb(0xF4, 0xF7, 0xFC);
/// 卡片（步骤容器/弹窗/日志外壳）底色（纯白）
pub const CARD: Color32 = Color32::from_rgb(0xFE, 0xFF, 0xFF);
/// 次级表面（输入框/代码块底，比卡片深一档形成"嵌入"感）
pub const CARD_SUBTLE: Color32 = Color32::from_rgb(0xF5, 0xF8, 0xFC);
/// 进度空段 / 轨道
pub const TRACK: Color32 = Color32::from_rgb(0xE7, 0xEC, 0xF4);

// ── 边框 ──
/// 卡片细描边（浅灰）
pub const BORDER: Color32 = Color32::from_rgb(0xE4, 0xE9, 0xF0);
/// 输入框描边（比卡片描边深，保证边界清晰可见；白底输入框靠它与卡片区分）
pub const FIELD_BORDER: Color32 = Color32::from_rgb(0xB3, 0xC1, 0xD3);

// ── 文字 ──
/// 主文字（深灰）
pub const TEXT: Color32 = Color32::from_rgb(0x1F, 0x29, 0x37);
/// 次要文字
pub const DIM: Color32 = Color32::from_rgb(0x64, 0x74, 0x8B);
/// 占位符文字
pub const PLACEHOLDER: Color32 = Color32::from_rgb(0x9C, 0xA8, 0xB8);

// ── 主色（品牌蓝）──
pub const ACCENT: Color32 = Color32::from_rgb(0x3B, 0x6E, 0xF0);
/// 主色悬停（更亮）
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x5C, 0x84, 0xF5);
/// 主色按下（更深）
pub const ACCENT_PRESS: Color32 = Color32::from_rgb(0x2E, 0x5A, 0xD0);
/// 主色浅底（次级按钮悬停底 / 标签）
pub const ACCENT_TINT: Color32 = Color32::from_rgb(0xE9, 0xEF, 0xFE);
/// 主色浅底 · 按下态（比 ACCENT_TINT 深一档）
pub const ACCENT_TINT_DEEP: Color32 = Color32::from_rgb(0xD6, 0xE2, 0xFD);
/// 主按钮上的白色文字
pub const CREAM_TEXT: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);

// ── 按钮禁用态（低饱和浅灰 + 灰字：与可用态形成明确对比，避免"看起来能点"）──
pub const BTN_DISABLED_BG: Color32 = Color32::from_rgb(0xEE, 0xF1, 0xF7);
pub const BTN_DISABLED_BORDER: Color32 = Color32::from_rgb(0xDA, 0xE0, 0xEA);
pub const BTN_DISABLED_TEXT: Color32 = Color32::from_rgb(0xA6, 0xB1, 0xC1);

// ── 语义色 ──
pub const OK: Color32 = Color32::from_rgb(0x16, 0xA3, 0x4A);
pub const WARN: Color32 = Color32::from_rgb(0xD9, 0x77, 0x06);
pub const ERR: Color32 = Color32::from_rgb(0xE5, 0x48, 0x3E);

// ── 日志面板（浅灰白终端）──
pub const LOG_BG: Color32 = Color32::from_rgb(0xFA, 0xFB, 0xFD);
pub const LOG_BORDER: Color32 = Color32::from_rgb(0xE4, 0xE9, 0xF0);
pub const LOG_TEXT: Color32 = Color32::from_rgb(0x33, 0x41, 0x55);
pub const LOG_DIM: Color32 = Color32::from_rgb(0x94, 0xA3, 0xB8);

// ── Toast（白色纸片 · 浅描边 · 柔和投影）──
pub const TOAST_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
pub const TOAST_TEXT: Color32 = Color32::from_rgb(0x1F, 0x29, 0x37);
pub const TOAST_OK: Color32 = Color32::from_rgb(0x16, 0xA3, 0x4A);

/// 柔和投影（带羽化的阴影）——浮层（弹窗/Toast）与卡片使用营造层级
pub fn hard_shadow() -> egui::Shadow {
    egui::Shadow {
        offset: egui::Vec2::new(0.0, 4.0),
        blur: 16.0,
        spread: 0.0,
        color: Color32::from_black_alpha(22),
    }
}

/// 应用主题：现代浅色视觉 + 控件样式 + 系统 UI/等宽/CJK 字体
pub fn apply(ctx: &Context) {
    let mut v = Visuals::light();

    v.panel_fill = APP_BG;
    v.window_fill = CARD;
    v.window_stroke = egui::Stroke::new(1.0_f32, BORDER);
    v.window_shadow = hard_shadow();
    v.extreme_bg_color = CARD; // TextEdit 背景（纯白，靠边框区分边界）
    v.faint_bg_color = CARD_SUBTLE;
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = ACCENT_TINT; // 浅蓝文本选区
    v.selection.stroke = egui::Stroke::new(1.0_f32, ACCENT);

    // 圆角基准
    let radius: egui::Rounding = 8.0.into();

    // ── 控件视觉 ──
    // 输入框等 inactive 控件：白底 + 清晰灰蓝描边；悬停/聚焦时描边转品牌蓝。
    // 按钮三态由 action_button 每次显式覆盖并恢复，这里作为全局基准。
    //
    // 注意：egui 0.29 的 Button 取 `weak_bg_fill` 作为背景填充（而非 bg_fill），
    // 因此每个状态都必须同时设置 weak_bg_fill，否则按钮会退回默认灰底。
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = APP_BG;
    w.noninteractive.weak_bg_fill = CARD_SUBTLE;
    w.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, FIELD_BORDER);
    w.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    w.noninteractive.rounding = radius;
    w.noninteractive.expansion = 0.0;

    w.inactive.bg_fill = CARD;
    w.inactive.weak_bg_fill = CARD;
    w.inactive.bg_stroke = egui::Stroke::new(1.0_f32, FIELD_BORDER);
    w.inactive.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    w.inactive.rounding = radius;
    w.inactive.expansion = 0.0;

    w.hovered.bg_fill = CARD;
    w.hovered.weak_bg_fill = ACCENT_TINT;
    w.hovered.bg_stroke = egui::Stroke::new(1.0_f32, ACCENT);
    w.hovered.fg_stroke = egui::Stroke::new(1.0_f32, ACCENT_PRESS);
    w.hovered.rounding = radius;
    w.hovered.expansion = 0.5;

    w.active.bg_fill = CARD;
    w.active.weak_bg_fill = ACCENT_TINT_DEEP;
    w.active.bg_stroke = egui::Stroke::new(1.0_f32, ACCENT);
    w.active.fg_stroke = egui::Stroke::new(1.0_f32, ACCENT_PRESS);
    w.active.rounding = radius;
    w.active.expansion = 0.5;

    w.open.bg_fill = CARD;
    w.open.weak_bg_fill = ACCENT_TINT;
    w.open.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    w.open.fg_stroke = egui::Stroke::new(1.0_f32, TEXT);
    w.open.rounding = radius;
    w.open.expansion = 0.0;

    ctx.set_visuals(v);

    // 全局间距节奏：留白更舒展、控件更轻盈
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 7.0);
        style.spacing.button_padding = egui::vec2(14.0, 5.0);
        style.spacing.interact_size = egui::vec2(48.0, 30.0);
    });

    install_fonts(ctx);
}

/// 安装字体：
///  - Segoe UI（Windows 原生 UI 字体）优先渲染拉丁字符
///  - Consolas 优先渲染等宽文本（日志 / PEM / 激活码）
///  - CJK 回退到系统中文字体（微软雅黑等）
/// exe 无需内嵌字库。
fn install_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();

    insert_front(&mut fonts, "segoe", "C:\\Windows\\Fonts\\segoeui.ttf", egui::FontFamily::Proportional);
    insert_front(&mut fonts, "consolas", "C:\\Windows\\Fonts\\consola.ttf", egui::FontFamily::Monospace);

    // Windows 中文字体候选（按优先级），追加到各族末尾作为回退
    const CANDIDATES: &[&str] = &[
        "C:\\Windows\\Fonts\\msyh.ttc",  // 微软雅黑（Win7+）
        "C:\\Windows\\Fonts\\msyh.ttf",
        "C:\\Windows\\Fonts\\simhei.ttf", // 黑体
        "C:\\Windows\\Fonts\\simsun.ttc", // 宋体
    ];
    if let Some(path) = CANDIDATES.iter().find(|p| std::path::Path::new(p).exists()) {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("cjk".into(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(family).or_default().push("cjk".into());
            }
        }
    }

    ctx.set_fonts(fonts);
}

/// 将系统字体插入指定族列表首位（找不到文件则跳过）
fn insert_front(
    fonts: &mut egui::FontDefinitions,
    name: &str,
    path: &str,
    family: egui::FontFamily,
) {
    if !std::path::Path::new(path).exists() {
        return;
    }
    let Ok(bytes) = std::fs::read(path) else { return };
    fonts
        .font_data
        .insert(name.into(), egui::FontData::from_owned(bytes).into());
    fonts.families.entry(family).or_default().insert(0, name.into());
}

/// 标题栏 logo 纹理（显示尺寸由调用方决定）
pub fn load_logo(ctx: &Context) -> Option<TextureHandle> {
    let bytes = include_bytes!("../assets/logo.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(
        ctx.load_texture(
            "logo",
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &img),
            egui::TextureOptions::default(),
        ),
    )
}

/// 窗口图标（任务栏/标题栏）
pub fn load_icon() -> Option<egui::IconData> {
    let bytes = include_bytes!("../assets/logo.png");
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}