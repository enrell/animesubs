#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
    env_logger::init();

    // Pre-detect system theme (Windows registry; fallback light=false)
    let dark_pref = detect_os_dark();
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

#[cfg(windows)]
fn detect_os_dark() -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;
    // Windows stores: 0 = dark, 1 = light
    const PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
    const VALUE: &str = "AppsUseLightTheme";
    if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(PATH) {
        if let Ok(val) = hkcu.get_value::<u32, _>(VALUE) {
            return val == 0;
        }
    }
    false
}

#[cfg(not(windows))]
fn detect_os_dark() -> bool {
    #[cfg(target_os = "macos")]
    {
        use core_foundation::base::{CFRelease, TCFType};
        use core_foundation::preferences::CFPreferencesCopyAppValue;
        use core_foundation::string::{CFString, CFStringRef};
        // Key: AppleInterfaceStyle (exists and equals "Dark" when dark mode enabled)
        let key = CFString::new("AppleInterfaceStyle");
        let app_id = CFString::new("NSGlobalDomain");
        unsafe {
            let value_ref =
                CFPreferencesCopyAppValue(key.as_concrete_TypeRef(), app_id.as_concrete_TypeRef());
            if !value_ref.is_null() {
                let cf_str = CFString::wrap_under_get_rule(value_ref as CFStringRef);
                let is_dark = cf_str.to_string().to_ascii_lowercase().contains("dark");
                // value_ref retained by wrap_under_get_rule; no manual release needed here.
                return is_dark;
            }
        }
        return false;
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
