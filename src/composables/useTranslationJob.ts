import { listen } from '@tauri-apps/api/event'
import { computed, ref, type Ref } from 'vue'
import { startTranslationJob } from '../api/animesubs'
import {
  hasUsableApiConfig,
  providerRequiresApiKey,
  type Settings
} from '../config/settings'
import { localizeBackendMessage } from '../i18n'
import type {
  OperationResult,
  SelectedFile,
  TranslationBatchProgress,
  TranslationJobProgress
} from '../types/domain'
import type { SettingsModalExpose } from './useSettingsState'
import type { TranslationOptions } from './useTranslationOptions'

type TranslateFn = (key: string, named?: Record<string, unknown>) => string

const validateApiConnection = async (
  settings: Settings,
  t: TranslateFn
): Promise<{ valid: boolean; error?: string }> => {
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
      if (response.status === 401) return { valid: false, error: t('status.invalidApiKey') }
      if (response.status === 403) return { valid: false, error: t('status.accessDenied') }
      if (response.status === 429) return { valid: false, error: t('status.rateLimited') }
      return {
        valid: false,
        error: t('status.apiError', { status: response.status, details: text.slice(0, 100) })
      }
    }

    return { valid: true }
  } catch (e) {
    if (e instanceof Error) {
      if (e.name === 'AbortError' || e.name === 'TimeoutError') {
        return { valid: false, error: t('status.connectionTimeout') }
      }
      if (e.message.includes('fetch')) return { valid: false, error: t('status.cannotConnect') }
      return { valid: false, error: t('status.connectionFailed', { message: e.message }) }
    }
    return { valid: false, error: t('status.unknownConnectionError') }
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
  t: TranslateFn
}

export const useTranslationJob = ({
  selectedFiles,
  cachedSettings,
  ffmpegStatus,
  translationOptions,
  settingsRef,
  showSettings,
  getSettings,
  t
}: UseTranslationJobParams) => {
  const isTranslating = ref(false)
  const translationProgress = ref(0)
  const currentStatus = ref('')
  const estimatedTime = ref('')
  const currentFileIndex = ref(0)
  let queuedProgress: number | null = null
  let queuedStatus: string | null = null
  let progressFrame: number | null = null

  const setProgress = (value: number) => {
    const clamped = Math.min(100, Math.max(0, Number.isFinite(value) ? value : 0))
    translationProgress.value = clamped
  }

  const flushProgressUpdate = () => {
    progressFrame = null
    if (queuedProgress !== null) {
      setProgress(queuedProgress)
      queuedProgress = null
    }
    if (queuedStatus !== null) {
      currentStatus.value = queuedStatus
      queuedStatus = null
    }
  }

  const queueProgressUpdate = (progress: number, status?: string) => {
    queuedProgress = progress
    if (status !== undefined) {
      queuedStatus = status
    }

    if (progressFrame === null) {
      progressFrame = window.requestAnimationFrame(flushProgressUpdate)
    }
  }

  const resetProgress = () => {
    if (progressFrame !== null) {
      window.cancelAnimationFrame(progressFrame)
      progressFrame = null
    }
    queuedProgress = null
    queuedStatus = null
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

    currentStatus.value = t('status.validatingApi')
    const validation = await validateApiConnection(settings, t)
    if (!validation.valid) {
      currentStatus.value = validation.error || t('status.apiValidationFailed')
      return
    }

    const filesToProcess = selectedFiles.value.filter(
      f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0
    )
    const videoPaths = filesToProcess.map(file => file.path)
    const systemPrompt = settingsRef.value?.getSystemPrompt?.()
      || t('prompts.fallbackSystemPrompt', { targetLanguage: settings.targetLanguage })

    isTranslating.value = true
    setProgress(0)
    currentFileIndex.value = 0
    const latestJobProgress = ref<TranslationJobProgress | null>(null)

    const unlistenProgress = await listen<TranslationJobProgress>('translation-job-progress', (event) => {
      latestJobProgress.value = event.payload
      currentFileIndex.value = Math.max(0, event.payload.currentFile - 1)
      queueProgressUpdate(
        event.payload.progress,
        localizeBackendMessage(event.payload.status, t)
      )
    })
    const unlistenBatchProgress = await listen<TranslationBatchProgress>('translation-progress', (event) => {
      const jobProgress = latestJobProgress.value
      if (!jobProgress?.totalFiles) return

      const chunkRatio = event.payload.total_chunks > 0
        ? event.payload.current_chunk / event.payload.total_chunks
        : 0
      const fileBase = ((jobProgress.currentFile - 1) / jobProgress.totalFiles) * 100
      const fileSpan = 100 / jobProgress.totalFiles
      queueProgressUpdate(
        fileBase + ((0.2 + (0.6 * chunkRatio)) * fileSpan),
        event.payload.status
        ? localizeBackendMessage(event.payload.status, t)
        : t('status.translatingLines', {
          translated: event.payload.lines_translated,
          total: event.payload.total_lines
        })
      )
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
        outputFormat: settings.outputFormat,
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

      flushProgressUpdate()
      setProgress(100)
      if (result.failures.length === 0) {
        currentStatus.value = t('status.translationComplete')
      } else if (result.completedFiles === 0) {
        currentStatus.value = t('status.translationFailed', {
          failure: localizeBackendMessage(result.failures[0], t)
        })
      } else {
        currentStatus.value = t('status.translationFinishedWithErrors', {
          completed: result.completedFiles,
          total: result.totalFiles,
          failure: localizeBackendMessage(result.failures[0], t)
        })
      }
    } catch (e) {
      console.error('Translation error:', e)
      currentStatus.value = t('status.error', { error: e })
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
