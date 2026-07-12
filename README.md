# Navicat Patcher — Navicat 17.3.x 激活补丁工具

> ## ⚠️ 免责声明
>
> 本项目仅供**学习、研究和技术交流**使用，严禁用于任何商业用途或非法用途。
>
> 使用本工具补丁、激活的软件仍需购买正版授权。本项目不对任何因使用本工具而产生的法律责任负责。使用即代表您已阅读并同意本声明。
>
> 如您代表软件版权方且认为本项目侵犯了您的合法权益，请联系本项目所有者删除。

基于 Java + JavaFX 实现的 Navicat Premium 离线激活可视化工具，通过向 `libcc.dll` 注入自定义 RSA 公钥实现补丁，配合离线激活码生成完成全流程。

![navicat-patcher](data/md/navicat-patcher.png)

---

## 适用版本

| 项目 | 说明 |
|------|------|
| 软件 | Navicat Premium **17.3.x**（简体中文版） |
| 测试 | Navicat 17.3.11 中文版 ✅ |
| 系统 | Windows x64 |

---

## 环境准备

| 依赖 | 版本要求 | 说明 |
|------|---------|------|
| JDK | 17+ | 推荐 [Adoptium Temurin](https://adoptium.net/) |
| JavaFX | 17.0.10 | 已在 `pom.xml` 中声明，编译时自动下载 |
| Maven | 3.6+ | 用于编译和打包 |

> 从 JDK 11 开始 JavaFX 不再捆绑在 JDK 中，本项目通过 Maven 依赖自动引入，无需单独安装。

### Navicat 下载

- 官网：[https://www.navicat.com.cn](https://www.navicat.com.cn)
- 网盘（可选）：[Navicat 17.3.11 中文版](https://www.alipan.com/s/JxuduumBbSH)

---

## 编译与运行

### 编译

```bat
mvn compile
```

### 打包（生成 Fat JAR）

```bat
mvn package
```

打包后生成 `target/navicat-patcher-1.0.0.jar`，包含所有依赖。

### 运行

```bat
java -jar target/navicat-patcher-1.0.0.jar
```

> 也可通过 Maven 直接运行（开发阶段）：`mvn javafx:run`

---

## 操作流程

工具采用分步向导式界面，按 **Step 1 → Step 4** 顺序依次执行，每个 Step 内部按钮均标注了 ①②③ 序号。

### Step 1：选择安装目录并备份

| 序号 | 操作 |
|------|------|
| ① | 点击「选择目录」选择 Navicat 安装目录，自动查找并备份 `libcc.dll` |

- 也可手动选择 `libcc.dll` 文件，安装目录会自动回填
- 备份文件保存为 `libcc.dll.bak`，如已存在则跳过

### Step 2：生成密钥对

| 序号 | 操作 |
|------|------|
| ① | 点击「生成 2048 位 RSA 密钥对」 |

- 公钥将注入到 `libcc.dll`，私钥用于后续签名激活码
- 每次生成的密钥对均为随机，可多次重新生成

### Step 3：应用补丁

| 序号 | 操作 |
|------|------|
| ① | 点击「应用补丁到 libcc.dll」 |

- 自动向 PE 文件注入 `.pkey` 节（存放公钥）
- 修改 `.text` 节中的指令，使 Navicat 读取注入的公钥替代内置公钥
- 补丁后的文件直接覆盖原 `libcc.dll`

### Step 4：离线激活

> ⚠ **请先断网，关闭 Navicat 进程后再操作！**

| 序号 | 操作 |
|------|------|
| ① | 点击「复制密钥」，粘贴到 Navicat 注册窗口 |
| ② | 点击「启动 Navicat」，在 Navicat 中选择「手动激活」获取请求码 |
| ③ | 将请求码粘贴到输入框，填写用户名和组织名 |
| ④ | 点击「生成激活码」 |
| ⑤ | 点击「复制激活码」，粘贴到 Navicat 激活窗口完成激活 |

---

## 界面预览

![ScreenPage](data/md/ScreenPage.png)

---

## 项目结构

```
navicat-patcher/
├── pom.xml                          # Maven 配置（JavaFX + Jackson + Shade）
├── README.md
├── md/                              # 文档资源
│   ├── navicat-patcher.png
│   └── ScreenPage.png
└── src/main/
    ├── java/com/navicat/patcher/
    │   ├── Main.java                # 程序入口
    │   ├── AppUI.java               # JavaFX 可视化主界面
    │   ├── PatcherService.java      # 核心业务：备份/补丁/激活码生成
    │   ├── PEFile.java              # 纯 Java PE 文件解析与节注入
    │   ├── RSAHelper.java           # RSA 2048 密钥生成与私钥签名
    │   └── ByteUtils.java           # 字节操作工具
    └── resources/
        ├── styles.css               # 深色主题样式
        └── icon.png                 # 应用图标
```

---

## 致谢

- [lihaotong0712/navicat-17.3.x-crack](https://github.com/lihaotong0712/navicat-17.3.x-crack) — 原始 Python 脚本
- [Navicat 17 破解教程 - 吾爱破解](https://www.52pojie.cn/thread-2052969-1-1.html)
