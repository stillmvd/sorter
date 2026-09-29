use tauri::{image::Image, WebviewWindow};

const ON_DARK: &[u8] = include_bytes!("../icons/taskbar-dark.png");
const ON_LIGHT: &[u8] = include_bytes!("../icons/taskbar-light.png");

pub fn apply(window: &WebviewWindow) {
    let bytes = if light_taskbar() { ON_LIGHT } else { ON_DARK };
    if let Ok(icon) = Image::from_bytes(bytes) {
        let _ = window.set_icon(icon);
    }
}

#[cfg(windows)]
fn light_taskbar() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD};
    let mut value = 0u32;
    let mut size = 4u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    status.is_ok() && value == 1
}

#[cfg(not(windows))]
fn light_taskbar() -> bool {
    false
}
