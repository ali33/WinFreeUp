//! Hộp thoại gốc Windows (trước khi có WebView) — không bao giờ để màn trắng.
use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, IDYES, MB_ICONERROR, MB_OK, MB_YESNO};

const WEBVIEW2_URL: &str = "https://go.microsoft.com/fwlink/p/?LinkId=2124703";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn show_missing(detail: &str) {
    let text = format!(
        "WinFreeUp cần Microsoft Edge WebView2 Runtime để hiển thị giao diện, nhưng máy này chưa có.\n\n\
         Bấm Yes để mở trang tải chính thức của Microsoft (miễn phí). Cài xong, mở lại WinFreeUp.\n\n\
         Chi tiết: {detail}"
    );
    let (text, caption) = (wide(&text), wide("WinFreeUp"));
    let answer = unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_YESNO | MB_ICONERROR) };
    if answer == IDYES {
        let _ = std::process::Command::new("explorer.exe").arg(WEBVIEW2_URL).spawn();
    }
}

pub fn show_fatal(detail: &str) {
    let text = wide(&format!("WinFreeUp không khởi động được.\n\nChi tiết: {detail}"));
    let caption = wide("WinFreeUp");
    unsafe {
        MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_OK | MB_ICONERROR);
    }
}
