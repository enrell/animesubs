import {
  defaultInterfaceLanguage,
  isInterfaceLocale,
  type InterfaceLocale
} from '../i18n'

export interface Settings {
  provider: string
  apiEndpoint: string
  apiKey: string
  selectedModel: string | null
  interfaceLanguage: InterfaceLocale
  hasSelectedInterfaceLanguage: boolean
  sourceLanguage: string
  targetLanguage: string
  translationStyle: string
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
  interfaceLanguage: defaultInterfaceLanguage,
  hasSelectedInterfaceLanguage: false,
  sourceLanguage: '',
  targetLanguage: 'en',
  translationStyle: 'natural',
  outputDirectory: '',
  outputFormat: '',
  ffmpegPath: '',
  autoBackup: true,
  keepOriginalTrack: true
}

export const settingsForStorage = (settings: Settings): Settings => ({
  ...settings,
  apiKey: ''
})

export const normalizeSettings = (settings: Partial<Settings>): Settings => {
  const interfaceLanguage = isInterfaceLocale(settings.interfaceLanguage)
    ? settings.interfaceLanguage
    : defaultInterfaceLanguage

  return {
    ...defaultSettings,
    ...settings,
    interfaceLanguage,
    hasSelectedInterfaceLanguage: Boolean(settings.hasSelectedInterfaceLanguage)
  }
}

export const sharedLanguageOptions = [
  { value: '' },
  { value: 'ja' },
  { value: 'en' },
  { value: 'zh-CN' },
  { value: 'zh-TW' },
  { value: 'ko' },
  { value: 'es' },
  { value: 'fr' },
  { value: 'de' },
  { value: 'fa' },
  { value: 'pt' },
  { value: 'ru' },
  { value: 'it' },
  { value: 'ar' },
  { value: 'th' },
  { value: 'vi' },
  { value: 'id' },
  { value: 'pl' },
  { value: 'tr' }
]

export const providerRequiresApiKey = (provider?: string | null): boolean => {
  return provider === 'openai'
    || provider === 'gemini'
    || provider === 'openrouter'
    || provider === 'nvidia'
    || provider === 'minimax'
}

export const hasUsableApiConfig = (settings?: Settings | null): boolean => {
  if (!settings?.selectedModel) return false
  if (providerRequiresApiKey(settings.provider)) {
    return Boolean(settings.apiKey)
  }
  return true
}
