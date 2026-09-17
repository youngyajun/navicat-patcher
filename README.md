# Navicat Patcher — egui 纯原生版本

`navicat-patcher` 的 **Rust + egui** 纯原生重构版本。**零运行时依赖**：不使用 WebView / WebView2 / 浏览器内核 / JVM / .NET，所有代码静态编译进**单个绿色 exe**，双击即用。

核心补丁逻辑（PE 解析、RSA 密钥运算、四步激活流程）与 Java 版**字节级等价**，界面布局、文案与交互流程逐一对齐。


## 目录结构

```
navicat-patcher-egui/
├── Cargo.toml               # 依赖清单（全部静态编译，无运行时依赖）
├── build.rs                 # winres：把 icon.ico 嵌入 exe 资源
├── assets/logo.png          # 标题栏 logo（编译期内嵌）
├── icons/icon.ico           # exe 图标（资源段嵌入）
└── src/
    ├── main.rs              # 入口：窗口 900×680（最小 860×620）
    ├── app.rs               # 主界面：手风琴四步向导 + 终端日志 + Toast + 错误弹窗
    ├── theme.rs             # 复古暖色主题/中文字体/图标（米黄画布 + 奶油卡片 + 焦糖橙主色）
    ├── platform.rs          # 原生对话框(rfd)/剪贴板(arboard)/启动进程
    └── core/                # 核心算法（与 Tauri 版完全一致，UI 无关）
        ├── byte_utils.rs    #   字节搜索/小端读写/对齐
        ├── pe_file.rs       #   PE 解析/加节（纯 Rust，无第三方 PE 库）
        ├── rsa.rs           #   RSA：密钥生成/PKCS1 解密/私钥 m^d mod n
        └── patcher.rs       #   四步流程服务（日志回调注入）
```

## 环境要求

- **Rust**（MSVC toolchain）：<https://rustup.rs>（需 Visual Studio C++ Build Tools）
- 无需 Node.js、无需 WebView2、无需任何运行库

## 常用命令

```bash
# 开发运行（增量编译 + 热重启）
cargo run

# 编译检查 / 单元测试（字节工具 + RSA 填充结构验证）
cargo check
cargo test

# 发布构建：产物 target/release/navicat-patcher.exe（绿色单文件）
cargo build --release
```

> ⚠️ 若终端报 `cargo: program not found`，说明该终端在安装 Rust 前打开——重启终端即可。

## 已知注意事项

- 未签名 exe 可能被 SmartScreen / 杀软误报，根因是工具修改第三方 DLL 的行为本身；如需消除请配置代码签名证书。
- 中文输入法（IME）在 egui 0.29 的 `TextEdit` 中受支持；若在特殊环境下候选框定位异常，可升级 egui 版本。
