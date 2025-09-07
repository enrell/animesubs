//! UI composition modules.
pub mod top_bar {
    use super::super::Home;
    #[derive(Debug, Clone, Copy)]
    pub enum TopBarAction {
        SelectFile,
        SelectFolder,
        Quit,
        ClearLogs,
        StartProcessing,
    }

    pub fn top_bar(
        ctx: &egui::Context,
        app: &mut Home,
        mut handle: impl FnMut(&mut Home, TopBarAction),
    ) {
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Import file").clicked() {
                        handle(app, TopBarAction::SelectFile);
                        ui.close();
                    }
                    if ui.button("Import folder").clicked() {
                        handle(app, TopBarAction::SelectFolder);
                    }
                    if ui.button("Quit").clicked() {
                        handle(app, TopBarAction::Quit);
                    }
                });
                ui.menu_button("Run", |ui| {
                    if ui.button("Start Processing").clicked() {
                        handle(app, TopBarAction::StartProcessing);
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Clear Logs").clicked() {
                        handle(app, TopBarAction::ClearLogs);
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_buttons(ui);
                    ui.label("AnimeSubs");
                });
            });
        });
    }
}

pub mod side_panel {
    use crate::AppState;
    pub fn side_panel(ctx: &egui::Context, state: &mut AppState) -> bool {
        let mut open_folder = false;
        egui::SidePanel::left("left_side")
            .resizable(true)
            .default_width(240.0)
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.separator();
                ui.label("Target language (e.g. pt-BR, en, es)");
                ui.text_edit_singleline(&mut state.target_language);
                ui.checkbox(&mut state.preserve_honorifics, "Preserve honorifics");
                ui.checkbox(&mut state.dry_run, "Dry run (no file writes)");
                if let Some(folder) = &state.selected_folder {
                    ui.label(format!("Folder: {}", folder.display()));
                } else {
                    ui.label("No folder selected");
                }
                if ui.button("Select Folder...").clicked() {
                    open_folder = true;
                }
                ui.separator();
                ui.collapsing("About", |ui| {
                    ui.label("Anime-focused subtitle translation tool.");
                });
            });
        open_folder
    }
}

pub mod main_panel {
    use crate::AppState;

    pub fn main_panel(ctx: &egui::Context, state: &mut AppState, last_error: Option<&str>) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Anime Subtitle Translation");
            if let Some(err) = last_error {
                ui.colored_label(egui::Color32::RED, err);
            }
            ui.separator();
            if let Some(progress) = &state.progress {
                let pct = if progress.total_files == 0 {
                    0.0
                } else {
                    progress.processed as f32 / progress.total_files as f32
                };
                ui.label(format!(
                    "Progress: {} / {} ({} skipped, {} failed)",
                    progress.processed, progress.total_files, progress.skipped, progress.failed
                ));
                ui.add(egui::ProgressBar::new(pct).text(format!("{:.0}%", pct * 100.0)));
            } else {
                ui.label("Idle.");
            }
            ui.separator();
            ui.label("Logs:");
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for line in &state.logs {
                        ui.monospace(line);
                    }
                });
        });
    }
}
