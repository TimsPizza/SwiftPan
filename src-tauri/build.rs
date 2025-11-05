fn main() {
    tauri_build::build();

    // 检查是否在为 Android 构建 (通过环境变量,不是 cfg)
    if let Ok(target_os) = std::env::var("CARGO_CFG_TARGET_OS") {
        if target_os == "android" {
            use std::path::Path;

            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
            let target_assets_dir =
                Path::new(&manifest_dir).join("gen/android/app/src/main/assets");

            // 复制 build 目录到 assets
            let build_dir = Path::new(&manifest_dir).parent().unwrap().join("build");
            if build_dir.exists() && target_assets_dir.exists() {
                println!(
                    "cargo:warning=Copying frontend assets from {:?} to {:?}",
                    build_dir, target_assets_dir
                );
                if let Err(e) = copy_dir_recursive(&build_dir, &target_assets_dir) {
                    println!("cargo:warning=Failed to copy assets: {}", e);
                } else {
                    println!(
                        "cargo:warning=Successfully copied frontend assets to Android assets directory"
                    );
                }
            } else {
                println!(
                    "cargo:warning=Build dir exists: {}, Target assets dir exists: {}",
                    build_dir.exists(),
                    target_assets_dir.exists()
                );
            }
        }
    }
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    use std::fs;
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}
