import type { GlobalThemeOverrides } from 'naive-ui'
import kitsuneBg from '../assets/bg/kitsune.svg?url'
import matsuriBg from '../assets/bg/matsuri.svg?url'
import hanamiBg from '../assets/bg/hanami.svg?url'
import matchaBg from '../assets/bg/matcha.svg?url'

export type ThemeMode = 'dark' | 'light'

export type ThemeName = 'kitsune' | 'matsuri' | 'hanami' | 'matcha'

export interface ThemeDefinition {
  name: ThemeName
  mode: ThemeMode
  labelKey: string
  tokens: Record<string, string>
  background: string
  backgroundOpacity: string
  naive: GlobalThemeOverrides
}

const sharedRadii = {
  '--radius-sm': '6px',
  '--radius-md': '10px',
  '--radius-lg': '14px'
}

const buildNaiveOverrides = (t: {
  accent: string
  accentHover: string
  accentPressed: string
  accentContrast: string
  body: string
  surface: string
  elevated: string
  inset: string
  textPrimary: string
  textSecondary: string
  textMuted: string
  border: string
  divider: string
  hover: string
  success: string
  warning: string
  danger: string
}): GlobalThemeOverrides => ({
  common: {
    primaryColor: t.accent,
    primaryColorHover: t.accentHover,
    primaryColorPressed: t.accentPressed,
    primaryColorSuppl: t.accent,
    successColor: t.success,
    successColorHover: t.success,
    successColorPressed: t.success,
    successColorSuppl: t.success,
    warningColor: t.warning,
    warningColorHover: t.warning,
    warningColorPressed: t.warning,
    warningColorSuppl: t.warning,
    errorColor: t.danger,
    errorColorHover: t.danger,
    errorColorPressed: t.danger,
    errorColorSuppl: t.danger,
    bodyColor: t.body,
    cardColor: t.elevated,
    modalColor: t.elevated,
    popoverColor: t.elevated,
    tableColor: t.surface,
    inputColor: t.inset,
    actionColor: t.elevated,
    hoverColor: t.hover,
    borderColor: t.border,
    dividerColor: t.divider,
    textColorBase: t.textPrimary,
    textColor1: t.textPrimary,
    textColor2: t.textSecondary,
    textColor3: t.textMuted,
    placeholderColor: t.textMuted,
    borderRadius: '8px',
    borderRadiusSmall: '6px',
    fontSize: '14px'
  },
  Button: {
    textColorPrimary: t.accentContrast,
    textColorHoverPrimary: t.accentContrast,
    textColorPressedPrimary: t.accentContrast,
    textColorFocusPrimary: t.accentContrast
  },
  Progress: {
    fillColor: t.accent
  }
})

export const themes: Record<ThemeName, ThemeDefinition> = {
  kitsune: {
    name: 'kitsune',
    mode: 'dark',
    labelKey: 'themes.kitsune',
    background: kitsuneBg,
    backgroundOpacity: '0.55',
    tokens: {
      ...sharedRadii,
      '--bg-base': '#171310',
      '--bg-surface': 'rgba(37, 29, 23, 0.86)',
      '--bg-elevated': '#2b221b',
      '--bg-inset': 'rgba(16, 12, 10, 0.72)',
      '--text-primary': '#f3ece2',
      '--text-secondary': '#c6b8a7',
      '--text-muted': '#8f8071',
      '--border-subtle': 'rgba(243, 236, 226, 0.09)',
      '--border-strong': 'rgba(243, 236, 226, 0.18)',
      '--accent': '#e4573d',
      '--accent-hover': '#ee6c53',
      '--accent-pressed': '#c94a32',
      '--accent-contrast': '#fff5ee',
      '--accent-soft': 'rgba(228, 87, 61, 0.14)',
      '--success': '#63a96b',
      '--success-soft': 'rgba(99, 169, 107, 0.14)',
      '--warning': '#d9a13b',
      '--warning-soft': 'rgba(217, 161, 59, 0.14)',
      '--danger': '#e05252',
      '--danger-soft': 'rgba(224, 82, 82, 0.14)',
      '--focus-ring': 'rgba(228, 87, 61, 0.55)',
      '--hover-overlay': 'rgba(243, 236, 226, 0.05)',
      '--shadow-pop': '0 14px 40px rgba(0, 0, 0, 0.5)',
      '--font-sans': 'Inter, "Segoe UI", system-ui, -apple-system, sans-serif',
      '--font-mono': 'ui-monospace, "SF Mono", "Cascadia Code", Consolas, monospace'
    },
    naive: buildNaiveOverrides({
      accent: '#e4573d',
      accentHover: '#ee6c53',
      accentPressed: '#c94a32',
      accentContrast: '#fff5ee',
      body: '#171310',
      surface: '#251d17',
      elevated: '#2b221b',
      inset: '#141010',
      textPrimary: '#f3ece2',
      textSecondary: '#c6b8a7',
      textMuted: '#8f8071',
      border: 'rgba(243, 236, 226, 0.18)',
      divider: 'rgba(243, 236, 226, 0.09)',
      hover: 'rgba(243, 236, 226, 0.06)',
      success: '#63a96b',
      warning: '#d9a13b',
      danger: '#e05252'
    })
  },
  matsuri: {
    name: 'matsuri',
    mode: 'dark',
    labelKey: 'themes.matsuri',
    background: matsuriBg,
    backgroundOpacity: '0.55',
    tokens: {
      ...sharedRadii,
      '--bg-base': '#191410',
      '--bg-surface': 'rgba(40, 30, 20, 0.86)',
      '--bg-elevated': '#30251a',
      '--bg-inset': 'rgba(18, 14, 10, 0.72)',
      '--text-primary': '#f6efe0',
      '--text-secondary': '#cebd9f',
      '--text-muted': '#97836a',
      '--border-subtle': 'rgba(246, 239, 224, 0.09)',
      '--border-strong': 'rgba(246, 239, 224, 0.18)',
      '--accent': '#e39a2d',
      '--accent-hover': '#f0ab45',
      '--accent-pressed': '#c98622',
      '--accent-contrast': '#2a1b06',
      '--accent-soft': 'rgba(227, 154, 45, 0.16)',
      '--success': '#6fae5f',
      '--success-soft': 'rgba(111, 174, 95, 0.14)',
      '--warning': '#e8c04a',
      '--warning-soft': 'rgba(232, 192, 74, 0.14)',
      '--danger': '#e05252',
      '--danger-soft': 'rgba(224, 82, 82, 0.14)',
      '--focus-ring': 'rgba(227, 154, 45, 0.55)',
      '--hover-overlay': 'rgba(246, 239, 224, 0.05)',
      '--shadow-pop': '0 14px 40px rgba(0, 0, 0, 0.5)',
      '--font-sans': 'Inter, "Segoe UI", system-ui, -apple-system, sans-serif',
      '--font-mono': 'ui-monospace, "SF Mono", "Cascadia Code", Consolas, monospace'
    },
    naive: buildNaiveOverrides({
      accent: '#e39a2d',
      accentHover: '#f0ab45',
      accentPressed: '#c98622',
      accentContrast: '#2a1b06',
      body: '#191410',
      surface: '#281e14',
      elevated: '#30251a',
      inset: '#16110c',
      textPrimary: '#f6efe0',
      textSecondary: '#cebd9f',
      textMuted: '#97836a',
      border: 'rgba(246, 239, 224, 0.18)',
      divider: 'rgba(246, 239, 224, 0.09)',
      hover: 'rgba(246, 239, 224, 0.06)',
      success: '#6fae5f',
      warning: '#e8c04a',
      danger: '#e05252'
    })
  },
  hanami: {
    name: 'hanami',
    mode: 'light',
    labelKey: 'themes.hanami',
    background: hanamiBg,
    backgroundOpacity: '0.7',
    tokens: {
      ...sharedRadii,
      '--bg-base': '#faf4ee',
      '--bg-surface': 'rgba(255, 252, 248, 0.9)',
      '--bg-elevated': '#ffffff',
      '--bg-inset': 'rgba(240, 231, 222, 0.6)',
      '--text-primary': '#31251f',
      '--text-secondary': '#5d4f47',
      '--text-muted': '#9a8b80',
      '--border-subtle': 'rgba(49, 37, 31, 0.09)',
      '--border-strong': 'rgba(49, 37, 31, 0.16)',
      '--accent': '#d6406b',
      '--accent-hover': '#e2557d',
      '--accent-pressed': '#b8355b',
      '--accent-contrast': '#fff7f9',
      '--accent-soft': 'rgba(214, 64, 107, 0.1)',
      '--success': '#3d8a4f',
      '--success-soft': 'rgba(61, 138, 79, 0.12)',
      '--warning': '#a8791a',
      '--warning-soft': 'rgba(168, 121, 26, 0.12)',
      '--danger': '#c73e3e',
      '--danger-soft': 'rgba(199, 62, 62, 0.1)',
      '--focus-ring': 'rgba(214, 64, 107, 0.4)',
      '--hover-overlay': 'rgba(49, 37, 31, 0.04)',
      '--shadow-pop': '0 14px 40px rgba(73, 45, 35, 0.18)',
      '--font-sans': 'Inter, "Segoe UI", system-ui, -apple-system, sans-serif',
      '--font-mono': 'ui-monospace, "SF Mono", "Cascadia Code", Consolas, monospace'
    },
    naive: buildNaiveOverrides({
      accent: '#d6406b',
      accentHover: '#e2557d',
      accentPressed: '#b8355b',
      accentContrast: '#fff7f9',
      body: '#faf4ee',
      surface: '#fffcf8',
      elevated: '#ffffff',
      inset: '#f4ece4',
      textPrimary: '#31251f',
      textSecondary: '#5d4f47',
      textMuted: '#9a8b80',
      border: 'rgba(49, 37, 31, 0.16)',
      divider: 'rgba(49, 37, 31, 0.09)',
      hover: 'rgba(49, 37, 31, 0.05)',
      success: '#3d8a4f',
      warning: '#a8791a',
      danger: '#c73e3e'
    })
  },
  matcha: {
    name: 'matcha',
    mode: 'light',
    labelKey: 'themes.matcha',
    background: matchaBg,
    backgroundOpacity: '0.65',
    tokens: {
      ...sharedRadii,
      '--bg-base': '#f3f4eb',
      '--bg-surface': 'rgba(251, 252, 245, 0.9)',
      '--bg-elevated': '#ffffff',
      '--bg-inset': 'rgba(232, 236, 218, 0.6)',
      '--text-primary': '#27311f',
      '--text-secondary': '#4d5942',
      '--text-muted': '#8b937b',
      '--border-subtle': 'rgba(39, 49, 31, 0.09)',
      '--border-strong': 'rgba(39, 49, 31, 0.17)',
      '--accent': '#5a7a2e',
      '--accent-hover': '#688c38',
      '--accent-pressed': '#4c6826',
      '--accent-contrast': '#f7faf0',
      '--accent-soft': 'rgba(90, 122, 46, 0.12)',
      '--success': '#3d8a4f',
      '--success-soft': 'rgba(61, 138, 79, 0.12)',
      '--warning': '#a8791a',
      '--warning-soft': 'rgba(168, 121, 26, 0.12)',
      '--danger': '#c73e3e',
      '--danger-soft': 'rgba(199, 62, 62, 0.1)',
      '--focus-ring': 'rgba(90, 122, 46, 0.4)',
      '--hover-overlay': 'rgba(39, 49, 31, 0.04)',
      '--shadow-pop': '0 14px 40px rgba(52, 66, 38, 0.18)',
      '--font-sans': 'Inter, "Segoe UI", system-ui, -apple-system, sans-serif',
      '--font-mono': 'ui-monospace, "SF Mono", "Cascadia Code", Consolas, monospace'
    },
    naive: buildNaiveOverrides({
      accent: '#5a7a2e',
      accentHover: '#688c38',
      accentPressed: '#4c6826',
      accentContrast: '#f7faf0',
      body: '#f3f4eb',
      surface: '#fbfcf5',
      elevated: '#ffffff',
      inset: '#ecefe0',
      textPrimary: '#27311f',
      textSecondary: '#4d5942',
      textMuted: '#8b937b',
      border: 'rgba(39, 49, 31, 0.17)',
      divider: 'rgba(39, 49, 31, 0.09)',
      hover: 'rgba(39, 49, 31, 0.05)',
      success: '#3d8a4f',
      warning: '#a8791a',
      danger: '#c73e3e'
    })
  }
}

export const themeList = Object.values(themes)
