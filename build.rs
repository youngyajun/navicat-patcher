fn main() {
    // Windows MSVC：把 icons/icon.ico 嵌入 exe 资源（任务栏/资源管理器图标）
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("icons/icon.ico");
        res.compile().unwrap();
    }
    println!("cargo:rerun-if-changed=icons/icon.ico");
}
