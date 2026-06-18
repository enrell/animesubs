import { invoke } from '@tauri-apps/api/core'
import type {
  BackupInfo,
  ExtractResult,
  LlmConfig,
  OperationResult,
  SubtitleData,
  TranslationJobRequest,
  TranslationJobResult,
  VideoInfo
} from '../types/domain'

export const checkFfmpeg = (ffmpegPath?: string | null) =>
  invoke<OperationResult>('check_ffmpeg', { ffmpegPath: ffmpegPath || null })

export const getVideoInfo = (videoPath: string, ffmpegPath?: string | null) =>
  invoke<VideoInfo>('get_video_info', { videoPath, ffmpegPath: ffmpegPath || null })

export const scanFolderForVideos = (folderPath: string) =>
  invoke<string[]>('scan_folder_for_videos', { folderPath })

export const extractSubtitle = (params: {
  videoPath: string
  trackIndex: number
  outputPath?: string | null
  format?: string | null
  temporary?: boolean | null
  ffmpegPath?: string | null
}) => invoke<ExtractResult>('extract_subtitle', params)

export const parseSubtitleFile = (filePath: string) =>
  invoke<SubtitleData>('parse_subtitle_file', { filePath })

export const translateSubtitles = (params: {
  subtitleData: SubtitleData
  config: LlmConfig
  sourceLang: string
  targetLang: string
}) => invoke<SubtitleData>('translate_subtitles', params)

export const saveTranslatedSubtitles = (params: {
  translatedData: SubtitleData
  outputPath?: string | null
  originalFilePath?: string | null
  temporary?: boolean | null
}) => invoke<OperationResult>('save_translated_subtitles', params)

export const backupSubtitle = (videoPath: string, trackIndex: number, ffmpegPath?: string | null) =>
  invoke<BackupInfo>('backup_subtitle', { videoPath, trackIndex, ffmpegPath: ffmpegPath || null })

export const listBackups = (videoPath: string) =>
  invoke<BackupInfo[]>('list_backups', { videoPath })

export const restoreSubtitle = (params: {
  videoPath: string
  backupPath: string
  trackIndex: number
  ffmpegPath?: string | null
}) => invoke<OperationResult>('restore_subtitle', params)

export const deleteBackup = (backupPath: string, videoPath: string) =>
  invoke<OperationResult>('delete_backup', { backupPath, videoPath })

export const removeSubtitleTrack = (videoPath: string, trackIndex: number, ffmpegPath?: string | null) =>
  invoke<OperationResult>('remove_subtitle_track', { videoPath, trackIndex, ffmpegPath: ffmpegPath || null })

export const embedSubtitle = (params: {
  videoPath: string
  subtitlePath: string
  language: string
  title: string
  setDefault: boolean
  ffmpegPath?: string | null
  useMkvmerge: boolean
}) => invoke<OperationResult>('embed_subtitle', params)

export const deleteFile = (filePath: string) =>
  invoke<OperationResult>('delete_file', { filePath })

export const startTranslationJob = (request: TranslationJobRequest) =>
  invoke<TranslationJobResult>('start_translation_job', { request })

export const loadApiKey = (provider: string) =>
  invoke<OperationResult>('load_api_key', { provider })

export const saveApiKey = (provider: string, apiKey: string) =>
  invoke<OperationResult>('save_api_key', { provider, apiKey })

export const fetchModels = (endpoint: string, apiKey?: string | null, provider?: string | null) =>
  invoke<{ label: string; value: string }[]>('fetch_models', {
    endpoint,
    apiKey: apiKey || null,
    provider: provider || null
  })
