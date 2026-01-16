fn main() {
    // 强制使用 OpenSSL vendored 版本，避免依赖系统 OpenSSL
    println!("cargo:rustc-env=OPENSSL_NO_VENDOR=0");

    // tauri::generate_context! 在编译期会校验 `frontendDist` 路径是否存在。
    // 开发/CI 场景下可能只跑 `cargo check/test` 而未先构建前端，从而导致宏 panic。
    // 这里提前创建配置中的 `../dist` 目录，避免无关的编译阻塞。
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest_path = std::path::PathBuf::from(&manifest_dir);
        let dist_dir = manifest_path.join("../dist");
        let _ = std::fs::create_dir_all(dist_dir);

        // 检查 models 资源是否存在
        check_models_resources(&manifest_path);
    }
    tauri_build::build()
}

/// 检查 models 资源目录是否存在
/// 如果不存在，输出警告提示用户运行下载脚本
fn check_models_resources(manifest_dir: &std::path::Path) {
    let models_dir = manifest_dir.join("resources/models");
    let index_file = models_dir.join("index.json");

    if !index_file.exists() {
        println!("cargo:warning=======================================================");
        println!("cargo:warning=Models 资源不存在！请运行以下命令下载：");
        println!("cargo:warning=  ./scripts/download-models.sh");
        println!("cargo:warning=======================================================");

        // 创建空目录结构，避免 Tauri 构建失败
        let _ = std::fs::create_dir_all(models_dir.join("providers"));
        let _ = std::fs::create_dir_all(models_dir.join("aliases"));
    }
}
