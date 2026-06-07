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
      primaryColor: '#c8bd98',
      primaryColorHover: '#e0d4a8',
      primaryColorPressed: '#a89d7d',
      primaryColorSuppl: '#c99a86',
      borderRadius: '0px',
      ...(isDark.value ? {
        bodyColor: '#030303',
        cardColor: '#121013',
        modalColor: '#121013',
        popoverColor: '#1c151d',
        tableColor: '#121013',
        inputColor: '#050505',
        actionColor: '#1c151d',
        hoverColor: 'rgba(201, 154, 134, 0.12)',
        borderColor: 'rgba(200, 189, 152, 0.22)',
        dividerColor: 'rgba(200, 189, 152, 0.22)',
        textColor1: '#e0d4a8',
        textColor2: '#c8bd98',
        textColor3: '#8d8064'
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
