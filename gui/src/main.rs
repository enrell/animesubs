#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gui::detect_os_dark;

trait ViewportBuilderIconExt {
    fn with_icon_if_some(self, icon: Option<egui::IconData>) -> Self;
}

impl ViewportBuilderIconExt for egui::ViewportBuilder {
    fn with_icon_if_some(self, icon: Option<egui::IconData>) -> Self {
        if let Some(icon) = icon {
            self.with_icon(icon)
        } else {
            self
        }
    }
}

fn main() -> eframe::Result {
    let mut builder = env_logger::Builder::new();
    builder.filter_level(log::LevelFilter::Info);

    #[cfg(debug_assertions)]
    {
        // Carrega exclusivamente do arquivo .env local se existir
        let debug_enabled = std::fs::read_to_string(".env")
            .ok()
            .and_then(|content| {
                for line in content.lines() {
                    let line = line.trim();
                    if line.starts_with('#') || line.is_empty() { continue; }
                    if let Some(rest) = line.strip_prefix("GUI_DEBUG=") {
                        return Some(rest.trim_matches(|c| c=='"' || c=='\'')).map(|v| v.to_string());
                    }
                }
                None
            })
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        if debug_enabled { builder.filter_level(log::LevelFilter::Debug); }
    }
    builder.init();

    // Pre-detect system theme (Windows registry; fallback light=false)
    let dark_pref = detect_os_dark();
    log::debug!("main: dark_pref={}", dark_pref);
    let initial_icon = if dark_pref {
        eframe::icon_data::from_png_bytes(&include_bytes!("../assets/dark_icon.png")[..])
    } else {
        eframe::icon_data::from_png_bytes(&include_bytes!("../assets/light_icon.png")[..])
    }
    .ok();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 400.0])
            .with_always_on_top()
            .with_icon_if_some(initial_icon),
        ..Default::default()
    };

    eframe::run_native(
        "AnimeSubs",
        native_options,
        Box::new(|cc| Ok(Box::new(gui::Home::new_with_forced_theme(cc, dark_pref)))),
    )
}
