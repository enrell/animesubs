import { reactive, watch } from 'vue'
import { TRANSLATION_OPTIONS_STORAGE_KEY } from '../config/settings'

export interface TranslationOptions {
  subtitleTrack: string
  embedSubtitles: boolean
  useMkvmerge: boolean
  customPrompt: string
}

export const useTranslationOptions = () => {
  const translationOptions = reactive<TranslationOptions>({
    subtitleTrack: '',
    embedSubtitles: false,
    useMkvmerge: true,
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

  let saveTimer: ReturnType<typeof setTimeout> | null = null

  const scheduleSave = () => {
    if (saveTimer !== null) clearTimeout(saveTimer)
    saveTimer = setTimeout(() => {
      saveTimer = null
      try {
        localStorage.setItem(
          TRANSLATION_OPTIONS_STORAGE_KEY,
          JSON.stringify(translationOptions)
        )
      } catch (e) {
        console.error('Failed to save translation options:', e)
      }
    }, 300)
  }

  watch(translationOptions, scheduleSave, { deep: true })

  return {
    translationOptions,
    loadTranslationOptions
  }
}
