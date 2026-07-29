<template>
  <n-config-provider :theme="theme" :theme-overrides="themeOverrides">
    <n-message-provider>
      <n-notification-provider>
        <n-dialog-provider>
          <div class="app-shell" :class="{ 'is-dragging': isDragging }">
            <header class="app-header">
              <div class="header-brand">
                <div class="brand-icon">
                  <n-icon size="20"><film-outline /></n-icon>
                </div>
                <div class="brand-text">
                  <p class="brand-eyebrow">{{ t('app.eyebrow') }}</p>
                  <h1>{{ t('app.title') }}</h1>
                </div>
              </div>

              <div class="status-strip" :aria-label="t('app.runtimeStatus')">
                <span class="status-badge">{{ t('app.provider', { provider: providerLabel }) }}</span>
                <span class="status-badge truncate">{{ t('app.model', { model: modelLabel }) }}</span>
                <span class="status-badge" :class="ffmpegStatusClass">
                  {{ t('app.ffmpeg', { status: ffmpegStatusLabel }) }}
                </span>
              </div>

              <div class="header-actions">
                <n-button
                  quaternary
                  circle
                  class="icon-button"
                  :title="isDark ? t('app.lightMode') : t('app.darkMode')"
                  @click="toggleTheme"
                >
                  <template #icon>
                    <n-icon size="18">
                      <sunny-outline v-if="isDark" />
                      <moon-outline v-else />
                    </n-icon>
                  </template>
                </n-button>
                <div class="language-menu-wrap">
                  <n-button
                    quaternary
                    circle
                    class="icon-button"
                    :title="t('app.language')"
                    @click="showLanguageMenu = !showLanguageMenu"
                  >
                    <template #icon>
                      <n-icon size="18"><language-outline /></n-icon>
                    </template>
                  </n-button>
                  <div v-if="showLanguageMenu" class="language-menu">
                    <button
                      v-for="option in interfaceLanguageSelectOptions"
                      :key="option.value"
                      type="button"
                      class="language-menu-item"
                      :class="{ active: option.value === cachedSettings?.interfaceLanguage }"
                      @click="changeInterfaceLanguage(option.value)"
                    >
                      {{ option.label }}
                    </button>
                  </div>
                </div>
                <n-button
                  quaternary
                  circle
                  class="icon-button"
                  :title="t('app.settings')"
                  @click="showSettings = true"
                >
                  <template #icon>
                    <n-icon size="18"><settings-outline /></n-icon>
                  </template>
                </n-button>
              </div>
            </header>

            <main class="app-main">
              <section
                class="drop-zone"
                :class="{ 'drop-zone-active': isDragging }"
                @dragover.prevent="setDragging(true)"
                @dragleave.prevent="setDragging(false)"
                @drop.prevent="handleDrop"
              >
                <div class="drop-zone-content">
                  <div class="drop-zone-icon">
                    <n-icon size="48"><cloud-upload-outline /></n-icon>
                  </div>
                  <h2>{{ t('app.heroTitle') }}</h2>
                  <p>{{ t('app.heroDescription') }}</p>
                  <div class="drop-zone-actions">
                    <n-button type="primary" size="large" @click="selectFiles" :loading="loadingFiles">
                      <template #icon><n-icon><document-outline /></n-icon></template>
                      {{ t('app.selectFiles') }}
                    </n-button>
                    <n-button size="large" @click="selectFolder" :loading="loadingFiles">
                      <template #icon><n-icon><folder-open-outline /></n-icon></template>
                      {{ t('app.scanFolder') }}
                    </n-button>
                  </div>
                </div>
              </section>

              <n-alert
                v-if="ffmpegStatus && !ffmpegStatus.success"
                type="warning"
                :title="t('app.ffmpegMissingTitle')"
                closable
                class="ffmpeg-alert"
              >
                {{ t('app.ffmpegMissingDescription') }}
              </n-alert>

              <section v-if="selectedFiles.length > 0" class="workspace">
                <div class="queue-panel app-card">
                  <div class="panel-header">
                    <div>
                      <p class="panel-label">{{ t('app.mediaQueue') }}</p>
                      <h3 class="panel-title">{{ t('app.packetsAttached', { count: selectedFiles.length }) }}</h3>
                    </div>
                    <n-button text type="error" @click="clearFiles">
                      <template #icon><n-icon><trash-outline /></n-icon></template>
                      {{ t('app.clear') }}
                    </n-button>
                  </div>

                  <div class="queue-stats">
                    <div class="stat-item">
                      <strong>{{ readyFileCount }}</strong>
                      <span>{{ t('app.ready') }}</span>
                    </div>
                    <div class="stat-item">
                      <strong>{{ totalSubtitleTracks }}</strong>
                      <span>{{ t('app.tracks') }}</span>
                    </div>
                    <div class="stat-item">
                      <strong>{{ totalBackups }}</strong>
                      <span>{{ t('app.backups') }}</span>
                    </div>
                  </div>

                  <n-scrollbar class="queue-scroll">
                    <div class="file-list">
                      <article v-for="(file, index) in selectedFiles" :key="file.path" class="file-card">
                        <div class="file-header">
                          <div class="file-title-row">
                            <n-icon size="20" class="file-icon"><videocam-outline /></n-icon>
                            <div class="file-title-wrap">
                              <h4>{{ file.name }}</h4>
                              <p class="file-path">{{ file.path }}</p>
                            </div>
                          </div>
                          <div class="file-actions">
                            <n-spin v-if="file.loading" size="small" />
                            <n-tag v-if="file.videoInfo" size="small" round :bordered="false" type="info">
                              {{ t('app.subs', { count: file.videoInfo.subtitle_tracks.length }) }}
                            </n-tag>
                            <n-button text type="error" @click="removeFile(index)">
                              <template #icon><n-icon><close-outline /></n-icon></template>
                            </n-button>
                          </div>
                        </div>

                        <div v-if="file.videoInfo && file.videoInfo.subtitle_tracks.length > 0" class="subtitle-tracks">
                          <div v-for="track in file.videoInfo.subtitle_tracks" :key="track.index" class="track-row">
                            <div class="track-meta">
                              <span class="track-lang">{{ track.language || 'und' }}</span>
                              <span class="track-title">{{ track.title || t('track.title', { index: track.index }) }}</span>
                              <span class="track-codec">{{ track.codec }}</span>
                              <n-tag v-if="track.default" size="tiny" round :bordered="false" type="success">{{ t('app.default') }}</n-tag>
                              <n-tag v-if="track.forced" size="tiny" round :bordered="false" type="warning">{{ t('app.forced') }}</n-tag>
                            </div>
                            <div class="track-actions">
                              <n-button
                                size="small"
                                quaternary
                                :title="t('app.extractSubtitle')"
                                @click="extractSubtitle(file, track.index)"
                                :loading="extractingSubtitle === file.path"
                              >
                                <template #icon><n-icon><download-outline /></n-icon></template>
                              </n-button>
                              <n-button
                                size="small"
                                quaternary
                                type="success"
                                :title="t('app.backupSubtitle')"
                                @click="backupSubtitle(file, track.index)"
                                :loading="backingUp === file.path"
                              >
                                <template #icon><n-icon><shield-checkmark-outline /></n-icon></template>
                              </n-button>
                            </div>
                          </div>
                        </div>

                        <div v-else-if="file.videoInfo && file.videoInfo.subtitle_tracks.length === 0" class="no-subs">
                          <n-icon size="16"><information-circle-outline /></n-icon>
                          <span>{{ t('app.noSubtitleTracks') }}</span>
                        </div>

                        <div v-if="file.error" class="file-error">
                          <n-text type="error" depth="3">{{ file.error }}</n-text>
                        </div>

                        <div v-if="file.backups.length > 0" class="backups-section">
                          <n-divider>{{ t('app.backupDivider') }}</n-divider>
                          <div v-for="backup in file.backups" :key="backup.backup_path" class="backup-item">
                            <span class="backup-meta">
                              {{ t('app.backupMeta', {
                                track: backup.track_index,
                                format: backup.format,
                                date: backup.created_at
                              }) }}
                            </span>
                            <div class="backup-actions">
                              <n-popconfirm @positive-click="restoreBackup(file, backup)">
                                <template #trigger>
                                  <n-button size="tiny" quaternary type="warning">
                                    <template #icon><n-icon><arrow-undo-outline /></n-icon></template>
                                  </n-button>
                                </template>
                                {{ t('app.restoreBackupConfirm') }}
                              </n-popconfirm>
                              <n-popconfirm @positive-click="deleteBackup(file, backup)">
                                <template #trigger>
                                  <n-button size="tiny" quaternary type="error">
                                    <template #icon><n-icon><trash-outline /></n-icon></template>
                                  </n-button>
                                </template>
                                {{ t('app.deleteBackupConfirm') }}
                              </n-popconfirm>
                            </div>
                          </div>
                        </div>
                      </article>
                    </div>
                  </n-scrollbar>
                </div>

                <aside class="translation-panel app-card">
                  <div class="panel-header">
                    <div>
                      <p class="panel-label">{{ t('app.translationProtocol') }}</p>
                      <h3 class="panel-title">{{ targetLanguageLabel }}</h3>
                    </div>
                    <span class="status-indicator" :class="{ online: canStartTranslation }"></span>
                  </div>

                  <div class="translation-form">
                    <n-form-item :label="t('app.targetLanguage')">
                      <n-input v-model:value="targetLanguageModel" :placeholder="t('app.targetLanguagePlaceholder')" />
                    </n-form-item>
                    <n-form-item :label="t('app.subtitleTrack')">
                      <n-select v-model:value="translationOptions.subtitleTrack" :options="subtitleTrackOptions" :placeholder="t('app.autoDetectFirstAvailable')" />
                    </n-form-item>

                    <div class="switch-stack">
                      <n-checkbox v-model:checked="translationOptions.embedSubtitles">
                        <span class="checkbox-label-content">
                          <n-icon><layers-outline /></n-icon>
                          {{ t('app.embedTranslatedSubtitles') }}
                        </span>
                      </n-checkbox>
                      <n-checkbox v-model:checked="translationOptions.useMkvmerge" :disabled="!translationOptions.embedSubtitles">
                        <span class="checkbox-label-content">
                          <n-icon><layers-outline /></n-icon>
                          {{ t('app.routeThroughMkvmerge') }}
                        </span>
                      </n-checkbox>
                    </div>

                    <n-collapse>
                      <n-collapse-item :title="t('app.advancedSignalControls')" name="advanced">
                        <n-form-item :label="t('app.customPrompt')">
                          <n-input v-model:value="translationOptions.customPrompt" type="textarea" :placeholder="t('app.customPromptPlaceholder')" :rows="4" />
                        </n-form-item>

                        <ContextExtractionPanel
                          :context-profile="contextProfile"
                          :is-extracting="isExtractingContext"
                          :extraction-status="extractionStatus"
                          :context-enabled="contextEnabled"
                          :can-extract="canExtractContext"
                          @toggle="toggleContext"
                          @clear="onClearContext"
                          @update="onUpdateContext"
                          @extract="onExtractContext"
                          @save-file="onSaveContextToFile"
                          @load-file="onLoadContextFromFile"
                        />
                      </n-collapse-item>
                    </n-collapse>
                  </div>

                  <div class="execute-block">
                    <n-button type="primary" size="large" block :loading="isTranslating" :disabled="!canStartTranslation" @click="startTranslation">
                      <template #icon><n-icon><play-outline /></n-icon></template>
                      {{ isTranslating ? t('app.translatingSignal') : t('app.initiateTranslation') }}
                    </n-button>
                    <p v-if="!canStartTranslation" class="disabled-hint">
                      {{ t('app.disabledHint') }}
                    </p>
                  </div>

                  <div v-if="isTranslating || translationProgress > 0" class="progress-block">
                    <div class="progress-header">
                      <span>{{ t('app.sync', { progress: Math.round(translationProgress) }) }}</span>
                      <span class="progress-time">{{ estimatedTime }}</span>
                    </div>
                    <n-progress type="line" :percentage="translationProgress" :status="translationProgress === 100 ? 'success' : 'default'" :show-indicator="false" />
                    <p class="progress-status">{{ currentStatus || t('app.awaitingPacketResponse') }}</p>
                  </div>
                </aside>
              </section>
            </main>
          </div>

          <SettingsModal v-model:show="showSettings" ref="settingsRef" />

          <div v-if="showLanguageSetup" class="setup-overlay" role="dialog" aria-modal="true">
            <section class="setup-card" :aria-label="t('setup.title')">
              <header class="setup-header">
                <h2>{{ t('setup.title') }}</h2>
              </header>
              <div class="setup-body">
                <div>
                  <p class="panel-label">{{ t('setup.eyebrow') }}</p>
                  <p class="setup-description">{{ t('setup.description') }}</p>
                </div>
                <div class="language-choice-grid">
                  <button
                    v-for="option in interfaceLanguageSelectOptions"
                    :key="option.value"
                    type="button"
                    class="language-choice"
                    :class="{ active: option.value === setupLanguage }"
                    @click="setupLanguage = option.value"
                  >
                    {{ option.label }}
                  </button>
                </div>
                <button type="button" class="setup-continue" @click="completeLanguageSetup">
                  <n-icon><language-outline /></n-icon>
                  {{ t('setup.continue') }}
                </button>
              </div>
            </section>
          </div>
        </n-dialog-provider>
      </n-notification-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  NConfigProvider,
  NMessageProvider,
  NNotificationProvider,
  NDialogProvider,
  NButton,
  NIcon,
  NScrollbar,
  NFormItem,
  NSelect,
  NCheckbox,
  NCollapse,
  NCollapseItem,
  NInput,
  NProgress,
  NSpin,
  NTag,
  NPopconfirm,
  NAlert,
  NDivider,
  NText
} from 'naive-ui'
import {
  SettingsOutline,
  SunnyOutline,
  MoonOutline,
  LanguageOutline,
  DocumentOutline,
  FolderOpenOutline,
  TrashOutline,
  VideocamOutline,
  CloseOutline,
  ShieldCheckmarkOutline,
  LayersOutline,
  PlayOutline,
  DownloadOutline,
  ArrowUndoOutline,
  InformationCircleOutline,
  FilmOutline,
  CloudUploadOutline
} from '@vicons/ionicons5'
import { getCurrentWindow, type DragDropEvent } from '@tauri-apps/api/window'
import { scanFolderForVideos } from './api/animesubs'
import { sharedLanguageOptions } from './config/settings'
import {
  defaultInterfaceLanguage,
  interfaceLanguageOptions,
  setInterfaceLocale,
  translationLanguageKey,
  type InterfaceLocale
} from './i18n'
import { useAppTheme } from './composables/useAppTheme'
import { useSettingsState, type SettingsModalExpose } from './composables/useSettingsState'
import { useTranslationOptions } from './composables/useTranslationOptions'
import { useVideoFiles } from './composables/useVideoFiles'
import { useTranslationJob } from './composables/useTranslationJob'
import { useContextProfile } from './composables/useContextProfile'
import type { ContextProfile } from './types/domain'

const SettingsModal = defineAsyncComponent(() => import('./components/SettingsModal.vue'))
const ContextExtractionPanel = defineAsyncComponent(() => import('./components/ContextExtractionPanel.vue'))

const { t } = useI18n()
const { isDark, theme, themeOverrides, toggleTheme } = useAppTheme()

const showSettings = ref(false)
const showLanguageSetup = ref(false)
const showLanguageMenu = ref(false)
const setupLanguage = ref<InterfaceLocale>(defaultInterfaceLanguage)
const settingsRef = ref<SettingsModalExpose | null>(null)

const {
  cachedSettings,
  ffmpegStatus,
  loadCachedSettings,
  getSettings,
  updateSettings,
  targetLanguageModel,
  checkFFmpeg
} = useSettingsState(showSettings, settingsRef)

const {
  translationOptions,
  loadTranslationOptions
} = useTranslationOptions()

const languageOptions = computed(() => {
  return sharedLanguageOptions
    .filter(option => option.value)
    .map(option => ({
      ...option,
      label: t(translationLanguageKey(option.value))
    }))
})

const interfaceLanguageSelectOptions = computed(() => {
  return interfaceLanguageOptions.map(option => ({
    label: t(option.labelKey),
    value: option.value
  }))
})

const {
  selectedFiles,
  subtitleTrackOptions,
  isDragging,
  loadingFiles,
  extractingSubtitle,
  backingUp,
  addFiles,
  selectFiles,
  selectFolder,
  handleDrop,
  removeFile,
  clearFiles: clearSelectedFiles,
  extractSubtitle,
  backupSubtitle,
  restoreBackup,
  deleteBackup
} = useVideoFiles(getSettings, t)

const providerLabel = computed(() => cachedSettings.value?.provider || t('app.unconfigured'))
const modelLabel = computed(() => cachedSettings.value?.selectedModel || t('app.noModel'))
const targetLanguageLabel = computed(() => {
  const target = cachedSettings.value?.targetLanguage || targetLanguageModel.value
  return languageOptions.value.find(option => option.value === target)?.label
    || target
    || t('app.targetUnknown')
})
const readyFileCount = computed(() => selectedFiles.value.filter(file => file.videoInfo && file.videoInfo.subtitle_tracks.length > 0).length)
const totalSubtitleTracks = computed(() => selectedFiles.value.reduce((total, file) => total + (file.videoInfo?.subtitle_tracks.length || 0), 0))
const totalBackups = computed(() => selectedFiles.value.reduce((total, file) => total + file.backups.length, 0))
const ffmpegStatusLabel = computed(() => {
  if (!ffmpegStatus.value) return t('app.checking')
  return ffmpegStatus.value.success ? t('app.online') : t('app.missing')
})
const ffmpegStatusClass = computed(() => ({
  online: ffmpegStatus.value?.success,
  offline: ffmpegStatus.value && !ffmpegStatus.value.success
}))

const {
  contextProfile,
  isExtracting: isExtractingContext,
  extractionStatus,
  contextEnabled,
  loadStoredProfile,
  updateProfile,
  clearProfile,
  extractFromVideo,
  saveProfileToFile,
  loadProfileFromFile,
  toggleContext
} = useContextProfile({ cachedSettings, getSettings })

const contextProfileRef = computed(() => contextProfile.value)

const {
  isTranslating,
  translationProgress,
  currentStatus,
  estimatedTime,
  canStartTranslation,
  resetProgress,
  startTranslation
} = useTranslationJob({
  selectedFiles,
  cachedSettings,
  ffmpegStatus,
  translationOptions,
  showSettings,
  getSettings,
  t,
  contextProfile: contextProfileRef,
  contextEnabled
})

const canExtractContext = computed(() => {
  const settings = cachedSettings.value
  return Boolean(settings?.selectedModel) && selectedFiles.value.some(
    f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0
  )
})

const onClearContext = () => {
  clearProfile()
}

const onUpdateContext = (profile: ContextProfile) => {
  updateProfile(profile)
}

const onExtractContext = async () => {
  const file = selectedFiles.value.find(
    f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0
  )
  if (!file || !file.videoInfo) return
  const trackIndex = translationOptions.subtitleTrack
    ? parseInt(translationOptions.subtitleTrack)
    : 0
  try {
    await extractFromVideo(file.path, file.name, trackIndex)
  } catch (e) {
    console.error('Context extraction failed:', e)
  }
}

const onSaveContextToFile = async () => {
  if (!contextProfile.value) return
  try {
    const { save } = await import('@tauri-apps/plugin-dialog')
    const filePath = await save({
      title: t('context.saveToFile'),
      defaultPath: 'context-profile.json',
      filters: [{ name: 'JSON', extensions: ['json'] }]
    })
    if (filePath) {
      await saveProfileToFile(filePath)
    }
  } catch (e) {
    console.error('Failed to save context profile:', e)
  }
}

const onLoadContextFromFile = async (filePath: string) => {
  try {
    await loadProfileFromFile(filePath)
  } catch (e) {
    console.error('Failed to load context profile:', e)
  }
}

const clearFiles = () => {
  clearSelectedFiles()
  resetProgress()
}

const setDragging = (value: boolean) => {
  if (isDragging.value !== value) {
    isDragging.value = value
  }
}

const changeInterfaceLanguage = (language: string | number) => {
  if (typeof language !== 'string') return
  updateSettings({
    interfaceLanguage: language as InterfaceLocale,
    hasSelectedInterfaceLanguage: true
  })
  showLanguageMenu.value = false
}

const completeLanguageSetup = () => {
  updateSettings({
    interfaceLanguage: setupLanguage.value,
    hasSelectedInterfaceLanguage: true
  })
  showLanguageSetup.value = false
}

watch(cachedSettings, (settings) => {
  const language = settings?.interfaceLanguage || defaultInterfaceLanguage
  setupLanguage.value = language
  setInterfaceLocale(language)
}, { immediate: true })

watch(isDark, (dark) => {
  document.documentElement.classList.toggle('theme-dark', dark)
  document.documentElement.classList.toggle('theme-light', !dark)
}, { immediate: true })

let cleanupDragDrop: (() => void) | null = null

const preventDefaults = (e: Event) => {
  e.preventDefault()
  e.stopPropagation()
}

onMounted(async () => {
  await loadCachedSettings()
  const settings = getSettings()
  if (settings && !settings.hasSelectedInterfaceLanguage) {
    setupLanguage.value = settings.interfaceLanguage
    showLanguageSetup.value = true
  }
  loadTranslationOptions()
  loadStoredProfile()
  await checkFFmpeg()

  window.addEventListener('dragover', preventDefaults)
  window.addEventListener('drop', preventDefaults)

  const unlistenDragDrop = await getCurrentWindow().onDragDropEvent(async (event) => {
    const payload = event.payload as DragDropEvent
    if (payload.type === 'enter') {
      setDragging(true)
      return
    }
    if (payload.type === 'leave') {
      setDragging(false)
      return
    }
    if (payload.type === 'drop') {
      setDragging(false)
      const paths = payload.paths || []
      if (paths.length > 0) {
        loadingFiles.value = true
        try {
          const allVideoPaths: string[] = []
          for (const path of paths) {
            try {
              const videos = await scanFolderForVideos(path)
              allVideoPaths.push(...videos)
            } catch {
              allVideoPaths.push(path)
            }
          }
          await addFiles(allVideoPaths)
        } finally {
          loadingFiles.value = false
        }
      }
    }
  })

  cleanupDragDrop = () => {
    unlistenDragDrop()
    window.removeEventListener('dragover', preventDefaults)
    window.removeEventListener('drop', preventDefaults)
  }
})

onUnmounted(() => {
  cleanupDragDrop?.()
})
</script>

<style>
:root {
  --font-sans: "Helvetica Neue", "Arial", -apple-system, sans-serif;
  --font-mono: "Courier New", "Courier", ui-monospace, monospace;
  --font-display: "Arial Black", "Helvetica Neue", sans-serif;
  --transition: 80ms ease;
}

html.theme-light {
  --bg-base: #ffffff;
  --bg-card: #ffffff;
  --bg-elevated: #f5f5f5;
  --border: #000000;
  --border-thin: #000000;
  --text-primary: #000000;
  --text-secondary: #555555;
  --text-muted: #888888;
  --accent: #000000;
  --accent-hover: #333333;
}

html.theme-dark {
  --bg-base: #0a0a0a;
  --bg-card: #0a0a0a;
  --bg-elevated: #141414;
  --border: #ffffff;
  --border-thin: #333333;
  --text-primary: #ffffff;
  --text-secondary: #888888;
  --text-muted: #555555;
  --accent: #ffffff;
  --accent-hover: #cccccc;
}

* {
  box-sizing: border-box;
}

html,
body,
#app {
  height: 100%;
  margin: 0;
  overflow: hidden;
}

body {
  color: var(--text-primary);
  background: var(--bg-base);
  font-family: var(--font-sans);
  font-size: 14px;
  line-height: 1.5;
  -webkit-font-smoothing: antialiased;
}

:focus-visible {
  outline: 3px solid var(--text-primary);
  outline-offset: 1px;
}

button,
input,
textarea {
  font-family: inherit;
}

.n-config-provider {
  height: 100%;
}
</style>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg-base);
  border: 2px solid var(--border);
}

/* ---- Header ---- */
.app-header {
  display: flex;
  align-items: center;
  gap: 24px;
  padding: 0 20px;
  height: 56px;
  border-bottom: 2px solid var(--border);
  background: var(--bg-base);
  -webkit-app-region: drag;
  flex-shrink: 0;
}

.app-header :deep(button) {
  -webkit-app-region: no-drag;
}

.header-brand {
  display: flex;
  align-items: center;
  gap: 10px;
  flex-shrink: 0;
}

.brand-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  background: var(--accent);
  color: var(--bg-base);
}

.brand-eyebrow {
  margin: 0;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.1em;
}

.brand-text h1 {
  margin: 0;
  font-family: var(--font-display);
  font-size: 16px;
  font-weight: 900;
  color: var(--text-primary);
  text-transform: uppercase;
  letter-spacing: -0.02em;
  line-height: 1;
}

/* ---- Status strip ---- */
.status-strip {
  display: flex;
  gap: 0;
  min-width: 0;
  flex-wrap: nowrap;
}

.status-badge {
  max-width: 200px;
  padding: 4px 10px;
  overflow: hidden;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-secondary);
  white-space: nowrap;
  text-overflow: ellipsis;
  border: 1px solid var(--border-thin);
  border-right: none;
  background: var(--bg-base);
}

.status-badge:last-child {
  border-right: 1px solid var(--border-thin);
}

.status-badge.online {
  color: var(--text-primary);
  font-weight: 700;
}

.status-badge.offline {
  color: var(--text-primary);
  font-weight: 700;
  text-decoration: line-through;
}

.header-actions {
  display: flex;
  gap: 0;
  margin-left: auto;
}

.icon-button {
  color: var(--text-primary) !important;
  border: 1px solid var(--border-thin) !important;
  border-radius: 0 !important;
  background: var(--bg-base) !important;
  transition: background var(--transition) !important;
}

.icon-button:hover {
  background: var(--accent) !important;
  color: var(--bg-base) !important;
}

.icon-button:active {
  transform: translate(2px, 2px);
}

/* ---- Language menu ---- */
.language-menu-wrap {
  position: relative;
  -webkit-app-region: no-drag;
}

.language-menu {
  position: absolute;
  top: 100%;
  right: 0;
  z-index: 20;
  display: grid;
  gap: 0;
  min-width: 180px;
  border: 2px solid var(--border);
  background: var(--bg-base);
}

.language-menu-item {
  padding: 10px 14px;
  color: var(--text-primary);
  font-size: 13px;
  text-align: left;
  border: none;
  border-bottom: 1px solid var(--border-thin);
  background: var(--bg-base);
  cursor: pointer;
  transition: background var(--transition);
}

.language-menu-item:last-child {
  border-bottom: none;
}

.language-menu-item:hover,
.language-menu-item.active {
  background: var(--accent);
  color: var(--bg-base);
}

/* ---- Main ---- */
.app-main {
  flex: 1;
  padding: 32px 24px;
  overflow: auto;
}

/* ---- Drop zone ---- */
.drop-zone {
  display: flex;
  align-items: center;
  justify-content: center;
  max-width: 600px;
  margin: 0 auto 24px;
  padding: 48px 32px;
  text-align: center;
  border: 2px solid var(--border);
  background: var(--bg-base);
  transition: background var(--transition);
}

.drop-zone:hover {
  background: var(--bg-elevated);
}

.drop-zone.drop-zone-active {
  background: var(--accent);
  color: var(--bg-base);
}

.drop-zone-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  max-width: 460px;
}

.drop-zone-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 56px;
  height: 56px;
  margin-bottom: 20px;
  border: 2px solid var(--border);
  background: var(--bg-base);
  color: var(--text-primary);
}

.drop-zone-content h2 {
  margin: 0;
  font-family: var(--font-display);
  font-size: 36px;
  font-weight: 900;
  color: var(--text-primary);
  line-height: 1;
  text-transform: uppercase;
  letter-spacing: -0.03em;
  text-wrap: balance;
}

.drop-zone.drop-zone-active .drop-zone-content h2,
.drop-zone.drop-zone-active .drop-zone-content p,
.drop-zone.drop-zone-active .drop-zone-icon {
  color: var(--bg-base);
  border-color: var(--bg-base);
}

.drop-zone-content > p {
  margin: 12px 0 0;
  color: var(--text-secondary);
  font-size: 14px;
  line-height: 1.5;
  max-width: 380px;
  font-family: var(--font-mono);
}

.drop-zone-actions {
  display: flex;
  justify-content: center;
  flex-wrap: wrap;
  gap: 0;
  margin-top: 28px;
}

/* ---- Alert ---- */
.ffmpeg-alert {
  max-width: 1260px;
  margin: 0 auto 20px;
  border: 2px solid var(--border) !important;
  border-radius: 0 !important;
  background: var(--bg-base) !important;
}

/* ---- Workspace ---- */
.workspace {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(340px, 380px);
  gap: 0;
  max-width: 1260px;
  margin: 0 auto;
  align-items: start;
  border: 2px solid var(--border);
}

.app-card {
  background: var(--bg-base);
}

.queue-panel {
  padding: 20px;
  border-right: 2px solid var(--border);
}

.translation-panel {
  padding: 20px;
  position: sticky;
  top: 0;
}

/* ---- Panel header ---- */
.panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border-thin);
}

.panel-label {
  margin: 0 0 2px;
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.1em;
}

.panel-title {
  margin: 0;
  font-family: var(--font-display);
  font-size: 18px;
  font-weight: 900;
  color: var(--text-primary);
  text-transform: uppercase;
  letter-spacing: -0.02em;
  line-height: 1;
}

/* ---- Queue stats ---- */
.queue-stats {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0;
  margin-bottom: 16px;
  border: 1px solid var(--border-thin);
}

.stat-item {
  padding: 12px;
  border-right: 1px solid var(--border-thin);
  background: var(--bg-base);
}

.stat-item:last-child {
  border-right: none;
}

.stat-item strong {
  display: block;
  font-family: var(--font-display);
  font-size: 28px;
  font-weight: 900;
  color: var(--text-primary);
  line-height: 1;
  letter-spacing: -0.03em;
}

.stat-item span {
  font-family: var(--font-mono);
  font-size: 10px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

/* ---- File list ---- */
.queue-scroll {
  max-height: 52vh;
}

.file-list {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.file-card {
  padding: 14px;
  border: 1px solid var(--border-thin);
  border-bottom: none;
  background: var(--bg-base);
  transition: background var(--transition);
}

.file-card:last-child {
  border-bottom: 1px solid var(--border-thin);
}

.file-card:hover {
  background: var(--bg-elevated);
}

.file-header {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  justify-content: space-between;
}

.file-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.file-icon {
  color: var(--text-primary);
  flex-shrink: 0;
}

.file-title-wrap {
  min-width: 0;
}

.file-title-wrap h4 {
  margin: 0;
  font-family: var(--font-sans);
  font-size: 14px;
  font-weight: 700;
  color: var(--text-primary);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.file-path {
  margin: 2px 0 0;
  font-size: 10px;
  color: var(--text-muted);
  font-family: var(--font-mono);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  max-width: 520px;
}

.file-actions {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  gap: 4px;
}

/* ---- Subtitle tracks ---- */
.subtitle-tracks,
.backups-section,
.file-error,
.no-subs {
  margin-top: 10px;
}

.subtitle-tracks {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.track-row {
  display: flex;
  gap: 8px;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  border: 1px solid var(--border-thin);
  border-top: none;
  background: var(--bg-base);
  transition: background var(--transition);
}

.track-row:first-child {
  border-top: 1px solid var(--border-thin);
}

.track-row:hover {
  background: var(--bg-elevated);
}

.track-meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  font-size: 12px;
}

.track-lang {
  padding: 1px 6px;
  font-family: var(--font-mono);
  font-weight: 700;
  font-size: 10px;
  color: var(--bg-base);
  background: var(--accent);
  text-transform: uppercase;
}

.track-title {
  color: var(--text-secondary);
}

.track-codec {
  color: var(--text-muted);
  font-size: 10px;
  font-family: var(--font-mono);
}

.track-actions,
.backup-actions {
  display: flex;
  flex-shrink: 0;
  gap: 0;
}

.no-subs {
  display: flex;
  gap: 6px;
  align-items: center;
  color: var(--text-muted);
  font-size: 12px;
  font-family: var(--font-mono);
  text-transform: uppercase;
}

/* ---- Backups ---- */
.backup-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  border: 1px solid var(--border-thin);
  border-top: none;
  background: var(--bg-elevated);
}

.backup-item:first-child {
  border-top: 1px solid var(--border-thin);
}

.backup-meta {
  font-size: 11px;
  color: var(--text-secondary);
  font-family: var(--font-mono);
}

/* ---- Translation panel ---- */
.status-indicator {
  width: 10px;
  height: 10px;
  border: 2px solid var(--border);
  background: var(--bg-base);
  transition: background var(--transition);
}

.status-indicator.online {
  background: var(--accent);
}

.translation-form {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.switch-stack {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin: 4px 0 12px;
  padding: 12px;
  border: 1px solid var(--border-thin);
  background: var(--bg-base);
}

.checkbox-label-content {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
}

.execute-block {
  margin-top: 16px;
}

.disabled-hint {
  margin: 8px 0 0;
  font-size: 11px;
  color: var(--text-muted);
  line-height: 1.5;
  font-family: var(--font-mono);
}

.progress-block {
  margin-top: 12px;
  padding: 12px;
  border: 1px solid var(--border-thin);
  background: var(--bg-base);
}

.progress-header {
  display: flex;
  justify-content: space-between;
  margin-bottom: 6px;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-secondary);
}

.progress-time {
  color: var(--text-muted);
}

.progress-status {
  margin: 6px 0 0;
  font-size: 11px;
  color: var(--text-muted);
  line-height: 1.5;
  font-family: var(--font-mono);
}

/* ---- Setup overlay ---- */
.setup-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: grid;
  place-items: center;
  padding: 16px;
  background: var(--bg-base);
}

.setup-card {
  width: min(420px, calc(100vw - 32px));
  border: 2px solid var(--border);
  background: var(--bg-base);
}

.setup-header {
  padding: 16px 20px;
  border-bottom: 2px solid var(--border);
}

.setup-header h2 {
  margin: 0;
  font-family: var(--font-display);
  font-size: 22px;
  font-weight: 900;
  color: var(--text-primary);
  text-transform: uppercase;
  letter-spacing: -0.02em;
  line-height: 1;
}

.setup-body {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 20px;
}

.setup-description {
  margin: 6px 0 0;
  color: var(--text-secondary);
  line-height: 1.5;
  font-size: 13px;
  font-family: var(--font-mono);
}

.language-choice-grid {
  display: grid;
  gap: 0;
}

.language-choice {
  padding: 12px 14px;
  color: var(--text-primary);
  font-size: 14px;
  text-align: left;
  border: 1px solid var(--border-thin);
  border-bottom: none;
  background: var(--bg-base);
  cursor: pointer;
  transition: background var(--transition), color var(--transition);
}

.language-choice:last-child {
  border-bottom: 1px solid var(--border-thin);
}

.language-choice:hover {
  background: var(--bg-elevated);
}

.language-choice:active {
  transform: translate(2px, 2px);
}

.language-choice.active {
  background: var(--accent);
  color: var(--bg-base);
  font-weight: 700;
}

.setup-continue {
  display: inline-flex;
  justify-content: center;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 12px 16px;
  color: var(--bg-base);
  font-family: var(--font-display);
  font-size: 14px;
  font-weight: 900;
  text-transform: uppercase;
  letter-spacing: 0.02em;
  border: 2px solid var(--border);
  background: var(--accent);
  cursor: pointer;
  transition: transform var(--transition);
}

.setup-continue:hover {
  background: var(--accent-hover);
}

.setup-continue:active {
  transform: translate(2px, 2px);
}

/* ---- Deep overrides for Naive UI ---- */
:deep(.n-button) {
  border-radius: 0 !important;
  font-weight: 700;
  letter-spacing: 0;
  transition: all var(--transition) !important;
}

:deep(.n-button:active) {
  transform: translate(2px, 2px);
}

:deep(.n-button--primary-type) {
  color: var(--bg-base) !important;
  font-weight: 900;
  background: var(--accent) !important;
  border: 2px solid var(--accent) !important;
  box-shadow: none;
}

:deep(.n-button--primary-type:hover) {
  background: var(--accent-hover) !important;
  border-color: var(--accent-hover) !important;
}

:deep(.n-button--quaternary-type) {
  border: 1px solid var(--border-thin) !important;
  color: var(--text-primary) !important;
}

:deep(.n-button--quaternary-type:hover) {
  background: var(--accent) !important;
  color: var(--bg-base) !important;
}

:deep(.n-base-selection),
:deep(.n-input),
:deep(.n-input-number),
:deep(.n-input-number .n-input) {
  border-radius: 0 !important;
  background: var(--bg-base) !important;
  border: 1px solid var(--border-thin) !important;
}

:deep(.n-form-item-label__text),
:deep(.n-collapse-item__header-main) {
  color: var(--text-secondary) !important;
  font-size: 12px !important;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  font-family: var(--font-mono);
}

:deep(.n-checkbox__label) {
  color: var(--text-primary) !important;
  font-size: 13px;
}

:deep(.n-tag) {
  border-radius: 0;
  font-weight: 700;
  font-family: var(--font-mono);
  font-size: 10px !important;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  border: 1px solid var(--border-thin) !important;
  background: var(--bg-base) !important;
  color: var(--text-primary) !important;
}

:deep(.n-progress-graph-line-fill) {
  background: var(--accent) !important;
}

:deep(.n-divider) {
  --n-color: var(--border-thin);
}

:deep(.n-divider__title) {
  font-family: var(--font-mono);
  color: var(--text-muted);
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
}

:deep(.n-alert) {
  border-radius: 0 !important;
  border: 2px solid var(--border) !important;
}

/* ---- Responsive ---- */
@media (max-width: 980px) {
  .status-strip {
    display: none;
  }
}

@media (max-width: 820px) {
  .workspace {
    grid-template-columns: 1fr;
  }

  .queue-panel {
    border-right: none;
    border-bottom: 2px solid var(--border);
  }

  .translation-panel {
    position: static;
  }
}

@media (max-width: 720px) {
  .app-main {
    padding: 16px 12px;
  }

  .app-header {
    padding: 0 12px;
  }

  .drop-zone,
  .queue-panel,
  .translation-panel {
    padding: 16px;
  }

  .drop-zone-content h2 {
    font-size: 26px;
  }

  .track-row,
  .backup-item,
  .file-header {
    flex-direction: column;
  }

  .queue-stats {
    grid-template-columns: 1fr;
  }

  .stat-item {
    border-right: none;
    border-bottom: 1px solid var(--border-thin);
  }

  .stat-item:last-child {
    border-bottom: none;
  }

  .queue-scroll {
    max-height: none;
  }
}
</style>
