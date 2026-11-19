//! The folded map mark shared by the atlas UI and native window chrome.
use bevy::ecs::system::NonSendMarker;

pub(super) const PNG: &[u8] = include_bytes!("../../assets/branding/elmapa.png");
pub(super) const SYMBOL: &str = "elmapa-brand";

pub(super) fn icon_pixels(size: u32) -> Vec<u8> {
    let image = image::load_from_memory_with_format(PNG, image::ImageFormat::Png)
        .expect("Elmapa icon")
        .into_rgba8();
    image::imageops::resize(&image, size, size, image::imageops::FilterType::Lanczos3).into_raw()
}

#[cfg(target_os = "macos")]
pub(super) fn set_app_icon(_: NonSendMarker) {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let marker = MainThreadMarker::new().expect("application icon on main thread");
    let image =
        NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(PNG)).expect("Elmapa icon");
    // SAFETY: AppKit receives a valid NSImage on the main thread.
    unsafe { NSApplication::sharedApplication(marker).setApplicationIconImage(Some(&image)) };
}

#[cfg(not(target_os = "macos"))]
pub(super) fn set_window_icon(
    _: NonSendMarker,
    mut created: bevy::prelude::MessageReader<bevy::window::WindowCreated>,
) {
    use bevy::winit::WINIT_WINDOWS;
    use winit::window::Icon;

    for event in created.read() {
        WINIT_WINDOWS.with_borrow(|windows| {
            if let Some(window) = windows.get_window(event.window) {
                let icon = Icon::from_rgba(icon_pixels(64), 64, 64).expect("window icon");
                window.set_window_icon(Some(icon));
            }
        });
    }
}
