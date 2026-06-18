use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubtitleTrack {
    pub index: u32,
    pub stream_index: u32,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub default: bool,
    pub forced: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VideoInfo {
    pub path: String,
    pub filename: String,
    pub duration: Option<f64>,
    pub subtitle_tracks: Vec<SubtitleTrack>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupInfo {
    pub original_path: String,
    pub backup_path: String,
    pub track_index: u32,
    pub format: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractResult {
    pub success: bool,
    pub output_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
    pub data: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DialogLine {
    pub index: usize,
    pub text: String,
    pub original_with_formatting: String,
    pub start: String,
    pub end: String,
    pub style: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubtitleData {
    pub format: String,
    pub lines: Vec<DialogLine>,
    pub line_count: usize,
    pub source_path: String,
    pub ass_header: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationRequest {
    pub lines: Vec<TranslationLine>,
    pub source_lang: String,
    pub target_lang: String,
    pub style: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationLine {
    pub id: usize,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationResponse {
    pub translations: Vec<TranslatedLine>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslatedLine {
    pub id: usize,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationProgress {
    pub current_chunk: usize,
    pub total_chunks: usize,
    pub lines_translated: usize,
    pub total_lines: usize,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMConfig {
    pub provider: String,
    pub api_key: String,
    pub endpoint: String,
    pub model: String,
    pub system_prompt: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranslationJobRequest {
    pub video_paths: Vec<String>,
    pub config: LLMConfig,
    pub source_lang: String,
    pub target_lang: String,
    pub output_format: String,
    pub output_directory: Option<String>,
    pub ffmpeg_path: Option<String>,
    pub subtitle_track: Option<u32>,
    pub embed_subtitles: bool,
    pub use_mkvmerge: bool,
    pub auto_backup: bool,
    pub keep_original_track: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranslationJobProgress {
    pub current_file: usize,
    pub total_files: usize,
    pub progress: f64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranslationJobOutput {
    pub video_path: String,
    pub subtitle_path: Option<String>,
    pub embedded: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TranslationJobResult {
    pub completed_files: usize,
    pub total_files: usize,
    pub failures: Vec<String>,
    pub outputs: Vec<TranslationJobOutput>,
}
