fn main() {
    // Manifest riêng: Admin ngay khi mở (UAC) + Common Controls v6 (tauri-build yêu cầu giữ khi thay manifest).
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("app.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("failed to run tauri-build");
}
