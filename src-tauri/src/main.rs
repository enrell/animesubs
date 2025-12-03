// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Fix AppImage EGL conflicts on Wayland systems
    // The bundled libwayland-egl.so conflicts with host GPU drivers
    // These env vars must be set BEFORE WebKit initializes
    #[cfg(target_os = "linux")]
    unsafe {
        // Disable DMABuf renderer which requires hardware EGL
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        // Disable compositing mode that triggers EGL initialization
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }

    animesubs_lib::run()
}
