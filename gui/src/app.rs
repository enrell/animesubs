//! Main application wiring. High-level orchestration & persistence.
//!
//! This file keeps only the minimal glue. Most logic lives in:
//! - state.rs (pure state & data structures)
//! - actions.rs (operations that mutate state)
//! - ui/ (pure egui composition widgets)

use crate::actions::{select_folder, start_processing};
use crate::state::AppState;
use crate::ui;

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
}

impl Default for Home {
    fn default() -> Self {
        Self {
            state: AppState::default(),
            file_dialog: None,
            last_error: None,
        }
    }
}

impl Home {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Try restore from storage
        if let Some(storage) = cc.storage {
            // persistence via `persistence` feature
            if let Some(app) = eframe::get_value::<Home>(storage, eframe::APP_KEY) {
                return app;
            }
        }
        Default::default()
    }

    fn select_folder(&mut self) {
        let mut dialog = egui_file::FileDialog::select_folder(self.state.selected_folder.clone());
        dialog.open();
        self.file_dialog = Some(dialog);
    }

    fn select_file(&mut self) {
        let mut dialog = egui_file::FileDialog::open_file(
            self.state.selected_file.clone()
        );
        dialog.open();
        self.file_dialog = Some(dialog);
    }
}

impl eframe::App for Home {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
