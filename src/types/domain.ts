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

export interface LocalServerStatus {
  available: boolean
  owned: boolean
  url: string | null
  version: string | null
  message: string
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
  contextProfile: ContextProfile | null
  batchSize?: number | null
  episodeConcurrency?: number | null
  concurrency?: number | null
}

export interface TranslationJobProgress {
  currentFile: number
  totalFiles: number
  progress: number
  status: string
}

export interface TranslationBatchProgress {
  current_chunk: number
  total_chunks: number
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

export interface CharacterProfile {
  name: string
  role: string | null
  description: string | null
  relationships: string[] | null
  verified: boolean
}

export interface TerminologyEntry {
  source_term: string
  translated_term: string
  notes: string | null
}

export interface MediaMatch {
  provider: string
  source_id: string
  title_display: string
  title_native: string | null
  title_english: string | null
  title_romaji: string | null
  synopsis: string | null
  genres: string[]
  tags: string[]
  aliases: string[]
  format: string | null
  episodes: number | null
  season: string | null
  season_year: number | null
}

export interface ContextProfile {
  title: string | null
  characters: CharacterProfile[]
  locations: string[]
  plot_summary: string | null
  terminology: TerminologyEntry[]
  tone_notes: string | null
  custom_instructions: string | null
  source_path: string | null
  created_at: string
  updated_at: string
  media_match: MediaMatch | null
}

export interface ContextExtractionRequest {
  filename: string
  subtitleData: SubtitleData
  config: LlmConfig
  sourceLang: string
  targetLang: string
  customInstructions: string | null
  tmdbApiKey?: string | null
}

export interface ContextExtractionResult {
  profile: ContextProfile
}
