fn main() {
    // Manifest riêng (app.manifest): Admin ngay khi mở (UAC) + Common Controls v6 + supportedOS.
    // Không đưa qua tauri-build (resource winres gắn vào MỌI mục tiêu link, kể cả test harness ⇒
    // `cargo test` không nâng quyền gặp os error 740). Thay vào đó chỉ nhúng cho binary sản phẩm
    // bằng tham số linker `-bins`; /MANIFESTUAC:NO để linker không tự sinh trustInfo asInvoker chồng lên.
    let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("app.manifest");
    println!("cargo:rerun-if-changed=app.manifest");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());
        println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:NO");
    }
    let windows = tauri_build::WindowsAttributes::new_without_app_manifest();
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("failed to run tauri-build");
}
