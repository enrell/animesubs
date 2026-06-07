export interface SubtitleTrack {
  index: number
  stream_index: number
  codec: string
  language: string | null
  title: string | null
  default: boolean
  forced: boolean
}

export interface VideoInfo {
  path: string
  filename: string
  duration: number | null
  subtitle_tracks: SubtitleTrack[]
}

export interface BackupInfo {
  original_path: string
  backup_path: string
  track_index: number
  format: string
  created_at: string
}

export interface OperationResult {
  success: boolean
  message: string
  data: string | null
}

export interface ExtractResult {
  success: boolean
  output_path: string | null
  error: string | null
}

export interface DialogLine {
  index: number
  start: string
  end: string
  text: string
  original_with_formatting: string
  style: string | null
  name: string | null
}

export interface SubtitleData {
  format: string
  line_count: number
  lines: DialogLine[]
  source_path: string | null
  ass_header: string | null
}

export interface SelectedFile {
  name: string
  path: string
  size: number
  videoInfo: VideoInfo | null
  backups: BackupInfo[]
  loading: boolean
  error: string | null
}

export interface LlmConfig {
  provider: string
  api_key: string
  endpoint: string
  model: string
  system_prompt: string
}

export interface TranslationJobRequest {
  videoPaths: string[]
  config: LlmConfig
  sourceLang: string
  targetLang: string
  outputFormat: string
  outputDirectory: string | null
  ffmpegPath: string | null
  subtitleTrack: number | null
  embedSubtitles: boolean
  useMkvmerge: boolean
  autoBackup: boolean
  keepOriginalTrack: boolean
  batchSize: number
  concurrency: number
  requestDelay: number
}

export interface TranslationJobProgress {
  currentFile: number
  totalFiles: number
  progress: number
  status: string
}

export interface TranslationBatchProgress {
  current_batch: number
  total_batches: number
  lines_translated: number
  total_lines: number
  status: string
}

export interface TranslationJobOutput {
  videoPath: string
  subtitlePath: string | null
  embedded: boolean
}

export interface TranslationJobResult {
  completedFiles: number
  totalFiles: number
  failures: string[]
  outputs: TranslationJobOutput[]
}
