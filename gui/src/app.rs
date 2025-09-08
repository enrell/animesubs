//! Main application wiring. High-level orchestration & persistence.
//!
//! This file keeps only the minimal glue. Most logic lives in:
//! - state.rs (pure state & data structures)
//! - actions.rs (operations that mutate state)
//! - ui/ (pure egui composition widgets)

use crate::actions::{select_folder, start_processing};
use crate::state::AppState;
use crate::ui;
use std::sync::Arc;
use std::time::{Duration, Instant};
#[cfg(any(windows, target_os = "macos"))]
use crate::detect_os_dark;

fn load_icon_from_bytes(bytes: &[u8]) -> Result<Arc<egui::IconData>, Box<dyn std::error::Error>> {
    let image = image::load_from_memory(bytes)?.into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();

    Ok(Arc::new(egui::IconData {
        rgba,
        width: width as u32,
        height: height as u32,
    }))
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct Home {
    /// Persistent application state (serializable).
    pub state: AppState,

    // --- Ephemeral (not persisted) ---
    #[serde(skip)]
    file_dialog: Option<egui_file::FileDialog>,
    #[serde(skip)]
    last_error: Option<String>,
    #[serde(skip)]
    last_theme_dark: Option<bool>,
    #[serde(skip)]
    last_theme_check: Option<Instant>,
}

impl Default for Home {
    fn default() -> Self {
        Self {
            state: AppState::default(),
            file_dialog: None,
            last_error: None,
            last_theme_dark: None,
            last_theme_check: None,
        }
    }
}

impl Home {
    pub fn new_with_forced_theme(cc: &eframe::CreationContext<'_>, forced_dark: bool) -> Self {
        let mut app = if let Some(storage) = cc.storage {
            if let Some(app) = eframe::get_value::<Home>(storage, eframe::APP_KEY) {
                app
            } else {
                Home::default()
            }
        } else {
            Home::default()
        };

        app.last_theme_dark = Some(forced_dark);
        app.last_theme_check = Some(Instant::now());
        if let Ok(icon) = Self::get_icon_for_theme(forced_dark) {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Icon(Some(icon)));
        }
        app
    }
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Try restore from storage but reset theme state
        let mut app = if let Some(storage) = cc.storage {
            if let Some(app) = eframe::get_value::<Home>(storage, eframe::APP_KEY) {
                app
            } else {
                Home::default()
            }
        } else {
            Home::default()
        };

        // Always reset theme-related state on startup to avoid persistence issues
        let is_dark = cc.egui_ctx.style().visuals.dark_mode;
        app.last_theme_dark = Some(is_dark);
        app.last_theme_check = Some(Instant::now());

        if let Ok(icon) = Self::get_icon_for_theme(is_dark) {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Icon(Some(icon)));
        }
        app
    }

    fn get_icon_for_theme(
        is_dark: bool,
    ) -> Result<Arc<egui::IconData>, Box<dyn std::error::Error>> {
        let icon_bytes: &[u8] = if is_dark {
            include_bytes!("../assets/dark_icon.png")
        } else {
            include_bytes!("../assets/light_icon.png")
        };

        load_icon_from_bytes(icon_bytes)
    }

    fn select_folder(&mut self) {
        let mut dialog = egui_file::FileDialog::select_folder(self.state.selected_folder.clone());
        dialog.open();
        self.file_dialog = Some(dialog);
    }

    fn select_file(&mut self) {
        let mut dialog = egui_file::FileDialog::open_file(self.state.selected_file.clone());
        dialog.open();
        self.file_dialog = Some(dialog);
    }
}

impl eframe::App for Home {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let should_check_theme = self
            .last_theme_check
            .map(|last| now.duration_since(last) > Duration::from_secs(1))
            .unwrap_or(true);

        if should_check_theme {
            // Prefer OS-level detection on supported platforms; fall back to egui visuals otherwise
            #[cfg(any(windows, target_os = "macos"))]
            let current_theme_dark = detect_os_dark();
            #[cfg(not(any(windows, target_os = "macos")))]
            let current_theme_dark = ctx.style().visuals.dark_mode;
            if self.last_theme_dark != Some(current_theme_dark) {
                self.last_theme_dark = Some(current_theme_dark);

                if let Ok(icon) = Self::get_icon_for_theme(current_theme_dark) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(icon)));
                }
            }
            self.last_theme_check = Some(now);
        }

        // Top bar & menus
        ui::top_bar::top_bar(ctx, self, |this, action| match action {
            ui::top_bar::TopBarAction::SelectFile => this.select_file(),
            ui::top_bar::TopBarAction::SelectFolder => this.select_folder(),
            ui::top_bar::TopBarAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            ui::top_bar::TopBarAction::ClearLogs => this.state.logs.clear(),
            ui::top_bar::TopBarAction::StartProcessing => {
                if !this.state.is_processing {
                    if let Err(e) = start_processing(&mut this.state) {
                        this.last_error = Some(e.to_string());
                    }
                }
            }
        });

        // Layout panels
        if ui::side_panel::side_panel(ctx, &mut self.state) {
            self.select_folder();
        }
        ui::main_panel::main_panel(ctx, &mut self.state, self.last_error.as_deref());

        if let Some(dialog) = &mut self.file_dialog {
            if dialog.show(ctx).selected() {
                if let Some(path) = dialog.path() {
                    if let Err(e) = select_folder(&mut self.state, path) {
                        self.last_error = Some(e.to_string());
                    }
                }
                self.file_dialog = None;
            }
        }

        // Trigger repaint while processing (simple polling loop)
        if self.state.is_processing {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}
