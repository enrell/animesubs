import {
  defaultInterfaceLanguage,
  isInterfaceLocale,
  type InterfaceLocale
} from '../i18n'

export interface Settings {
  provider: string
  apiEndpoint: string
  apiKey: string
  tmdbApiKey: string
  selectedModel: string | null
  interfaceLanguage: InterfaceLocale
  hasSelectedInterfaceLanguage: boolean
  sourceLanguage: string
  targetLanguage: string
  customSystemPrompt: string
  outputDirectory: string
  outputFormat: string
  ffmpegPath: string
  autoBackup: boolean
  keepOriginalTrack: boolean
  batchSize: number
  episodeConcurrency: number
  concurrency: number
  autoExtractContext: boolean
}

export const SETTINGS_STORAGE_KEY = 'animesubs-settings'
export const TRANSLATION_OPTIONS_STORAGE_KEY = 'animesubs-translation-options'

export const DEFAULT_BATCH_SIZE = 150
export const DEFAULT_EPISODE_CONCURRENCY = 2
export const DEFAULT_CONCURRENCY = 6

export const defaultSettings: Settings = {
  provider: 'openai',
  apiEndpoint: 'https://api.openai.com/v1',
  apiKey: '',
  tmdbApiKey: '',
  selectedModel: null,
  interfaceLanguage: defaultInterfaceLanguage,
  hasSelectedInterfaceLanguage: false,
  sourceLanguage: '',
  targetLanguage: 'en',
  customSystemPrompt: '',
  outputDirectory: '',
  outputFormat: '',
  ffmpegPath: '',
  autoBackup: true,
  keepOriginalTrack: true,
  batchSize: DEFAULT_BATCH_SIZE,
  episodeConcurrency: DEFAULT_EPISODE_CONCURRENCY,
  concurrency: DEFAULT_CONCURRENCY,
  autoExtractContext: true
}

export const settingsForStorage = (settings: Settings): Settings => ({
  ...settings,
  apiKey: '',
  tmdbApiKey: settings.tmdbApiKey || ''
})

export const normalizeSettings = (settings: Partial<Settings>): Settings => {
  const interfaceLanguage = isInterfaceLocale(settings.interfaceLanguage)
    ? settings.interfaceLanguage
    : defaultInterfaceLanguage

  const batchSize = typeof settings.batchSize === 'number' && settings.batchSize > 0
    ? settings.batchSize
    : defaultSettings.batchSize
  const episodeConcurrency = typeof settings.episodeConcurrency === 'number' && settings.episodeConcurrency > 0
    ? settings.episodeConcurrency
    : defaultSettings.episodeConcurrency
  const concurrency = typeof settings.concurrency === 'number' && settings.concurrency > 0
    ? settings.concurrency
    : defaultSettings.concurrency
  const autoExtractContext = typeof settings.autoExtractContext === 'boolean'
    ? settings.autoExtractContext
    : defaultSettings.autoExtractContext

  return {
    ...defaultSettings,
    ...settings,
    interfaceLanguage,
    hasSelectedInterfaceLanguage: Boolean(settings.hasSelectedInterfaceLanguage),
    batchSize,
    episodeConcurrency,
    concurrency,
    autoExtractContext
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
