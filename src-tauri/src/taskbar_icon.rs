use tauri::{image::Image, WebviewWindow};

const ICONS: [(u32, &[u8]); 4] = [
    (24, include_bytes!("../icons/taskbar-24.png")),
    (30, include_bytes!("../icons/taskbar-30.png")),
    (36, include_bytes!("../icons/taskbar-36.png")),
    (48, include_bytes!("../icons/taskbar-48.png")),
];

pub fn apply(window: &WebviewWindow) {
    let want = (24.0 * window.scale_factor().unwrap_or(1.0)).round() as u32;
    let (_, bytes) = ICONS.iter().find(|(px, _)| *px >= want).unwrap_or(&ICONS[ICONS.len() - 1]);
    if let Ok(icon) = Image::from_bytes(bytes) {
        let _ = window.set_icon(icon);
    }
}
