import { ref, type Ref } from 'vue'
import {
  extractContextProfile,
  loadContextProfile,
  saveContextProfile
} from '../api/animesubs'
import { parseSubtitleFile, extractSubtitle } from '../api/animesubs'
import type {
  ContextProfile,
  LlmConfig,
  SubtitleData
} from '../types/domain'

const CONTEXT_STORAGE_KEY = 'animesubs-context-profile'

export interface UseContextProfileParams {
  cachedSettings: Ref<{
    provider: string
    apiEndpoint: string
    apiKey: string
    selectedModel: string | null
    sourceLanguage: string
    targetLanguage: string
  } | null>
  getSettings: () => {
    provider: string
    apiEndpoint: string
    apiKey: string
    selectedModel: string | null
    sourceLanguage: string
    targetLanguage: string
  } | null
}

export const useContextProfile = ({
  getSettings
}: UseContextProfileParams) => {
  const contextProfile = ref<ContextProfile | null>(null)
  const isExtracting = ref(false)
  const extractionStatus = ref('')
  const contextEnabled = ref(false)

  const loadStoredProfile = () => {
    try {
      const stored = localStorage.getItem(CONTEXT_STORAGE_KEY)
      if (stored) {
        contextProfile.value = JSON.parse(stored)
      }
    } catch (e) {
      console.error('Failed to load stored context profile:', e)
    }
  }

  const persistProfile = () => {
    try {
      if (contextProfile.value) {
        localStorage.setItem(CONTEXT_STORAGE_KEY, JSON.stringify(contextProfile.value))
      } else {
        localStorage.removeItem(CONTEXT_STORAGE_KEY)
      }
    } catch (e) {
      console.error('Failed to persist context profile:', e)
    }
  }

  const updateProfile = (profile: ContextProfile) => {
    profile.updated_at = new Date().toISOString()
    contextProfile.value = profile
    persistProfile()
  }

  const clearProfile = () => {
    contextProfile.value = null
    contextEnabled.value = false
    persistProfile()
  }

  const buildLlmConfig = (): LlmConfig | null => {
    const settings = getSettings()
    if (!settings?.selectedModel) return null
    return {
      provider: settings.provider,
      api_key: settings.apiKey,
      endpoint: settings.apiEndpoint,
      model: settings.selectedModel,
      system_prompt: ''
    }
  }

  const extractFromSubtitleData = async (
    filename: string,
    subtitleData: SubtitleData,
    customInstructions?: string
  ): Promise<ContextProfile | null> => {
    const config = buildLlmConfig()
    if (!config) {
      throw new Error('No model configured')
    }

    const settings = getSettings()
    isExtracting.value = true
    extractionStatus.value = 'Classifying anime title...'

    try {
      const settingsObj = settings as Record<string, any> | null
      const result = await extractContextProfile({
        filename,
        subtitleData,
        config,
        sourceLang: settings?.sourceLanguage || 'auto',
        targetLang: settings?.targetLanguage || 'en',
        customInstructions: customInstructions || null,
        tmdbApiKey: settingsObj?.tmdbApiKey || null
      })
      updateProfile(result.profile)
      extractionStatus.value = ''
      return result.profile
    } catch (e) {
      extractionStatus.value = `Error: ${e}`
      throw e
    } finally {
      isExtracting.value = false
    }
  }

  const extractFromVideo = async (
    videoPath: string,
    filename: string,
    trackIndex: number,
    customInstructions?: string
  ): Promise<ContextProfile | null> => {
    const settings = getSettings()
    const ffmpegPath = (settings as { ffmpegPath?: string } | null)?.ffmpegPath || null

    isExtracting.value = true
    extractionStatus.value = 'Extracting subtitles...'

    try {
      const extractResult = await extractSubtitle({
        videoPath,
        trackIndex,
        temporary: true,
        ffmpegPath
      })
      if (!extractResult.success || !extractResult.output_path) {
        throw new Error(extractResult.error || 'Failed to extract subtitle')
      }

      extractionStatus.value = 'Parsing subtitles...'
      const subtitleData = await parseSubtitleFile(extractResult.output_path)

      return extractFromSubtitleData(filename, subtitleData, customInstructions)
    } finally {
      isExtracting.value = false
    }
  }

  const saveProfileToFile = async (filePath: string): Promise<string> => {
    if (!contextProfile.value) throw new Error('No context profile to save')
    const result = await saveContextProfile(contextProfile.value, filePath)
    return result.data || filePath
  }

  const loadProfileFromFile = async (filePath: string) => {
    const profile = await loadContextProfile(filePath)
    updateProfile(profile)
  }

  const toggleContext = (enabled: boolean) => {
    contextEnabled.value = enabled
  }

  return {
    contextProfile,
    isExtracting,
    extractionStatus,
    contextEnabled,
    loadStoredProfile,
    updateProfile,
    clearProfile,
    extractFromSubtitleData,
    extractFromVideo,
    saveProfileToFile,
    loadProfileFromFile,
    toggleContext
  }
}
