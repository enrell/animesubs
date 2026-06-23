import { computed, ref } from 'vue'
import { darkTheme, type GlobalThemeOverrides } from 'naive-ui'

const THEME_KEY = 'animesubs-theme'

const getInitialTheme = () => {
  try {
    const saved = localStorage.getItem(THEME_KEY)
    if (saved === 'dark' || saved === 'light') {
      return saved === 'dark'
    }
  } catch (e) {
    console.error('Failed to read theme preference:', e)
  }
  return window.matchMedia('(prefers-color-scheme: dark)').matches
}

export const useAppTheme = () => {
  const isDark = ref(getInitialTheme())
  const theme = computed(() => isDark.value ? darkTheme : null)

  const themeOverrides = computed<GlobalThemeOverrides>(() => ({
    common: {
      primaryColor: '#7ce8a0',
      primaryColorHover: '#8fffb8',
      primaryColorPressed: '#5ab878',
      primaryColorSuppl: '#8088cc',
      borderRadius: '0px',
      ...(isDark.value ? {
        bodyColor: '#020205',
        cardColor: '#0a0a18',
        modalColor: '#0a0a18',
        popoverColor: '#111128',
        tableColor: '#0a0a18',
        inputColor: '#020208',
        actionColor: '#111128',
        hoverColor: 'rgba(124, 232, 160, 0.10)',
        borderColor: 'rgba(124, 232, 160, 0.16)',
        dividerColor: 'rgba(124, 232, 160, 0.16)',
        textColor1: '#8fffb8',
        textColor2: '#7ce8a0',
        textColor3: '#4a7a5c'
      } : {})
    }
  }))

  const toggleTheme = () => {
    isDark.value = !isDark.value
    try {
      localStorage.setItem(THEME_KEY, isDark.value ? 'dark' : 'light')
    } catch (e) {
      console.error('Failed to save theme preference:', e)
    }
  }

  return {
    isDark,
    theme,
    themeOverrides,
    toggleTheme
  }
}
