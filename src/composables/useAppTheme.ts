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
      primaryColor: '#ffffff',
      primaryColorHover: '#e0e0e0',
      primaryColorPressed: '#bdbdbd',
      primaryColorSuppl: '#ffffff',
      borderRadius: '0px',
      ...(isDark.value ? {
        bodyColor: '#0a0a0a',
        cardColor: '#0a0a0a',
        modalColor: '#0a0a0a',
        popoverColor: '#141414',
        tableColor: '#0a0a0a',
        inputColor: '#0a0a0a',
        actionColor: '#141414',
        hoverColor: 'rgba(255, 255, 255, 0.06)',
        borderColor: '#262626',
        dividerColor: '#1a1a1a',
        textColor1: '#ffffff',
        textColor2: '#888888',
        textColor3: '#555555'
      } : {
        bodyColor: '#ffffff',
        cardColor: '#ffffff',
        modalColor: '#ffffff',
        popoverColor: '#ffffff',
        tableColor: '#ffffff',
        inputColor: '#ffffff',
        actionColor: '#f5f5f5',
        hoverColor: 'rgba(0, 0, 0, 0.04)',
        borderColor: '#000000',
        dividerColor: '#000000',
        textColor1: '#000000',
        textColor2: '#666666',
        textColor3: '#999999'
      })
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
