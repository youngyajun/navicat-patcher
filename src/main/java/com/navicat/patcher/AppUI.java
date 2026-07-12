package com.navicat.patcher;

import javafx.animation.FadeTransition;
import javafx.animation.PauseTransition;
import javafx.application.Application;
import javafx.application.Platform;
import javafx.concurrent.Task;
import javafx.geometry.Insets;
import javafx.geometry.Pos;
import javafx.scene.Scene;
import javafx.scene.control.*;
import javafx.scene.layout.*;
import javafx.stage.DirectoryChooser;
import javafx.stage.FileChooser;
import javafx.stage.Popup;
import javafx.stage.Stage;
import javafx.util.Duration;

import java.io.File;
import java.time.LocalTime;
import java.time.format.DateTimeFormatter;

/**
 * JavaFX 可视化主界面：紧凑分步向导式操作流程，所有步骤一屏显示。
 */
public class AppUI extends Application {

    private final PatcherService service = new PatcherService(this::log);

    // ── UI 组件 ──
    private TextField installDirField;
    private TextField dllPathField;
    private Button browseDirBtn, browseDllBtn;
    private Label backupStatus;

    private Button generateKeyBtn;
    private TextArea publicKeyArea, privateKeyArea;
    private Label keyStatus;

    private Button patchBtn;
    private Label patchStatus;

    private TextArea requestCodeArea;
    private TextField usernameField, organizationField;
    private Button generateActivationBtn, copyBtn, launchNavicatBtn, copyKeyBtn;
    private TextArea activationCodeArea;
    private Label step4HintLabel;

    private TextArea logArea;

    // 步骤容器（用于启用/禁用）
    private VBox step2Box, step3Box, step4Box;

    // 主窗口引用（用于显示 Toast）
    private Stage mainStage;

    // 状态
    private String navicatInstallDir;
    private String libccDllPath;
    private String backupPath;

    @Override
    public void start(Stage primaryStage) {
        mainStage = primaryStage;
        logArea = new TextArea();
        logArea.setEditable(false);
        logArea.setWrapText(true);
        VBox.setVgrow(logArea, Priority.ALWAYS);
        logArea.setStyle("-fx-font-family: 'Consolas'; -fx-font-size: 11px;");

        VBox root = new VBox(5);
        root.setPadding(new Insets(10));

        // ── 标题行（标题 + 副标题同一行）──
        Label title = new Label("Navicat 17.3.x 激活补丁工具");
        title.setStyle("-fx-font-size: 18px; -fx-font-weight: bold; -fx-text-fill: #e0e0e0;");
        Label subtitle = new Label("  (请先断网，关闭 Navicat 进程)");
        subtitle.setStyle("-fx-font-size: 12px; -fx-text-fill: #9999aa;");
        HBox titleRow = new HBox(title, subtitle);
        titleRow.setAlignment(Pos.BASELINE_LEFT);

        // ── 各步骤 ──
        VBox step1 = createStep1();
        step2Box = createStep2();
        step3Box = createStep3();
        step4Box = createStep4();

        step2Box.setDisable(true);
        step3Box.setDisable(true);
        step4Box.setDisable(true);

        // ── 日志区（默认展开，自动填充剩余空间）──
        VBox logContainer = new VBox(logArea);
        VBox.setVgrow(logContainer, Priority.ALWAYS);
        logContainer.setFillWidth(true);

        TitledPane logPane = new TitledPane("日志输出", logContainer);
        logPane.setCollapsible(true);
        logPane.setExpanded(true);
        VBox.setVgrow(logPane, Priority.ALWAYS);
        logPane.setMaxHeight(Double.MAX_VALUE);

        root.getChildren().addAll(titleRow, step1, step2Box, step3Box, step4Box, logPane);

        Scene scene = new Scene(root, 1000, 760);
        scene.getStylesheets().add(getClass().getResource("/styles.css").toExternalForm());

        primaryStage.setTitle("Navicat Patcher");
        primaryStage.setScene(scene);
        primaryStage.setMinWidth(950);
        primaryStage.setMinHeight(700);

        // 设置窗口图标
        try {
            primaryStage.getIcons().add(
                    new javafx.scene.image.Image(getClass().getResourceAsStream("/icon.ico")));
        } catch (Exception e) {
            log("无法加载图标: " + e.getMessage());
        }

        primaryStage.show();

        log("=== Navicat 激活补丁工具已启动 ===");
        log("提示: 请先断网，然后选择 Navicat 安装目录开始操作。");
    }

    // ═══════════════════════════════════════════════════════════
    //  Step 1: 选择 Navicat 安装目录并自动备份
    // ═══════════════════════════════════════════════════════════
    private VBox createStep1() {
        VBox box = createStepBox("Step 1: 选择安装目录并备份");

        installDirField = new TextField();
        installDirField.setPromptText("选择 Navicat 安装目录...");
        HBox.setHgrow(installDirField, Priority.ALWAYS);
        installDirField.textProperty().addListener((obs, o, n) -> navicatInstallDir = n.trim());

        browseDirBtn = new Button("① 选择目录");
        browseDirBtn.setOnAction(e -> browseInstallDir());

        HBox dirRow = new HBox(6, new Label("安装目录:"), installDirField, browseDirBtn);
        dirRow.setAlignment(Pos.CENTER_LEFT);

        dllPathField = new TextField();
        dllPathField.setPromptText("libcc.dll 路径（选择目录后自动填充，也可手动选择）...");
        HBox.setHgrow(dllPathField, Priority.ALWAYS);
        dllPathField.textProperty().addListener((obs, o, n) -> libccDllPath = n.trim());

        browseDllBtn = new Button("手动选择");
        browseDllBtn.setOnAction(e -> browseDllFile());

        HBox dllRow = new HBox(6, new Label("DLL路径:"), dllPathField, browseDllBtn);
        dllRow.setAlignment(Pos.CENTER_LEFT);

        backupStatus = new Label("状态: 待操作 — 点击 ① 选择目录后自动查找并备份 libcc.dll");
        backupStatus.setStyle("-fx-text-fill: #9999aa;");

        box.getChildren().addAll(dirRow, dllRow, backupStatus);
        return box;
    }

    private void browseInstallDir() {
        DirectoryChooser dc = new DirectoryChooser();
        dc.setTitle("选择 Navicat 安装目录");
        File selectedDir = dc.showDialog(null);
        if (selectedDir == null) return;

        String dirPath = selectedDir.getAbsolutePath();
        installDirField.setText(dirPath);
        navicatInstallDir = dirPath;
        log("已选择 Navicat 安装目录: " + dirPath);

        File libccFile = new File(dirPath, "libcc.dll");
        if (libccFile.exists()) {
            dllPathField.setText(libccFile.getAbsolutePath());
            libccDllPath = libccFile.getAbsolutePath();
            log("已找到 libcc.dll: " + libccFile.getAbsolutePath());
            autoBackup(libccFile.getAbsolutePath());
        } else {
            log("在安装目录下未找到 libcc.dll，请手动选择。");
            backupStatus.setText("状态: ⚠️ 未找到 libcc.dll，请手动选择文件");
            backupStatus.setStyle("-fx-text-fill: #ffaa00;");
            step2Box.setDisable(true);
        }
    }

    private void browseDllFile() {
        FileChooser fc = new FileChooser();
        fc.setTitle("选择 libcc.dll");
        fc.getExtensionFilters().add(new FileChooser.ExtensionFilter("DLL 文件", "*.dll"));
        fc.getExtensionFilters().add(new FileChooser.ExtensionFilter("所有文件", "*.*"));
        File selected = fc.showOpenDialog(null);
        if (selected != null) {
            String dllPath = selected.getAbsolutePath();
            dllPathField.setText(dllPath);
            libccDllPath = dllPath;
            log("已选择 libcc.dll: " + dllPath);

            // libcc.dll 所在目录即为安装目录，自动回填
            File parentDir = selected.getParentFile();
            if (parentDir != null) {
                String dirPath = parentDir.getAbsolutePath();
                installDirField.setText(dirPath);
                navicatInstallDir = dirPath;
                log("已自动回填安装目录: " + dirPath);
            }

            autoBackup(dllPath);
        }
    }

    private void autoBackup(String dllPath) {
        backupStatus.setText("状态: 正在自动备份...");
        backupStatus.setStyle("-fx-text-fill: #ffaa00;");
        browseDirBtn.setDisable(true);
        browseDllBtn.setDisable(true);

        Task<String> task = new Task<>() {
            @Override
            protected String call() throws Exception {
                return service.backupFile(dllPath);
            }
        };
        task.setOnSucceeded(e -> {
            backupPath = task.getValue();
            backupStatus.setText("状态: ✅ 已自动备份 → " + backupPath);
            backupStatus.setStyle("-fx-text-fill: #4caf50;");
            browseDirBtn.setDisable(false);
            browseDllBtn.setDisable(false);
            step2Box.setDisable(false);
            log("Step 1 完成: libcc.dll 已自动备份");
        });
        task.setOnFailed(e -> {
            backupStatus.setText("状态: ❌ 备份失败");
            backupStatus.setStyle("-fx-text-fill: #f44336;");
            browseDirBtn.setDisable(false);
            browseDllBtn.setDisable(false);
            showError("自动备份失败", task.getException());
        });
        new Thread(task).start();
    }

    // ═══════════════════════════════════════════════════════════
    //  Step 2: 生成密钥对（公钥/私钥左右分栏）
    // ═══════════════════════════════════════════════════════════
    private VBox createStep2() {
        VBox box = createStepBox("Step 2: 生成密钥对");

        generateKeyBtn = new Button("① 生成 2048 位 RSA 密钥对");
        generateKeyBtn.setOnAction(e -> doGenerateKey());
        keyStatus = new Label("状态: 待操作");
        keyStatus.setStyle("-fx-text-fill: #9999aa;");
        HBox btnRow = new HBox(10, generateKeyBtn, keyStatus);
        btnRow.setAlignment(Pos.CENTER_LEFT);

        // 公钥和私钥左右分栏
        publicKeyArea = new TextArea();
        publicKeyArea.setPromptText("公钥...");
        publicKeyArea.setPrefRowCount(4);
        publicKeyArea.setEditable(false);
        publicKeyArea.setWrapText(true);
        publicKeyArea.setStyle("-fx-font-family: 'Consolas'; -fx-font-size: 10px;");
        VBox pubBox = new VBox(2, new Label("公钥:"), publicKeyArea);

        privateKeyArea = new TextArea();
        privateKeyArea.setPromptText("私钥...");
        privateKeyArea.setPrefRowCount(4);
        privateKeyArea.setEditable(false);
        privateKeyArea.setWrapText(true);
        privateKeyArea.setStyle("-fx-font-family: 'Consolas'; -fx-font-size: 10px;");
        VBox priBox = new VBox(2, new Label("私钥:"), privateKeyArea);

        HBox keysRow = new HBox(10, pubBox, priBox);
        HBox.setHgrow(pubBox, Priority.ALWAYS);
        HBox.setHgrow(priBox, Priority.ALWAYS);

        box.getChildren().addAll(btnRow, keysRow);
        return box;
    }

    private void doGenerateKey() {
        generateKeyBtn.setDisable(true);
        keyStatus.setText("状态: 正在生成...");
        keyStatus.setStyle("-fx-text-fill: #ffaa00;");

        Task<Void> task = new Task<>() {
            @Override
            protected Void call() throws Exception {
                service.generateKeyPair();
                return null;
            }
        };
        task.setOnSucceeded(e -> {
            publicKeyArea.setText(service.getRsaHelper().getPublicKeyPEM());
            privateKeyArea.setText(service.getRsaHelper().getPrivateKeyPEM());
            keyStatus.setText("状态: ✅ 密钥对已生成");
            keyStatus.setStyle("-fx-text-fill: #4caf50;");
            generateKeyBtn.setDisable(false);
            step3Box.setDisable(false);
            log("Step 2 完成: RSA 密钥对已生成");
        });
        task.setOnFailed(e -> {
            keyStatus.setText("状态: ❌ 生成失败");
            keyStatus.setStyle("-fx-text-fill: #f44336;");
            generateKeyBtn.setDisable(false);
            showError("密钥生成失败", task.getException());
        });
        new Thread(task).start();
    }

    // ═══════════════════════════════════════════════════════════
    //  Step 3: 应用补丁（按钮+状态同一行，极紧凑）
    // ═══════════════════════════════════════════════════════════
    private VBox createStep3() {
        VBox box = createStepBox("Step 3: 应用补丁");

        patchBtn = new Button("① 应用补丁到 libcc.dll");
        patchBtn.setOnAction(e -> doApplyPatch());
        patchStatus = new Label("状态: 待操作");
        patchStatus.setStyle("-fx-text-fill: #9999aa;");

        HBox row = new HBox(10, patchBtn, patchStatus);
        row.setAlignment(Pos.CENTER_LEFT);

        box.getChildren().add(row);
        return box;
    }

    private void doApplyPatch() {
        if (backupPath == null || backupPath.isEmpty()) {
            showError("请先完成 Step 1 备份");
            return;
        }
        String outputPath = libccDllPath;
        if (outputPath == null || outputPath.isEmpty()) {
            showError("未指定 libcc.dll 路径");
            return;
        }

        patchBtn.setDisable(true);
        patchStatus.setText("状态: 正在应用补丁...");
        patchStatus.setStyle("-fx-text-fill: #ffaa00;");

        Task<PatcherService.PatchResult> task = new Task<>() {
            @Override
            protected PatcherService.PatchResult call() throws Exception {
                return service.applyPatch(backupPath, outputPath);
            }
        };
        task.setOnSucceeded(e -> {
            PatcherService.PatchResult result = task.getValue();
            patchStatus.setText(String.format("状态: ✅ 补丁已应用 (偏移: 0x%X, RIP位移: 0x%X)",
                    result.patchFileOffset, result.offsetFromRip));
            patchStatus.setStyle("-fx-text-fill: #4caf50;");
            patchBtn.setDisable(false);
            step4Box.setDisable(false);
            log("Step 3 完成: 补丁已应用");
        });
        task.setOnFailed(e -> {
            patchStatus.setText("状态: ❌ 补丁应用失败");
            patchStatus.setStyle("-fx-text-fill: #f44336;");
            patchBtn.setDisable(false);
            showError("补丁应用失败", task.getException());
        });
        new Thread(task).start();
    }

    // ═══════════════════════════════════════════════════════════
    //  Step 4: 离线激活（按 ①②③④⑤ 顺序操作）
    // ═══════════════════════════════════════════════════════════
    private VBox createStep4() {
        VBox box = createStepBox("Step 4: 离线激活");

        // 红色断网警告 + 产品密钥 + 复制 + 启动按钮 同一行
        Label warnLabel = new Label("⚠ 请断网后操作！");
        warnLabel.setStyle("-fx-text-fill: #f44336; -fx-font-size: 12px; -fx-font-weight: bold;");

        Label productKeyLabel = new Label("密钥:");
        Label productKey = new Label(PatcherService.PRODUCT_KEY_FORMATTED);
        productKey.setStyle("-fx-font-weight: bold; -fx-text-fill: #ffaa00; -fx-font-family: 'Consolas'; -fx-font-size: 13px;");

        copyKeyBtn = new Button("① 复制密钥");
        copyKeyBtn.setOnAction(e -> {
            javafx.scene.input.Clipboard clipboard = javafx.scene.input.Clipboard.getSystemClipboard();
            javafx.scene.input.ClipboardContent content = new javafx.scene.input.ClipboardContent();
            content.putString(PatcherService.PRODUCT_KEY_FORMATTED);
            clipboard.setContent(content);
            showToast("✅ 密钥复制成功");
            log("产品密钥已复制到剪贴板");
        });

        launchNavicatBtn = new Button("② 启动 Navicat");
        launchNavicatBtn.setOnAction(e -> launchNavicat());

        HBox keyRow = new HBox(10, warnLabel, new Separator(), productKeyLabel, productKey, copyKeyBtn,
                new Separator(), launchNavicatBtn);
        keyRow.setAlignment(Pos.CENTER_LEFT);

        // ── ③ 输入区域 ──
        Label step1Label = new Label("③ 输入请求码、用户名和组织名:");
        step1Label.setStyle("-fx-text-fill: #7c6cf0; -fx-font-size: 12px; -fx-font-weight: bold;");

        requestCodeArea = new TextArea();
        requestCodeArea.setPromptText("将 Navicat 离线激活窗口中的请求码粘贴到此处...");
        requestCodeArea.setPrefRowCount(3);
        requestCodeArea.setWrapText(true);
        requestCodeArea.setStyle("-fx-font-family: 'Consolas'; -fx-font-size: 10px;");

        usernameField = new TextField();
        usernameField.setPromptText("用户名");
        HBox.setHgrow(usernameField, Priority.ALWAYS);

        organizationField = new TextField();
        organizationField.setPromptText("组织名");
        HBox.setHgrow(organizationField, Priority.ALWAYS);

        HBox inputRow = new HBox(8, new Label("用户名:"), usernameField,
                new Label("组织名:"), organizationField);
        inputRow.setAlignment(Pos.CENTER_LEFT);

        VBox inputBox = new VBox(3, step1Label, requestCodeArea, inputRow);

        // ── ④ 生成激活码 ──
        generateActivationBtn = new Button("④ 生成激活码");
        generateActivationBtn.setStyle("-fx-font-weight: bold;");
        generateActivationBtn.setOnAction(e -> doGenerateActivation());
        generateActivationBtn.setDisable(true); // 初始禁用，等输入完成后启用

        step4HintLabel = new Label("(请先完成 ③ 中的输入)");
        step4HintLabel.setStyle("-fx-text-fill: #9999aa; -fx-font-size: 11px;");

        HBox genRow = new HBox(8, generateActivationBtn, step4HintLabel);
        genRow.setAlignment(Pos.CENTER_LEFT);

        // ── ⑤ 复制激活码 ──
        activationCodeArea = new TextArea();
        activationCodeArea.setPromptText("激活码生成后显示在此处...");
        activationCodeArea.setPrefRowCount(3);
        activationCodeArea.setEditable(false);
        activationCodeArea.setWrapText(true);
        activationCodeArea.setStyle("-fx-font-family: 'Consolas'; -fx-font-size: 10px;");

        copyBtn = new Button("⑤ 复制激活码");
        copyBtn.setStyle("-fx-font-weight: bold;");
        copyBtn.setOnAction(e -> copyActivationCode());
        copyBtn.setDisable(true); // 初始禁用，等激活码生成后启用

        HBox copyRow = new HBox(8, new Label("激活码:"), activationCodeArea, copyBtn);
        copyRow.setAlignment(Pos.TOP_LEFT);
        HBox.setHgrow(activationCodeArea, Priority.ALWAYS);

        // 监听输入变化，控制 ④ 按钮状态
        requestCodeArea.textProperty().addListener((obs, o, n) -> updateStep4Buttons());
        usernameField.textProperty().addListener((obs, o, n) -> updateStep4Buttons());
        organizationField.textProperty().addListener((obs, o, n) -> updateStep4Buttons());

        box.getChildren().addAll(keyRow, inputBox, genRow, copyRow);
        return box;
    }

    /**
     * 根据 ③ 中输入的完成情况，动态控制 ④ 按钮的启用/禁用和提示文字。
     */
    private void updateStep4Buttons() {
        boolean ready = !requestCodeArea.getText().trim().isEmpty()
                && !usernameField.getText().trim().isEmpty()
                && !organizationField.getText().trim().isEmpty();
        generateActivationBtn.setDisable(!ready);
        if (ready) {
            step4HintLabel.setText("(输入已完成，点击 ④ 生成)​");
            step4HintLabel.setStyle("-fx-text-fill: #4caf50; -fx-font-size: 11px;");
        } else {
            step4HintLabel.setText("(请先完成 ③ 中的输入)​");
            step4HintLabel.setStyle("-fx-text-fill: #9999aa; -fx-font-size: 11px;");
        }
    }

    private void launchNavicat() {
        try {
            if (navicatInstallDir == null || navicatInstallDir.isEmpty()) {
                showError("请先在 Step 1 中选择 Navicat 安装目录");
                return;
            }
            File navicatExe = new File(navicatInstallDir, "navicat.exe");
            if (navicatExe.exists()) {
                new ProcessBuilder(navicatExe.getAbsolutePath())
                        .directory(new File(navicatInstallDir))
                        .start();
                log("已启动: " + navicatExe.getAbsolutePath());
            } else {
                showError("在安装目录下未找到 navicat.exe:\n" + navicatInstallDir + "\n请确认目录是否正确。");
            }
        } catch (Exception ex) {
            log("启动 Navicat 失败: " + ex.getMessage());
            showError("启动 Navicat 失败", ex);
        }
    }

    private void doGenerateActivation() {
        String requestCode = requestCodeArea.getText().trim();
        String username = usernameField.getText().trim();
        String organization = organizationField.getText().trim();

        if (requestCode.isEmpty()) { showError("请输入请求码"); return; }
        if (username.isEmpty())    { showError("请输入用户名"); return; }
        if (organization.isEmpty()) { showError("请输入组织名"); return; }

        generateActivationBtn.setDisable(true);
        activationCodeArea.setText("正在生成激活码...");

        Task<String> task = new Task<>() {
            @Override
            protected String call() throws Exception {
                return service.generateActivationCode(requestCode, username, organization);
            }
        };
        task.setOnSucceeded(e -> {
            activationCodeArea.setText(task.getValue());
            generateActivationBtn.setDisable(false);
            copyBtn.setDisable(false); // 激活码生成后，启用 ⑤ 复制按钮
            log("Step 4 完成: 激活码已生成");
            log("请复制激活码到 Navicat 激活窗口完成激活。");
        });
        task.setOnFailed(e -> {
            activationCodeArea.setText("生成失败！");
            generateActivationBtn.setDisable(false);
            showError("激活码生成失败", task.getException());
        });
        new Thread(task).start();
    }

    private void copyActivationCode() {
        String code = activationCodeArea.getText();
        if (code.isEmpty() || code.equals("正在生成激活码...") || copyBtn.isDisabled()) {
            showError("暂无激活码可复制");
            return;
        }
        javafx.scene.input.Clipboard clipboard = javafx.scene.input.Clipboard.getSystemClipboard();
        javafx.scene.input.ClipboardContent content = new javafx.scene.input.ClipboardContent();
        content.putString(code);
        clipboard.setContent(content);
        showToast("✅ 激活码复制成功");
        log("激活码已复制到剪贴板");
    }

    // ═══════════════════════════════════════════════════════════
    //  工具方法
    // ═══════════════════════════════════════════════════════════

    private VBox createStepBox(String title) {
        VBox box = new VBox(4);
        box.setPadding(new Insets(8));
        box.setStyle("-fx-background-color: #2a2a3c; -fx-background-radius: 6;");

        Label titleLabel = new Label(title);
        titleLabel.setStyle("-fx-font-size: 13px; -fx-font-weight: bold; -fx-text-fill: #c0c0e0;");
        box.getChildren().add(titleLabel);
        return box;
    }

    /**
     * 在主窗口顶部中央显示一个短暂提示，1.5 秒后自动淡出消失。
     */
    private void showToast(String message) {
        Popup popup = new Popup();

        Label label = new Label(message);
        label.setStyle("-fx-background-color: #4caf50; -fx-text-fill: white; " +
                "-fx-padding: 8 20; -fx-background-radius: 6; -fx-font-size: 13px; -fx-font-weight: bold;");

        popup.getContent().add(label);
        popup.setAutoHide(false);
        popup.setOpacity(0);

        // 定位到主窗口顶部中央
        double x = mainStage.getX() + mainStage.getWidth() / 2 - 80;
        double y = mainStage.getY() + 60;
        popup.show(mainStage, x, y);

        // 淡入
        FadeTransition fadeIn = new FadeTransition(Duration.millis(200), label);
        fadeIn.setFromValue(0);
        fadeIn.setToValue(1);
        fadeIn.play();
        popup.setOpacity(1);

        // 1.2 秒后淡出并关闭
        PauseTransition delay = new PauseTransition(Duration.millis(1200));
        delay.setOnFinished(ev -> {
            FadeTransition fadeOut = new FadeTransition(Duration.millis(400), label);
            fadeOut.setFromValue(1);
            fadeOut.setToValue(0);
            fadeOut.setOnFinished(e2 -> popup.hide());
            fadeOut.play();
        });
        delay.play();
    }

    private void log(String msg) {
        String timestamp = LocalTime.now().format(DateTimeFormatter.ofPattern("HH:mm:ss"));
        String line = "[" + timestamp + "] " + msg + "\n";
        if (Platform.isFxApplicationThread()) {
            logArea.appendText(line);
        } else {
            Platform.runLater(() -> logArea.appendText(line));
        }
    }

    private void showError(String message) {
        Alert alert = new Alert(Alert.AlertType.ERROR, message, ButtonType.OK);
        alert.showAndWait();
    }

    private void showError(String header, Throwable ex) {
        log("错误: " + ex.getMessage());
        Alert alert = new Alert(Alert.AlertType.ERROR);
        alert.setTitle("错误");
        alert.setHeaderText(header);
        alert.setContentText(ex.getMessage());
        alert.showAndWait();
    }
}
