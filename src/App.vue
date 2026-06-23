<template>
  <n-config-provider :theme="theme" :theme-overrides="themeOverrides">
    <n-message-provider>
      <n-notification-provider>
        <n-dialog-provider>
          <div class="app-shell" :class="{ 'is-dragging': isDragging }">
            <header class="wired-header">
              <div class="identity-block">
                <div>
                  <p class="eyebrow">{{ t('app.eyebrow') }}</p>
                  <h1>{{ t('app.title') }}</h1>
                </div>
              </div>

              <div class="status-strip" :aria-label="t('app.runtimeStatus')">
                <span class="status-pill">{{ t('app.provider', { provider: providerLabel }) }}</span>
                <span class="status-pill truncate">{{ t('app.model', { model: modelLabel }) }}</span>
                <span class="status-pill" :class="ffmpegStatusClass">
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

            <main class="wired-main">
              <section
                class="hero-port"
                :class="{ 'drop-zone-active': isDragging }"
                @dragover.prevent="setDragging(true)"
                @dragleave.prevent="setDragging(false)"
                @drop.prevent="handleDrop"
              >
                <div class="hero-copy">
                  <p class="terminal-line">{{ t('app.terminalLine') }}</p>
                  <h2>{{ t('app.heroTitle') }}</h2>
                  <p>
                    {{ t('app.heroDescription') }}
                  </p>
                  <div class="hero-actions">
                    <n-button type="primary" size="large" class="primary-command" @click="selectFiles" :loading="loadingFiles">
                      <template #icon><n-icon><document-outline /></n-icon></template>
                      {{ t('app.selectFiles') }}
                    </n-button>
                    <n-button size="large" class="secondary-command" @click="selectFolder" :loading="loadingFiles">
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
                class="wired-alert"
              >
                {{ t('app.ffmpegMissingDescription') }}
              </n-alert>

              <section v-if="selectedFiles.length > 0" class="workspace-grid">
                <div class="queue-panel wired-panel">
                  <div class="panel-heading">
                    <div>
                      <p class="eyebrow">{{ t('app.mediaQueue') }}</p>
                      <h3>{{ t('app.packetsAttached', { count: selectedFiles.length }) }}</h3>
                    </div>
                    <n-button text type="error" class="clear-command" @click="clearFiles">
                      <template #icon><n-icon><trash-outline /></n-icon></template>
                      {{ t('app.clear') }}
                    </n-button>
                  </div>

                  <div class="queue-stats">
                    <div><strong>{{ readyFileCount }}</strong><span>{{ t('app.ready') }}</span></div>
                    <div><strong>{{ totalSubtitleTracks }}</strong><span>{{ t('app.tracks') }}</span></div>
                    <div><strong>{{ totalBackups }}</strong><span>{{ t('app.backups') }}</span></div>
                  </div>

                  <n-scrollbar class="queue-scroll">
                    <div class="file-list">
                      <article v-for="(file, index) in selectedFiles" :key="file.path" class="file-item">
                        <div class="file-header">
                          <div class="file-title-row">
                            <n-icon size="18"><videocam-outline /></n-icon>
                            <div class="file-title-wrap">
                              <h4>{{ file.name }}</h4>
                              <p>{{ file.path }}</p>
                            </div>
                          </div>
                          <div class="file-actions">
                            <n-spin v-if="file.loading" size="small" />
                            <n-tag v-if="file.videoInfo" size="small" :bordered="false" class="wired-tag">
                              {{ t('app.subs', { count: file.videoInfo.subtitle_tracks.length }) }}
                            </n-tag>
                            <n-button text type="error" @click="removeFile(index)">
                              <template #icon><n-icon><close-outline /></n-icon></template>
                            </n-button>
                          </div>
                        </div>

                        <div v-if="file.videoInfo && file.videoInfo.subtitle_tracks.length > 0" class="subtitle-tracks">
                          <div v-for="track in file.videoInfo.subtitle_tracks" :key="track.index" class="subtitle-track">
                            <div class="track-meta">
                              <span class="track-lang">{{ track.language || 'und' }}</span>
                              <span>{{ track.title || t('track.title', { index: track.index }) }}</span>
                              <span class="track-codec">{{ track.codec }}</span>
                              <span v-if="track.default" class="track-flag">{{ t('app.default') }}</span>
                              <span v-if="track.forced" class="track-flag warn">{{ t('app.forced') }}</span>
                            </div>
                            <div class="track-actions">
                              <n-button
                                size="tiny"
                                quaternary
                                :title="t('app.extractSubtitle')"
                                @click="extractSubtitle(file, track.index)"
                                :loading="extractingSubtitle === file.path"
                              >
                                <template #icon><n-icon><download-outline /></n-icon></template>
                              </n-button>
                              <n-button
                                size="tiny"
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

                        <div v-else-if="file.videoInfo && file.videoInfo.subtitle_tracks.length === 0" class="no-subs-warning">
                          <n-icon size="16"><information-circle-outline /></n-icon>
                          <span>{{ t('app.noSubtitleTracks') }}</span>
                        </div>

                        <div v-if="file.error" class="file-error">
                          <n-text type="error" depth="3">{{ file.error }}</n-text>
                        </div>

                        <div v-if="file.backups.length > 0" class="backups-section">
                          <n-divider class="wired-divider">{{ t('app.backupDivider') }}</n-divider>
                          <div v-for="backup in file.backups" :key="backup.backup_path" class="backup-item">
                            <span>
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

                <aside class="protocol-panel wired-panel">
                  <div class="panel-heading">
                    <div>
                      <p class="eyebrow">{{ t('app.translationProtocol') }}</p>
                      <h3>{{ targetLanguageLabel }}</h3>
                    </div>
                    <span class="status-dot" :class="{ online: canStartTranslation }"></span>
                  </div>

                  <div class="protocol-form">
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

                    <n-collapse class="wired-collapse">
                      <n-collapse-item :title="t('app.advancedSignalControls')" name="advanced">
                        <n-form-item :label="t('app.customPrompt')">
                          <n-input v-model:value="translationOptions.customPrompt" type="textarea" :placeholder="t('app.customPromptPlaceholder')" :rows="4" />
                        </n-form-item>
                      </n-collapse-item>
                    </n-collapse>
                  </div>

                  <div class="execute-block">
                    <n-button type="primary" size="large" block class="execute-command" :loading="isTranslating" :disabled="!canStartTranslation" @click="startTranslation">
                      <template #icon><n-icon><play-outline /></n-icon></template>
                      {{ isTranslating ? t('app.translatingSignal') : t('app.initiateTranslation') }}
                    </n-button>
                    <p v-if="!canStartTranslation" class="disabled-hint">
                      {{ t('app.disabledHint') }}
                    </p>
                  </div>

                  <div v-if="isTranslating || translationProgress > 0" class="progress-console">
                    <div class="progress-head">
                      <span>{{ t('app.sync', { progress: Math.round(translationProgress) }) }}</span>
                      <span>{{ estimatedTime }}</span>
                    </div>
                    <n-progress type="line" :percentage="translationProgress" :status="translationProgress === 100 ? 'success' : 'default'" :show-indicator="false" />
                    <p class="progress-status">{{ currentStatus || t('app.awaitingPacketResponse') }}</p>
                  </div>
                </aside>
              </section>
            </main>
          </div>

          <SettingsModal v-model:show="showSettings" ref="settingsRef" />

          <div v-if="showLanguageSetup" class="language-setup-overlay" role="dialog" aria-modal="true">
            <section class="language-setup-panel" :aria-label="t('setup.title')">
              <header class="language-setup-header">
                <h2>{{ t('setup.title') }}</h2>
              </header>
              <div class="language-setup-stack">
                <div>
                  <p class="eyebrow">{{ t('setup.eyebrow') }}</p>
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
                <button type="button" class="setup-continue-command" @click="completeLanguageSetup">
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
  InformationCircleOutline
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

const SettingsModal = defineAsyncComponent(() => import('./components/SettingsModal.vue'))

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
  settingsRef,
  showSettings,
  getSettings,
  t
})

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
  --wired-black: #020205;
  --wired-void: #050510;
  --wired-panel: #0a0a18;
  --wired-panel-2: #111128;
  --wired-panel-3: #181838;
  --wired-paper: #7ce8a0;
  --wired-paper-bright: #8fffb8;
  --wired-muted: #4a7a5c;
  --wired-faint: #2a4a38;
  --wired-pink: #8088cc;
  --wired-red: #cc5588;
  --wired-red-dark: #2a1040;
  --wired-border: rgba(124, 232, 160, 0.16);
  --wired-border-strong: rgba(143, 255, 184, 0.32);
  --wired-glow: rgba(124, 232, 160, 0.12);
  --wired-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
  --font-wired: ui-monospace, "SFMono-Regular", "Cascadia Code", "Liberation Mono", Menlo, monospace;
  --font-body: "Avenir Next", "Segoe UI", sans-serif;
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
  color: var(--wired-paper);
  background: var(--wired-black);
  font-family: var(--font-body);
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
  position: relative;
  display: flex;
  flex-direction: column;
  height: 100vh;
  color: var(--wired-paper);
  background: linear-gradient(180deg, #02030c 0%, #040615 40%, #050510 70%, #030418 100%);
  isolation: isolate;
}

.app-shell::before {
  position: absolute;
  inset: 0;
  z-index: -1;
  pointer-events: none;
  content: "";
  opacity: 0.18;
  background: radial-gradient(ellipse 80% 100% at 10% 0%, rgba(124, 232, 160, 0.04), transparent 55%);
}

.wired-header {
  display: flex;
  align-items: center;
  gap: 18px;
  padding: 12px 20px;
  border-bottom: 1px solid var(--wired-border);
  background: rgba(2, 2, 5, 0.78);
  -webkit-app-region: drag;
}

.wired-header :deep(button) {
  -webkit-app-region: no-drag;
}

.setup-description {
  margin-top: 8px;
  color: var(--wired-paper);
  line-height: 1.6;
}

.language-setup-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: grid;
  place-items: center;
  padding: 14px;
  background: rgba(2, 2, 5, 0.68);
}

.language-setup-panel {
  width: min(520px, calc(100vw - 28px));
  color: var(--wired-paper);
  border: 1px solid var(--wired-border-strong);
  background: linear-gradient(180deg, rgba(10, 10, 28, 0.98), rgba(3, 3, 14, 0.98));
  box-shadow: var(--wired-shadow);
}

.language-setup-header {
  padding: 18px 22px;
  border-bottom: 1px solid var(--wired-border);
  background: rgba(2, 2, 5, 0.40);
}

.language-setup-header h2 {
  color: var(--wired-paper-bright);
  font-family: var(--font-wired);
  font-size: 14px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
}

.language-setup-stack {
  display: flex;
  flex-direction: column;
  gap: 22px;
  padding: 22px;
}

.language-choice-grid {
  display: grid;
  gap: 10px;
}

.language-choice {
  padding: 12px 14px;
}

.setup-continue-command {
  display: inline-flex;
  justify-content: center;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 12px 16px;
  color: var(--wired-black);
  font-family: var(--font-wired);
  font-weight: 900;
  letter-spacing: 0.05em;
  border: 1px solid var(--wired-paper);
  background: var(--wired-paper);
  box-shadow: 3px 3px 0 var(--wired-red-dark);
  cursor: pointer;
}

.setup-continue-command:hover {
  background: var(--wired-paper-bright);
}

.identity-block,
.header-actions,
.status-strip,
.file-title-row,
.file-actions,
.panel-heading,
.track-meta,
.track-actions,
.backup-item,
.progress-head,
.hero-actions {
  display: flex;
  align-items: center;
}

.identity-block {
  display: flex;
  align-items: center;
  gap: 14px;
  flex-shrink: 0;
}

.eyebrow,
.terminal-line,
.status-pill,
.queue-stats span,
.disabled-hint,
.progress-head,
.progress-status {
  font-family: var(--font-wired);
  text-transform: uppercase;
  letter-spacing: 0.12em;
}

.eyebrow {
  margin: 0 0 4px;
  color: var(--wired-pink);
  font-size: 11px;
}

h1,
h2,
h3,
h4,
p {
  margin: 0;
}

h1 {
  color: var(--wired-paper-bright);
  font-family: var(--font-wired);
  font-size: 18px;
  font-weight: 800;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.status-strip {
  display: flex;
  gap: 8px;
  min-width: 0;
  flex-wrap: nowrap;
}

.status-pill {
  max-width: 220px;
  padding: 7px 10px;
  overflow: hidden;
  color: var(--wired-muted);
  font-size: 10px;
  white-space: nowrap;
  text-overflow: ellipsis;
  border: 1px solid var(--wired-border);
  background: rgba(5, 5, 16, 0.82);
}

.status-pill.online {
  color: var(--wired-paper-bright);
  border-color: rgba(124, 232, 160, 0.44);
}

.status-pill.offline {
  color: var(--wired-pink);
  border-color: rgba(204, 85, 136, 0.48);
}

.header-actions {
  display: flex;
  gap: 6px;
  margin-left: auto;
}

.language-menu-wrap {
  position: relative;
  -webkit-app-region: no-drag;
}

.language-menu {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  z-index: 20;
  display: grid;
  min-width: 180px;
  padding: 6px;
  border: 1px solid var(--wired-border);
  background: var(--wired-panel);
  box-shadow: var(--wired-shadow);
}

.language-menu-item,
.language-choice {
  color: var(--wired-paper);
  font-family: var(--font-wired);
  font-size: 12px;
  text-align: left;
  border: 1px solid transparent;
  background: transparent;
  cursor: pointer;
}

.language-menu-item {
  padding: 9px 10px;
}

.language-menu-item:hover,
.language-menu-item.active,
.language-choice:hover,
.language-choice.active {
  color: var(--wired-paper-bright);
  border-color: var(--wired-border-strong);
  background: rgba(124, 232, 160, 0.07);
}

.wired-main {
  flex: 1;
  padding: 24px;
  overflow: auto;
}

.hero-port {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  max-width: 720px;
  margin: 0 auto 22px;
  padding: 48px 24px 52px;
  text-align: center;
  border: 1px dashed var(--wired-border-strong);
  background: rgba(5, 5, 16, 0.34);
  transition: border-color 120ms ease, background-color 120ms ease;
}

.hero-port.drop-zone-active {
  border-color: var(--wired-paper-bright);
  background: rgba(10, 10, 28, 0.68);
}

.hero-copy {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  max-width: 600px;
  padding: 0 12px;
}

.terminal-line {
  color: var(--wired-red);
  font-size: 12px;
}

.hero-copy h2 {
  max-width: 580px;
  margin-top: 12px;
  color: var(--wired-paper-bright);
  font-family: var(--font-wired);
  font-size: clamp(32px, 5vw, 62px);
  line-height: 0.94;
  text-transform: uppercase;
  text-align: center;
}

.hero-copy > p:not(.terminal-line) {
  max-width: 480px;
  margin-top: 18px;
  color: var(--wired-muted);
  font-size: 15px;
  line-height: 1.65;
  text-align: center;
}

.hero-actions {
  display: flex;
  justify-content: center;
  flex-wrap: wrap;
  gap: 12px;
  margin-top: 28px;
}

.wired-alert,
.workspace-grid {
  max-width: 1260px;
  margin: 0 auto 18px;
}

.workspace-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(340px, 420px);
  gap: 18px;
  align-items: start;
}

.wired-panel {
  border: 1px solid var(--wired-border);
  background: linear-gradient(180deg, rgba(10, 10, 28, 0.9), rgba(6, 6, 18, 0.94));
  box-shadow: var(--wired-shadow);
}

.queue-panel,
.protocol-panel {
  padding: 18px;
}

.panel-heading {
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}

.panel-heading h3 {
  color: var(--wired-paper-bright);
  font-family: var(--font-wired);
  font-size: 20px;
  text-transform: uppercase;
}

.queue-stats {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin-bottom: 14px;
}

.queue-stats div {
  padding: 12px;
  border: 1px solid rgba(124, 232, 160, 0.12);
  background: rgba(2, 2, 5, 0.38);
}

.queue-stats strong {
  display: block;
  color: var(--wired-paper-bright);
  font-family: var(--font-wired);
  font-size: 24px;
}

.queue-stats span {
  color: var(--wired-muted);
  font-size: 10px;
}

.queue-scroll {
  max-height: 52vh;
}

.file-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding-right: 8px;
}

.file-item {
  content-visibility: auto;
  contain-intrinsic-size: 140px;
  padding: 14px;
  border: 1px solid rgba(124, 232, 160, 0.12);
  background: rgba(2, 2, 5, 0.42);
}

.file-header {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
}

.file-title-row {
  min-width: 0;
  gap: 10px;
  color: var(--wired-pink);
}

.file-title-wrap {
  min-width: 0;
}

.file-title-wrap h4 {
  overflow: hidden;
  color: var(--wired-paper-bright);
  font-family: var(--font-wired);
  font-size: 14px;
  line-height: 1.3;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.file-title-wrap p {
  max-width: 520px;
  overflow: hidden;
  color: var(--wired-faint);
  font-family: var(--font-wired);
  font-size: 10px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.file-actions {
  flex-shrink: 0;
  gap: 6px;
}

.subtitle-tracks,
.backups-section,
.file-error,
.no-subs-warning {
  margin-top: 12px;
}

.subtitle-tracks {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.subtitle-track {
  display: flex;
  gap: 10px;
  align-items: center;
  justify-content: space-between;
  padding: 8px 10px;
  border: 1px solid rgba(124, 232, 160, 0.09);
  background: rgba(2, 2, 5, 0.38);
}

.track-meta {
  flex-wrap: wrap;
  gap: 8px;
  color: var(--wired-muted);
  font-family: var(--font-wired);
  font-size: 11px;
}

.track-lang,
.track-flag {
  padding: 2px 6px;
  color: var(--wired-black);
  font-weight: 800;
  background: var(--wired-paper);
}

.track-flag {
  color: var(--wired-paper-bright);
  background: rgba(124, 232, 160, 0.12);
}

.track-flag.warn {
  color: var(--wired-pink);
}

.track-codec {
  color: var(--wired-faint);
}

.track-actions,
.backup-actions {
  display: flex;
  flex-shrink: 0;
  gap: 4px;
}

.no-subs-warning {
  display: flex;
  gap: 8px;
  align-items: center;
  color: var(--wired-pink);
  font-family: var(--font-wired);
  font-size: 11px;
  text-transform: uppercase;
}

.backup-item {
  justify-content: space-between;
  gap: 12px;
  padding: 7px 9px;
  color: var(--wired-muted);
  font-family: var(--font-wired);
  font-size: 10px;
  border: 1px solid rgba(128, 136, 204, 0.14);
  background: rgba(128, 136, 204, 0.06);
}

.protocol-panel {
  position: sticky;
  top: 0;
}

.status-dot {
  width: 12px;
  height: 12px;
  border: 1px solid var(--wired-red);
  background: var(--wired-red-dark);
}

.status-dot.online {
  border-color: var(--wired-paper-bright);
  background: var(--wired-paper);
}

.protocol-form {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.switch-stack {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin: 4px 0 12px;
  padding: 12px;
  border: 1px solid rgba(124, 232, 160, 0.10);
  background: rgba(2, 2, 5, 0.36);
}

.checkbox-label-content {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.execute-block {
  margin-top: 18px;
}

.disabled-hint {
  margin-top: 10px;
  color: var(--wired-faint);
  font-size: 10px;
  line-height: 1.5;
}

.progress-console {
  margin-top: 16px;
  padding: 12px;
  border: 1px solid rgba(124, 232, 160, 0.14);
  background: #030312;
}

.progress-head {
  justify-content: space-between;
  margin-bottom: 8px;
  color: var(--wired-paper-bright);
  font-size: 10px;
}

.progress-status {
  margin-top: 10px;
  color: var(--wired-muted);
  font-size: 10px;
  line-height: 1.5;
}

:deep(.n-button) {
  font-family: var(--font-wired);
  letter-spacing: 0.05em;
  border-radius: 0;
}

:deep(.n-button--primary-type) {
  color: var(--wired-black) !important;
  font-weight: 900;
  background: var(--wired-paper) !important;
  border-color: var(--wired-paper) !important;
  box-shadow: 3px 3px 0 var(--wired-red-dark);
}

:deep(.n-button--primary-type:hover) {
  background: var(--wired-paper-bright) !important;
  border-color: var(--wired-paper-bright) !important;
}

.secondary-command,
.icon-button {
  color: var(--wired-paper) !important;
  border: 1px solid var(--wired-border) !important;
  background: rgba(2, 2, 5, 0.34) !important;
}

:deep(.n-base-selection),
:deep(.n-input),
:deep(.n-input-number),
:deep(.n-input-number .n-input) {
  border-radius: 0 !important;
  background: rgba(2, 2, 5, 0.46) !important;
}

:deep(.n-base-selection .n-base-selection-label),
:deep(.n-input-wrapper),
:deep(.n-input__textarea-el),
:deep(.n-input__input-el) {
  color: var(--wired-paper) !important;
  font-family: var(--font-wired) !important;
}

:deep(.n-form-item-label__text),
:deep(.n-collapse-item__header-main) {
  color: var(--wired-muted) !important;
  font-family: var(--font-wired) !important;
  font-size: 11px !important;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

:deep(.n-checkbox__label) {
  color: var(--wired-paper) !important;
  font-family: var(--font-wired);
  font-size: 12px;
}

:deep(.n-tag) {
  border-radius: 0;
  font-family: var(--font-wired);
  text-transform: uppercase;
}

:deep(.n-progress-graph-line-fill) {
  background: linear-gradient(90deg, var(--wired-pink), var(--wired-paper-bright)) !important;
}

.wired-divider :deep(.n-divider__title) {
  color: var(--wired-faint);
  font-family: var(--font-wired);
  font-size: 10px;
  text-transform: uppercase;
}

@media (max-width: 980px) {
  .status-strip {
    display: none;
  }
}

@media (max-width: 820px) {
  .workspace-grid {
    grid-template-columns: 1fr;
  }

  .protocol-panel {
    position: static;
  }

}

@media (max-width: 720px) {
  .wired-main {
    padding: 14px;
  }

  .wired-header {
    gap: 12px;
    padding: 14px;
  }

  .hero-port,
  .queue-panel,
  .protocol-panel {
    padding: 14px;
  }

  .hero-copy {
    padding: 0;
  }

  .hero-copy h2 {
    font-size: 28px;
  }

  .hero-actions,
  .header-actions,
  .subtitle-track,
  .backup-item {
    align-items: stretch;
  }

  .subtitle-track,
  .backup-item,
  .file-header {
    flex-direction: column;
  }

  .queue-stats {
    grid-template-columns: 1fr;
  }

  .queue-scroll {
    max-height: none;
  }
}
</style>
