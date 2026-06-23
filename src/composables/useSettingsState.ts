import { computed, ref, watch, type Ref } from 'vue'
import { checkFfmpeg, loadApiKey } from '../api/animesubs'
import {
  defaultSettings,
  normalizeSettings,
  SETTINGS_STORAGE_KEY,
  settingsForStorage,
  type Settings
} from '../config/settings'
import { setInterfaceLocale } from '../i18n'
import type { OperationResult } from '../types/domain'

export interface SettingsModalExpose {
  settings: Settings
  getSystemPrompt?: () => string
}

export const useSettingsState = (
  showSettings: Ref<boolean>,
  settingsRef: Ref<SettingsModalExpose | null>
) => {
  const cachedSettings = ref<Settings | null>(null)
  const ffmpegStatus = ref<OperationResult | null>(null)

  const loadCachedSettings = async () => {
    try {
      const saved = localStorage.getItem(SETTINGS_STORAGE_KEY)
      const loaded = saved
        ? normalizeSettings({ ...JSON.parse(saved), apiKey: '' })
        : { ...defaultSettings }
      setInterfaceLocale(loaded.interfaceLanguage)
      const apiKey = await loadApiKey(loaded.provider)
      cachedSettings.value = {
        ...loaded,
        apiKey: apiKey.data || ''
      }
    } catch (e) {
      console.error('Failed to load settings:', e)
      cachedSettings.value = { ...defaultSettings }
    }
  }

  watch(showSettings, (isOpen, wasOpen) => {
    if (wasOpen && !isOpen) {
      void loadCachedSettings()
    }
  })

  const getSettings = (): Settings | null => {
    if (settingsRef.value?.settings) {
      return settingsRef.value.settings
    }
    return cachedSettings.value
  }

  const updateSettings = (patch: Partial<Settings>) => {
    const nextSettings = {
      ...(cachedSettings.value ?? defaultSettings),
      ...patch
    }

    cachedSettings.value = nextSettings
    setInterfaceLocale(nextSettings.interfaceLanguage)

    if (settingsRef.value?.settings) {
      Object.assign(settingsRef.value.settings, patch)
    }

    try {
      localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settingsForStorage(nextSettings)))
    } catch (e) {
      console.error('Failed to persist settings:', e)
    }
  }

  const targetLanguageModel = computed({
    get: () => getSettings()?.targetLanguage || defaultSettings.targetLanguage,
    set: (value: string) => updateSettings({ targetLanguage: value })
  })

  const checkFFmpeg = async () => {
    try {
      const settings = getSettings()
      ffmpegStatus.value = await checkFfmpeg(settings?.ffmpegPath || null)
    } catch (e) {
      ffmpegStatus.value = {
        success: false,
        message: `Error: ${e}`,
        data: null
      }
    }
  }

  return {
    cachedSettings,
    ffmpegStatus,
    loadCachedSettings,
    getSettings,
    updateSettings,
    targetLanguageModel,
    checkFFmpeg
  }
}
