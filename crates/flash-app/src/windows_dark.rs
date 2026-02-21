/// Apply dark-mode title bar to our process's top-level window.
/// Calls DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE = 20).
/// This is a best-effort no-op on unsupported Windows versions.
#[cfg(target_os = "windows")]
pub fn apply() {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
    use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };

    let our_pid = std::process::id();

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let our_pid = lparam as u32;
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == our_pid && IsWindowVisible(hwnd) != 0 {
            // DWMWA_USE_IMMERSIVE_DARK_MODE = 20 (Windows 11 / patched Win10)
            // Fall back to attribute 19 for older Win10 builds (no-op if unsupported)
            let dark: u32 = 1;
            DwmSetWindowAttribute(
                hwnd,
                20,
                &dark as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
            // Also try attribute 19 for older Windows 10 builds
            DwmSetWindowAttribute(
                hwnd,
                19,
                &dark as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            );
        }
        TRUE
    }

    unsafe {
        EnumWindows(Some(callback), our_pid as LPARAM);
    }
}
