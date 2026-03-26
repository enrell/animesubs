mod models;
mod utils;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::video::get_video_info,
            commands::video::scan_folder_for_videos,
            commands::subtitle::extract_subtitle,
            commands::subtitle::parse_subtitle_file,
            commands::translation::translate_subtitles,
            commands::translation::save_translated_subtitles,
            commands::backup::backup_subtitle,
            commands::backup::list_backups,
            commands::backup::restore_subtitle,
            commands::backup::delete_backup,
            commands::embedding::embed_subtitle,
            commands::embedding::remove_subtitle_track,
            commands::utils::check_ffmpeg,
            commands::utils::greet,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
