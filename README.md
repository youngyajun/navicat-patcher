

<h1 align="center" style="margin: 30px 0 30px; font-weight: bold;">Navicat 17.3.x Patcher</h1>



> ## ⚠️ 免责声明
>
> 本项目仅供**学习、研究和技术交流**使用，严禁用于任何商业用途或非法用途。
>
> 使用本工具补丁、激活的软件仍需购买正版授权。本项目不对任何因使用本工具而产生的法律责任负责。使用即代表您已阅读并同意本声明。
>
> 如您代表软件版权方且认为本项目侵犯了您的合法权益，请联系本项目所有者删除。



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
| JDK | 17+ | 推荐Oracle JDK 17+ |
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

---

## 界面预览

![ScreenPage](data/md/ScreenPage.png)



---

## 致谢

- [lihaotong0712/navicat-17.3.x-crack](https://github.com/lihaotong0712/navicat-17.3.x-crack)
- [Navicat 17 破解教程 - 吾爱破解](https://www.52pojie.cn/thread-2052969-1-1.html)
