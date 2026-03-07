export interface Settings {
  provider: string
  apiEndpoint: string
  apiKey: string
  selectedModel: string | null
  sourceLanguage: string
  targetLanguage: string
  translationStyle: string
  contextLines: number
  outputDirectory: string
  outputFormat: string
  ffmpegPath: string
  autoBackup: boolean
  keepOriginalTrack: boolean
}

export const SETTINGS_STORAGE_KEY = 'animesubs-settings'
export const TRANSLATION_OPTIONS_STORAGE_KEY = 'animesubs-translation-options'

export const defaultSettings: Settings = {
  provider: 'openai',
  apiEndpoint: 'https://api.openai.com/v1',
  apiKey: '',
  selectedModel: null,
  sourceLanguage: '',
  targetLanguage: 'en',
  translationStyle: 'natural',
  contextLines: 2,
  outputDirectory: '',
  outputFormat: 'srt',
  ffmpegPath: '',
  autoBackup: true,
  keepOriginalTrack: true
}

export const sharedLanguageOptions = [
  { label: 'Auto-detect', value: '' },
  { label: 'Japanese', value: 'ja' },
  { label: 'English', value: 'en' },
  { label: 'Chinese (Simplified)', value: 'zh-CN' },
  { label: 'Chinese (Traditional)', value: 'zh-TW' },
  { label: 'Korean', value: 'ko' },
  { label: 'Spanish', value: 'es' },
  { label: 'French', value: 'fr' },
  { label: 'German', value: 'de' },
  { label: 'Portuguese', value: 'pt' },
  { label: 'Russian', value: 'ru' },
  { label: 'Italian', value: 'it' },
  { label: 'Arabic', value: 'ar' },
  { label: 'Thai', value: 'th' },
  { label: 'Vietnamese', value: 'vi' },
  { label: 'Indonesian', value: 'id' },
  { label: 'Polish', value: 'pl' },
  { label: 'Turkish', value: 'tr' }
]

export const providerRequiresApiKey = (provider?: string | null): boolean => {
  return provider === 'openai' || provider === 'gemini' || provider === 'openrouter'
}

export const hasUsableApiConfig = (settings?: Settings | null): boolean => {
  if (!settings?.selectedModel) return false
  if (providerRequiresApiKey(settings.provider)) {
    return Boolean(settings.apiKey)
  }
  return true
}