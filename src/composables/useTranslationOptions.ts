import { reactive, watch } from 'vue'
import { TRANSLATION_OPTIONS_STORAGE_KEY } from '../config/settings'

export interface TranslationOptions {
  subtitleTrack: string
  embedSubtitles: boolean
  useMkvmerge: boolean
  batchSize: number
  concurrency: number
  requestDelay: number
  customPrompt: string
}

export const useTranslationOptions = () => {
  const translationOptions = reactive<TranslationOptions>({
    subtitleTrack: '',
    embedSubtitles: false,
    useMkvmerge: true,
    batchSize: 100,
    concurrency: 1,
    requestDelay: 500,
    customPrompt: ''
  })

  const loadTranslationOptions = () => {
    try {
      const saved = localStorage.getItem(TRANSLATION_OPTIONS_STORAGE_KEY)
      if (saved) {
        Object.assign(translationOptions, JSON.parse(saved))
      }
    } catch (e) {
      console.error('Failed to load translation options:', e)
    }
  }

  const saveTranslationOptions = () => {
    try {
      localStorage.setItem(TRANSLATION_OPTIONS_STORAGE_KEY, JSON.stringify(translationOptions))
    } catch (e) {
      console.error('Failed to save translation options:', e)
    }
  }

  watch(translationOptions, saveTranslationOptions, { deep: true })

  return {
    translationOptions,
    loadTranslationOptions,
    saveTranslationOptions
  }
}
