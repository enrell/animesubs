import { computed, ref, watch } from 'vue'
import { darkTheme } from 'naive-ui'
import { themes, themeList, type ThemeName } from '../themes/themes'

const THEME_KEY = 'animesubs-theme-v2'

const isValidTheme = (value: string | null): value is ThemeName =>
  value !== null && value in themes

const getInitialTheme = (): ThemeName => {
  try {
    const saved = localStorage.getItem(THEME_KEY)
    if (isValidTheme(saved)) return saved
  } catch (e) {
    console.error('Failed to read theme preference:', e)
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'kitsune' : 'hanami'
}

export const useAppTheme = () => {
  const themeName = ref<ThemeName>(getInitialTheme())
  const definition = computed(() => themes[themeName.value])
  const isDark = computed(() => definition.value.mode === 'dark')
  const theme = computed(() => (isDark.value ? darkTheme : null))
  const themeOverrides = computed(() => definition.value.naive)

  const themeOptions = computed(() =>
    themeList.map(t => ({ label: t.labelKey, value: t.name, mode: t.mode }))
  )

  const applyTheme = () => {
    const root = document.documentElement
    const def = definition.value
    for (const [key, value] of Object.entries(def.tokens)) {
      root.style.setProperty(key, value)
    }
    root.style.setProperty('--theme-bg-image', 'none')
    root.style.setProperty('--theme-bg-opacity', '0')
    root.dataset.theme = def.name
    root.classList.toggle('theme-dark', isDark.value)
    root.classList.toggle('theme-light', !isDark.value)
  }

  const setTheme = (name: ThemeName) => {
    themeName.value = name
    try {
      localStorage.setItem(THEME_KEY, name)
    } catch (e) {
      console.error('Failed to save theme preference:', e)
    }
  }

  watch(themeName, applyTheme, { immediate: true })

  return {
    themeName,
    isDark,
    theme,
    themeOverrides,
    themeOptions,
    setTheme
  }
}
