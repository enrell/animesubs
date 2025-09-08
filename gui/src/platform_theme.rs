#[cfg(windows)]
pub fn detect_os_dark() -> bool {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    const PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
    const VALUE: &str = "AppsUseLightTheme"; // 0 = dark, 1 = light
    if let Ok(hkcu) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(PATH) {
        if let Ok(val) = hkcu.get_value::<u32, _>(VALUE) {
            let is_dark = val == 0;
            log::debug!("detect_os_dark: registry AppsUseLightTheme={} => is_dark={}", val, is_dark);
            return is_dark;
        } else {
            log::debug!("detect_os_dark: value not found in record");
        }
    } else {
        log::debug!("detect_os_dark: unable to open registry key");
    }
    log::debug!("detect_os_dark: fallback false (light)");
    false
}

#[cfg(target_os = "macos")]
pub fn detect_os_dark() -> bool {
    use core_foundation::preferences::CFPreferencesCopyAppValue;
    use core_foundation::string::{CFString, CFStringRef};
    // AppleInterfaceStyle present and == "Dark" when dark mode enabled
    let key = CFString::new("AppleInterfaceStyle");
    let app_id = CFString::new("NSGlobalDomain");
    unsafe {
        let value = CFPreferencesCopyAppValue(key.as_concrete_TypeRef(), app_id.as_concrete_TypeRef());
        if !value.is_null() {
            let s = CFString::wrap_under_get_rule(value as CFStringRef).to_string();
            return s.to_ascii_lowercase().contains("dark");
        }
    }
    false
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn detect_os_dark() -> bool { false }
