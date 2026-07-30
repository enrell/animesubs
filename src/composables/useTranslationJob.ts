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
  ContextProfile,
  OperationResult,
  SelectedFile,
  TranslationBatchProgress,
  TranslationJobProgress
} from '../types/domain'
import type { TranslationOptions } from './useTranslationOptions'
import { useActivityLog } from './useActivityLog'

type TranslateFn = (key: string, named?: Record<string, unknown>) => string

export interface FileOutcome {
  status: 'done' | 'failed'
  detail?: string
  embedded?: boolean
}

export type TranslationPhase = 'extract' | 'translate' | 'write' | 'other'

const classifyPhase = (rawStatus: string): TranslationPhase => {
  if (/Extracting|Parsing/.test(rawStatus)) return 'extract'
  if (/Translating/.test(rawStatus)) return 'translate'
  if (/Saving|Embedding/.test(rawStatus)) return 'write'
  return 'other'
}

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
  showSettings: Ref<boolean>
  getSettings: () => Settings | null
  t: TranslateFn
  contextProfile: Ref<ContextProfile | null>
  contextEnabled: Ref<boolean>
}

export const useTranslationJob = ({
  selectedFiles,
  cachedSettings,
  ffmpegStatus,
  translationOptions,
  showSettings,
  getSettings,
  t,
  contextProfile,
  contextEnabled
}: UseTranslationJobParams) => {
  const { log } = useActivityLog()

  const isTranslating = ref(false)
  const translationProgress = ref(0)
  const currentStatus = ref('')
  const estimatedTime = ref('')
  const currentFileIndex = ref(0)
  const batchPaths = ref<string[]>([])
  const fileOutcomes = ref<Record<string, FileOutcome>>({})
  const activeFilePhase = ref<TranslationPhase>('other')
  const activeFileLines = ref<{ done: number; total: number } | null>(null)
  const activeFilePaths = ref<Set<string>>(new Set())

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
    activeFilePhase.value = 'other'
    activeFileLines.value = null
  }

  const clearOutcome = (path: string) => {
    const { [path]: _removed, ...rest } = fileOutcomes.value
    fileOutcomes.value = rest
  }

  const clearAllOutcomes = () => {
    fileOutcomes.value = {}
  }

  const activeFilePath = computed(() =>
    isTranslating.value ? batchPaths.value[currentFileIndex.value] ?? null : null
  )

  const activeFileName = computed(() => {
    const path = activeFilePath.value
    return path ? path.split('/').pop() || path : ''
  })

  const canStartTranslation = computed(() => {
    const settings = cachedSettings.value
    const hasApiConfig = hasUsableApiConfig(settings)
    const hasFiles = selectedFiles.value.length > 0
    const allProbed = selectedFiles.value.every(f => !f.loading)
    const filesReady = selectedFiles.value.some(f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0)
    return hasApiConfig && hasFiles && allProbed && filesReady && ffmpegStatus.value?.success
  })

  const startTranslation = async (paths?: string[]) => {
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
    log(t('status.validatingApi'))
    const validation = await validateApiConnection(settings, t)
    if (!validation.valid) {
      currentStatus.value = validation.error || t('status.apiValidationFailed')
      log(currentStatus.value, 'error')
      return
    }

    const pathFilter = paths ? new Set(paths) : null
    const filesToProcess = selectedFiles.value.filter(
      f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0
        && (!pathFilter || pathFilter.has(f.path))
    )
    if (filesToProcess.length === 0) return

    const videoPaths = filesToProcess.map(file => file.path)
    for (const path of videoPaths) {
      clearOutcome(path)
    }

    const systemPrompt = settings.customSystemPrompt?.trim() || ''

    isTranslating.value = true
    batchPaths.value = videoPaths
    setProgress(0)
    currentFileIndex.value = 0
    activeFileLines.value = null
    activeFilePhase.value = 'other'
    log(t('log.jobStarted', { count: videoPaths.length }))
    const latestJobProgress = ref<TranslationJobProgress | null>(null)

    const unlistenProgress = await listen<TranslationJobProgress>('translation-job-progress', (event) => {
      latestJobProgress.value = event.payload
      currentFileIndex.value = Math.max(0, event.payload.currentFile - 1)
      activeFilePhase.value = classifyPhase(event.payload.status || '')
      queueProgressUpdate(
        event.payload.progress,
        localizeBackendMessage(event.payload.status, t)
      )
    })
    const unlistenBatchProgress = await listen<TranslationBatchProgress>('translation-progress', (event) => {
      const jobProgress = latestJobProgress.value
      if (!jobProgress?.totalFiles) return

      activeFileLines.value = {
        done: event.payload.lines_translated,
        total: event.payload.total_lines
      }

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
    const unlistenActiveFiles = await listen<string[]>('translation-active-files', (event) => {
      activeFilePaths.value = new Set(event.payload)
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
        keepOriginalTrack: settings.keepOriginalTrack,
        contextProfile: contextEnabled.value ? contextProfile.value : null,
        batchSize: settings.batchSize || null,
        episodeConcurrency: settings.episodeConcurrency || null,
        concurrency: settings.concurrency || null
      })

      flushProgressUpdate()
      setProgress(100)

      const succeededPaths = new Set(result.outputs.map(o => o.videoPath))
      const outcomes: Record<string, FileOutcome> = { ...fileOutcomes.value }
      for (const output of result.outputs) {
        outcomes[output.videoPath] = { status: 'done', embedded: output.embedded }
        const name = output.videoPath.split('/').pop() || output.videoPath
        log(t('log.fileDone', { file: name }), 'success')
      }
      for (const path of videoPaths) {
        if (succeededPaths.has(path)) continue
        const name = path.split('/').pop() || path
        const failure = result.failures.find(f => f.includes(name))
        const reason = failure
          ? localizeBackendMessage(failure.replace(/^Error in [^:]+:\s*/, ''), t)
          : t('status.unknownError')
        outcomes[path] = { status: 'failed', detail: reason }
        log(t('log.fileFailed', { file: name, reason }), 'error')
      }
      fileOutcomes.value = outcomes

      if (result.failures.length === 0) {
        currentStatus.value = t('status.translationComplete')
        log(currentStatus.value, 'success')
      } else if (result.completedFiles === 0) {
        currentStatus.value = t('status.translationFailed', {
          failure: localizeBackendMessage(result.failures[0], t)
        })
        log(currentStatus.value, 'error')
      } else {
        currentStatus.value = t('status.translationFinishedWithErrors', {
          completed: result.completedFiles,
          total: result.totalFiles,
          failure: localizeBackendMessage(result.failures[0], t)
        })
        log(currentStatus.value, 'warning')
      }
    } catch (e) {
      console.error('Translation error:', e)
      currentStatus.value = t('status.error', { error: e })
      log(currentStatus.value, 'error')
    } finally {
      unlistenProgress()
      unlistenBatchProgress()
      unlistenActiveFiles()
      isTranslating.value = false
      activeFileLines.value = null
      activeFilePhase.value = 'other'
      activeFilePaths.value = new Set()
    }
  }

  const retryFailures = async (paths: string[]) => {
    if (paths.length === 0 || isTranslating.value) return
    await startTranslation(paths)
  }

  return {
    isTranslating,
    translationProgress,
    currentStatus,
    estimatedTime,
    currentFileIndex,
    batchPaths,
    fileOutcomes,
    activeFilePath,
    activeFileName,
    activeFilePhase,
    activeFileLines,
    activeFilePaths,
    canStartTranslation,
    setProgress,
    resetProgress,
    clearOutcome,
    clearAllOutcomes,
    startTranslation,
    retryFailures
  }
}
