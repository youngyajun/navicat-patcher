package com.navicat.patcher;

import javafx.application.Application;

/**
 * 程序入口。
 *
 * 运行方式:
 *   mvn javafx:run
 *
 * 或构建 fat JAR 后运行:
 *   java -jar target/navicat-patcher-1.0.0.jar
 *   (需要 JavaFX 运行时在模块路径上)
 */
public class Main {
    public static void main(String[] args) {
        Application.launch(AppUI.class, args);
    }
}
