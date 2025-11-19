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
        let open_folder = false;
        egui::SidePanel::left("left_side")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Settings");
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.collapsing("Translation", |ui| {
                        egui::Grid::new("translation_grid")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("Source Language:");
                                ui.text_edit_singleline(&mut state.source_language);
                                ui.end_row();

                                ui.label("Target Language:");
                                ui.text_edit_singleline(&mut state.target_language);
                                ui.end_row();
                            });
                        ui.checkbox(&mut state.preserve_honorifics, "Preserve honorifics");
                    });

                    ui.separator();

                    ui.collapsing("Provider", |ui| {
                        egui::Grid::new("provider_select_grid")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("Provider:");
                                egui::ComboBox::from_id_salt("provider_combo")
                                    .selected_text(&state.provider)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut state.provider,
                                            "gemini".to_string(),
                                            "Gemini",
                                        );
                                        ui.selectable_value(
                                            &mut state.provider,
                                            "openai".to_string(),
                                            "OpenAI",
                                        );
                                        ui.selectable_value(
                                            &mut state.provider,
                                            "ollama".to_string(),
                                            "Ollama",
                                        );
                                        ui.selectable_value(
                                            &mut state.provider,
                                            "lmstudio".to_string(),
                                            "LM Studio",
                                        );
                                    });
                                ui.end_row();
                            });

                        ui.add_space(5.0);

                        egui::Grid::new("provider_details_grid")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                ui.label("API Key:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut state.api_key).password(true),
                                );
                                ui.end_row();

                                ui.label("Model:");
                                ui.text_edit_singleline(&mut state.model);
                                ui.end_row();

                                ui.label("Base URL:");
                                ui.text_edit_singleline(&mut state.base_url);
                                ui.end_row();
                            });
                    });

                    ui.separator();

                    ui.collapsing("Processing", |ui| {
                        ui.checkbox(&mut state.dry_run, "Dry run (no file writes)");
                        ui.add_space(5.0);
                        if let Some(folder) = &state.selected_folder {
                            ui.label("Selected Folder:");
                            ui.monospace(folder.display().to_string());
                        } else if let Some(file) = &state.selected_file {
                            ui.label("Selected File:");
                            ui.monospace(file.display().to_string());
                        } else {
                            ui.label("No input selected");
                        }
                    });

                    ui.separator();
                    ui.collapsing("About", |ui| {
                        ui.label("Anime-focused subtitle translation tool.");
                        ui.hyperlink("https://github.com/enrell/animesubs");
                    });
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

            // Progress Section
            ui.group(|ui| {
                ui.heading("Status");
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
                    ui.add(
                        egui::ProgressBar::new(pct)
                            .text(format!("{:.0}%", pct * 100.0))
                            .animate(state.is_processing),
                    );
                } else {
                    ui.label("Idle. Ready to process.");
                }
            });

            ui.add_space(10.0);

            // Logs Section
            ui.heading("Logs");
            egui::Frame::dark_canvas(ui.style()).show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_min_height(200.0);
                        for line in &state.logs {
                            ui.monospace(line);
                        }
                    });
            });
        });
    }
}
