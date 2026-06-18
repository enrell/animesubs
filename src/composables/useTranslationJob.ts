import { listen } from '@tauri-apps/api/event'
import { computed, ref, type Ref } from 'vue'
import { startTranslationJob } from '../api/animesubs'
import {
  hasUsableApiConfig,
  providerRequiresApiKey,
  type Settings
} from '../config/settings'
import type {
  OperationResult,
  SelectedFile,
  TranslationBatchProgress,
  TranslationJobProgress
} from '../types/domain'
import type { SettingsModalExpose } from './useSettingsState'
import type { TranslationOptions } from './useTranslationOptions'

const validateApiConnection = async (settings: Settings): Promise<{ valid: boolean; error?: string }> => {
  try {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json'
    }

    if (settings.apiKey) {
      headers.Authorization = `Bearer ${settings.apiKey}`
    }

    let url = `${settings.apiEndpoint}/models`

    if (settings.provider === 'gemini' && !settings.apiEndpoint.includes('/openai')) {
      url = `https://generativelanguage.googleapis.com/v1beta/models?key=${settings.apiKey}`
      delete headers.Authorization
    }

    const response = await fetch(url, {
      headers,
      signal: AbortSignal.timeout(10000)
    })

    if (!response.ok) {
      const text = await response.text()
      if (response.status === 401) return { valid: false, error: 'Invalid API key. Please check your credentials in Settings.' }
      if (response.status === 403) return { valid: false, error: 'Access denied. Check API key permissions.' }
      if (response.status === 429) return { valid: false, error: 'Rate limited. Please wait and try again.' }
      return { valid: false, error: `API error (${response.status}): ${text.slice(0, 100)}` }
    }

    return { valid: true }
  } catch (e) {
    if (e instanceof Error) {
      if (e.name === 'AbortError' || e.name === 'TimeoutError') return { valid: false, error: 'Connection timeout. Check endpoint URL and network.' }
      if (e.message.includes('fetch')) return { valid: false, error: 'Cannot connect to API. Check endpoint URL.' }
      return { valid: false, error: `Connection failed: ${e.message}` }
    }
    return { valid: false, error: 'Unknown connection error' }
  }
}

interface UseTranslationJobParams {
  selectedFiles: Ref<SelectedFile[]>
  cachedSettings: Ref<Settings | null>
  ffmpegStatus: Ref<OperationResult | null>
  translationOptions: TranslationOptions
  settingsRef: Ref<SettingsModalExpose | null>
  showSettings: Ref<boolean>
  getSettings: () => Settings | null
}

export const useTranslationJob = ({
  selectedFiles,
  cachedSettings,
  ffmpegStatus,
  translationOptions,
  settingsRef,
  showSettings,
  getSettings
}: UseTranslationJobParams) => {
  const isTranslating = ref(false)
  const translationProgress = ref(0)
  const currentStatus = ref('')
  const estimatedTime = ref('')
  const currentFileIndex = ref(0)

  const setProgress = (value: number) => {
    const clamped = Math.min(100, Math.max(0, Number.isFinite(value) ? value : 0))
    translationProgress.value = clamped
  }

  const resetProgress = () => {
    setProgress(0)
    currentStatus.value = ''
    estimatedTime.value = ''
  }

  const canStartTranslation = computed(() => {
    const settings = cachedSettings.value
    const hasApiConfig = hasUsableApiConfig(settings)
    const hasFiles = selectedFiles.value.length > 0
    const filesReady = selectedFiles.value.some(f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0)
    return hasApiConfig && hasFiles && filesReady && ffmpegStatus.value?.success
  })

  const startTranslation = async () => {
    if (!canStartTranslation.value) {
      if (!ffmpegStatus.value?.success) {
        showSettings.value = true
      } else if (providerRequiresApiKey(getSettings()?.provider) && !getSettings()?.apiKey) {
        showSettings.value = true
      } else if (!getSettings()?.selectedModel) {
        showSettings.value = true
      }
      return
    }

    const settings = getSettings()
    if (!settings) return

    currentStatus.value = 'Validating API connection...'
    const validation = await validateApiConnection(settings)
    if (!validation.valid) {
      currentStatus.value = validation.error || 'API validation failed'
      return
    }

    const filesToProcess = selectedFiles.value.filter(
      f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0
    )
    const videoPaths = filesToProcess.map(file => file.path)
    const systemPrompt = settingsRef.value?.getSystemPrompt?.()
      || `You are a professional subtitle translator. Translate the following subtitle lines to ${settings.targetLanguage}. Keep translations natural and contextually appropriate for anime dialogue.`

    isTranslating.value = true
    setProgress(0)
    currentFileIndex.value = 0
    const latestJobProgress = ref<TranslationJobProgress | null>(null)

    const unlistenProgress = await listen<TranslationJobProgress>('translation-job-progress', (event) => {
      latestJobProgress.value = event.payload
      currentFileIndex.value = Math.max(0, event.payload.currentFile - 1)
      setProgress(event.payload.progress)
      currentStatus.value = event.payload.status
    })
    const unlistenBatchProgress = await listen<TranslationBatchProgress>('translation-progress', (event) => {
      const jobProgress = latestJobProgress.value
      if (!jobProgress?.totalFiles) return

      const chunkRatio = event.payload.total_chunks > 0
        ? event.payload.current_chunk / event.payload.total_chunks
        : 0
      const fileBase = ((jobProgress.currentFile - 1) / jobProgress.totalFiles) * 100
      const fileSpan = 100 / jobProgress.totalFiles
      setProgress(fileBase + ((0.2 + (0.6 * chunkRatio)) * fileSpan))
      currentStatus.value = event.payload.status
        || `Translating ${event.payload.lines_translated}/${event.payload.total_lines} lines...`
    })

    try {
      const result = await startTranslationJob({
        videoPaths,
        config: {
          provider: settings.provider,
          api_key: settings.apiKey,
          endpoint: settings.apiEndpoint,
          model: settings.selectedModel || '',
          system_prompt: systemPrompt
        },
        sourceLang: settings.sourceLanguage || 'auto',
        targetLang: settings.targetLanguage,
        outputFormat: settings.outputFormat || 'srt',
        outputDirectory: settings.outputDirectory || null,
        ffmpegPath: settings.ffmpegPath || null,
        subtitleTrack: translationOptions.subtitleTrack
          ? parseInt(translationOptions.subtitleTrack)
          : null,
        embedSubtitles: translationOptions.embedSubtitles,
        useMkvmerge: translationOptions.useMkvmerge,
        autoBackup: settings.autoBackup,
        keepOriginalTrack: settings.keepOriginalTrack
      })

      setProgress(100)
      if (result.failures.length === 0) {
        currentStatus.value = 'Translation complete!'
      } else if (result.completedFiles === 0) {
        currentStatus.value = `Translation failed: ${result.failures[0]}`
      } else {
        currentStatus.value = `Translation finished with errors (${result.completedFiles}/${result.totalFiles}): ${result.failures[0]}`
      }
    } catch (e) {
      console.error('Translation error:', e)
      currentStatus.value = `Error: ${e}`
    } finally {
      unlistenProgress()
      unlistenBatchProgress()
      isTranslating.value = false
    }
  }

  return {
    isTranslating,
    translationProgress,
    currentStatus,
    estimatedTime,
    currentFileIndex,
    canStartTranslation,
    setProgress,
    resetProgress,
    startTranslation
  }
}
