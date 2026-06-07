pub mod commands;
pub mod models;
pub mod providers;
pub mod utils;

use commands::{backup, embedding, subtitle, translation, utils as utility_commands, video};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            utility_commands::greet,
            video::get_video_info,
            video::scan_folder_for_videos,
            subtitle::extract_subtitle,
            backup::backup_subtitle,
            backup::list_backups,
            backup::restore_subtitle,
            backup::delete_backup,
            embedding::embed_subtitle,
            embedding::remove_subtitle_track,
            utility_commands::check_ffmpeg,
            utility_commands::delete_file,
            utility_commands::load_api_key,
            utility_commands::save_api_key,
            subtitle::parse_subtitle_file,
            translation::translate_subtitles,
            translation::save_translated_subtitles,
            translation::start_translation_job,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
