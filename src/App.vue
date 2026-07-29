<template>
  <n-config-provider :theme="theme" :theme-overrides="themeOverrides">
    <n-message-provider>
      <n-notification-provider>
        <n-dialog-provider>
          <div class="app-shell" :class="{ 'is-dragging': isDragging }">
            <header class="app-header">
              <div class="header-brand">
                <div class="brand-icon">
                  <n-icon size="18"><film-outline /></n-icon>
                </div>
                <div class="brand-text">
                  <p class="brand-eyebrow">{{ t('app.eyebrow') }}</p>
                  <h1>{{ t('app.title') }}</h1>
                </div>
              </div>

              <n-popover trigger="click" placement="bottom-start">
                <template #trigger>
                  <button type="button" class="engine-chip" :class="{ ready: engineReady }">
                    <span class="engine-dot" />
                    <span>{{ t('app.engine') }}</span>
                  </button>
                </template>
                <div class="engine-popover">
                  <div class="engine-row">
                    <span class="engine-label">{{ t('app.provider', { provider: providerLabel }) }}</span>
                  </div>
                  <div class="engine-row">
                    <span class="engine-label">{{ t('app.model', { model: modelLabel }) }}</span>
                  </div>
                  <div class="engine-row">
                    <span class="engine-label" :class="{ 'engine-error': !ffmpegOk }">
                      {{ t('app.ffmpeg', { status: ffmpegStatusLabel }) }}
                    </span>
                  </div>
                </div>
              </n-popover>

              <div class="header-actions">
                <n-dropdown
                  trigger="click"
                  :options="themeMenuOptions"
                  @select="setTheme"
                >
                  <n-button quaternary circle class="icon-button" :title="t('app.theme')">
                    <template #icon>
                      <n-icon size="18"><color-palette-outline /></n-icon>
                    </template>
                  </n-button>
                </n-dropdown>
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
              <EmptyState
                v-if="selectedFiles.length === 0"
                :loading="loadingFiles"
                :provider-ready="providerReady"
                :provider-name="providerLabel"
                :ffmpeg-ready="ffmpegOk"
                @select-files="selectFiles"
                @select-folder="selectFolder"
                @open-settings="showSettings = true"
              />

              <div v-else class="workspace">
                <section class="queue-panel">
                  <div class="queue-toolbar">
                    <div class="queue-toolbar-text">
                      <h2 class="queue-title">{{ t('queue.title') }}</h2>
                      <p class="queue-hint">
                        <n-icon size="13"><shield-checkmark-outline /></n-icon>
                        {{ t('queue.originalsSafe') }}
                      </p>
                    </div>
                    <div class="queue-toolbar-actions">
                      <n-button size="small" tertiary :loading="loadingFiles" @click="selectFiles">
                        <template #icon><n-icon><add-outline /></n-icon></template>
                        {{ t('queue.addFiles') }}
                      </n-button>
                      <n-button size="small" tertiary :loading="loadingFiles" @click="selectFolder">
                        <template #icon><n-icon><folder-open-outline /></n-icon></template>
                        {{ t('queue.addFolder') }}
                      </n-button>
                      <n-button size="small" quaternary type="error" @click="clearFiles">
                        <template #icon><n-icon><trash-outline /></n-icon></template>
                        {{ t('queue.clearAll') }}
                      </n-button>
                    </div>
                  </div>

                  <n-alert
                    v-if="ffmpegStatus && !ffmpegStatus.success"
                    type="warning"
                    :title="t('app.ffmpegMissingTitle')"
                    closable
                    class="ffmpeg-alert"
                  >
                    {{ t('app.ffmpegMissingDescription') }}
                  </n-alert>

                  <n-scrollbar class="queue-scroll">
                    <div class="queue-list">
                      <QueueRow
                        v-for="(file, index) in selectedFiles"
                        :key="file.path"
                        :file="file"
                        :state="getRowState(file)"
                        :queue-pos="queuePosition(file)"
                        :outcome="fileOutcomes[file.path]"
                        :phase="activeFilePath === file.path ? activeFilePhase : 'other'"
                        :lines="activeFilePath === file.path ? activeFileLines : null"
                        :expanded="!!expandedRows[file.path]"
                        :extracting="extractingSubtitle === file.path"
                        :backing-up="backingUp === file.path"
                        @remove="onRemoveFile(index, file.path)"
                        @retry="onRetryFile"
                        @retranslate="onRetryFile"
                        @toggle-expand="toggleRow(file.path)"
                        @extract="(track: number) => extractSubtitle(file, track)"
                        @backup="(track: number) => backupSubtitle(file, track)"
                        @restore="(backup: BackupInfo) => restoreBackup(file, backup)"
                        @delete-backup="(backup: BackupInfo) => deleteBackup(file, backup)"
                        @open-settings="showSettings = true"
                      />
                    </div>
                  </n-scrollbar>
                </section>

                <aside class="side-panel">
                  <h3 class="side-title">{{ t('app.translation') }}</h3>

                  <div class="side-field">
                    <label class="side-label">{{ t('app.targetLanguage') }}</label>
                    <n-select
                      v-model:value="targetLanguageModel"
                      :options="languageOptions"
                      filterable
                      tag
                    />
                  </div>

                  <div class="side-field">
                    <label class="side-label">{{ t('app.subtitleTrack') }}</label>
                    <n-select
                      v-model:value="translationOptions.subtitleTrack"
                      :options="subtitleTrackOptions"
                    />
                  </div>

                  <n-checkbox v-model:checked="translationOptions.embedSubtitles">
                    {{ t('app.embedTranslatedSubtitles') }}
                  </n-checkbox>

                  <n-collapse class="side-collapse">
                    <n-collapse-item name="context">
                      <template #header>
                        <div class="collapse-header">
                          <span>{{ t('app.seriesContext') }}</span>
                          <span class="collapse-hint">{{ t('app.seriesContextHint') }}</span>
                        </div>
                      </template>
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
                    <n-collapse-item :title="t('app.advancedOptions')" name="advanced">
                      <div class="side-field">
                        <label class="side-label">{{ t('app.customPrompt') }}</label>
                        <n-input
                          v-model:value="translationOptions.customPrompt"
                          type="textarea"
                          :placeholder="t('app.customPromptPlaceholder')"
                          :rows="3"
                        />
                      </div>
                      <n-checkbox
                        v-model:checked="translationOptions.useMkvmerge"
                        :disabled="!translationOptions.embedSubtitles"
                      >
                        {{ t('app.useMkvmerge') }}
                      </n-checkbox>
                    </n-collapse-item>
                  </n-collapse>
                </aside>
              </div>
            </main>

            <LogDrawer />

            <footer class="app-footer">
              <div class="footer-stats">
                <span v-if="counts.ready > 0">{{ t('footer.ready', { n: counts.ready }) }}</span>
                <span v-if="counts.translating > 0" class="stat-accent">
                  {{ t('footer.translating', { n: counts.translating }) }}
                </span>
                <span v-if="counts.done > 0" class="stat-success">
                  {{ t('footer.done', { n: counts.done }) }}
                </span>
                <span v-if="counts.failed > 0" class="stat-danger">
                  {{ t('footer.failed', { n: counts.failed }) }}
                </span>
                <span v-if="footerReason" class="footer-reason">{{ footerReason }}</span>
              </div>
              <div class="footer-action">
                <n-progress
                  v-if="isTranslating"
                  type="line"
                  :percentage="translationProgress"
                  :show-indicator="false"
                  :height="4"
                  border-radius="2px"
                  class="footer-progress"
                />
                <n-button
                  v-if="primaryKind === 'translate'"
                  type="primary"
                  size="large"
                  :disabled="!canStartTranslation"
                  @click="startTranslation()"
                >
                  <template #icon><n-icon><play-outline /></n-icon></template>
                  {{ t('footer.translate', { n: counts.ready }) }}
                </n-button>
                <n-button
                  v-else-if="primaryKind === 'retry'"
                  type="primary"
                  size="large"
                  :disabled="!canStartTranslation"
                  @click="retryFailures(failedPaths)"
                >
                  <template #icon><n-icon><refresh-outline /></n-icon></template>
                  {{ t('footer.retry', { n: failedPaths.length }) }}
                </n-button>
                <n-button
                  v-else-if="primaryKind === 'busy'"
                  type="primary"
                  size="large"
                  loading
                  disabled
                >
                  {{ t('footer.busy', {
                    file: activeFileName,
                    current: Math.min(currentFileIndex + 1, batchPaths.length),
                    total: batchPaths.length
                  }) }}
                </n-button>
                <n-button
                  v-else-if="primaryKind === 'setup'"
                  size="large"
                  @click="showSettings = true"
                >
                  <template #icon><n-icon><settings-outline /></n-icon></template>
                  {{ setupLabel }}
                </n-button>
              </div>
            </footer>

            <div v-if="isDragging" class="drop-overlay">
              <div class="drop-overlay-box">
                <n-icon size="40"><cloud-upload-outline /></n-icon>
                <p>{{ t('app.dropHint') }}</p>
              </div>
            </div>
          </div>

          <SettingsModal v-model:show="showSettings" ref="settingsRef" />

          <div v-if="showLanguageSetup" class="setup-overlay" role="dialog" aria-modal="true">
            <section class="setup-card" :aria-label="t('setup.title')">
              <div class="setup-body">
                <p class="setup-eyebrow">{{ t('setup.eyebrow') }}</p>
                <h2 class="setup-title">{{ t('setup.title') }}</h2>
                <p class="setup-description">{{ t('setup.description') }}</p>
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
                <n-button type="primary" block size="large" @click="completeLanguageSetup">
                  {{ t('setup.continue') }}
                </n-button>
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
  NSelect,
  NCheckbox,
  NCollapse,
  NCollapseItem,
  NInput,
  NProgress,
  NAlert,
  NPopover,
  NDropdown
} from 'naive-ui'
import {
  SettingsOutline,
  LanguageOutline,
  ColorPaletteOutline,
  FolderOpenOutline,
  TrashOutline,
  ShieldCheckmarkOutline,
  PlayOutline,
  RefreshOutline,
  FilmOutline,
  CloudUploadOutline,
  AddOutline
} from '@vicons/ionicons5'
import { getCurrentWindow, type DragDropEvent } from '@tauri-apps/api/window'
import { scanFolderForVideos } from './api/animesubs'
import { hasUsableApiConfig, sharedLanguageOptions } from './config/settings'
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
import { useQueueStates } from './composables/useQueueStates'
import { useContextProfile } from './composables/useContextProfile'
import type { BackupInfo, ContextProfile, SelectedFile } from './types/domain'
import type { ThemeName } from './themes/themes'
import QueueRow from './components/QueueRow.vue'
import LogDrawer from './components/LogDrawer.vue'
import EmptyState from './components/EmptyState.vue'

const SettingsModal = defineAsyncComponent(() => import('./components/SettingsModal.vue'))
const ContextExtractionPanel = defineAsyncComponent(() => import('./components/ContextExtractionPanel.vue'))

const { t } = useI18n()
const { themeName, theme, themeOverrides, themeOptions, setTheme: applyTheme } = useAppTheme()

const showSettings = ref(false)
const showLanguageSetup = ref(false)
const showLanguageMenu = ref(false)
const setupLanguage = ref<InterfaceLocale>(defaultInterfaceLanguage)
const settingsRef = ref<SettingsModalExpose | null>(null)
const expandedRows = ref<Record<string, boolean>>({})

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

const themeMenuOptions = computed(() =>
  themeOptions.value.map(option => ({
    label: `${t(option.label)} · ${option.mode === 'dark' ? '☾' : '☀'}`,
    key: option.value,
    disabled: option.value === themeName.value
  }))
)

const setTheme = (name: string) => applyTheme(name as ThemeName)

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
  removeFile,
  clearFiles: clearSelectedFiles,
  extractSubtitle,
  backupSubtitle,
  restoreBackup,
  deleteBackup
} = useVideoFiles(getSettings, t)

const providerLabel = computed(() => cachedSettings.value?.provider || t('app.unconfigured'))
const modelLabel = computed(() => cachedSettings.value?.selectedModel || t('app.noModel'))
const providerReady = computed(() => hasUsableApiConfig(cachedSettings.value))
const ffmpegOk = computed(() => ffmpegStatus.value?.success === true)
const engineReady = computed(() => providerReady.value && ffmpegOk.value)
const ffmpegStatusLabel = computed(() => {
  if (!ffmpegStatus.value) return t('app.checking')
  return ffmpegStatus.value.success ? t('app.online') : t('app.missing')
})

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
  currentFileIndex,
  batchPaths,
  fileOutcomes,
  activeFilePath,
  activeFileName,
  activeFilePhase,
  activeFileLines,
  activeFilePaths,
  canStartTranslation,
  resetProgress,
  clearOutcome,
  clearAllOutcomes,
  startTranslation,
  retryFailures
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

const { getRowState, queuePosition, counts, failedPaths } = useQueueStates({
  selectedFiles,
  isTranslating,
  batchPaths,
  currentFileIndex,
  fileOutcomes,
  activeFilePaths
})

const primaryKind = computed<'translate' | 'retry' | 'busy' | 'setup' | 'hidden'>(() => {
  if (isTranslating.value) return 'busy'
  if (selectedFiles.value.length === 0) return 'hidden'
  if (!providerReady.value || !ffmpegOk.value) return 'setup'
  if (failedPaths.value.length > 0) return 'retry'
  if (counts.value.ready > 0) return 'translate'
  return 'hidden'
})

const setupLabel = computed(() =>
  !providerReady.value ? t('footer.disabledProvider') : t('footer.disabledFfmpeg')
)

const footerReason = computed(() => {
  if (primaryKind.value === 'setup') return ''
  if (selectedFiles.value.length === 0) return t('footer.disabledAdd')
  return ''
})

const canExtractContext = computed(() => {
  const settings = cachedSettings.value
  return Boolean(settings?.selectedModel) && selectedFiles.value.some(
    f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0
  )
})

const toggleRow = (path: string) => {
  expandedRows.value = { ...expandedRows.value, [path]: !expandedRows.value[path] }
}

const onRemoveFile = (index: number, path: string) => {
  removeFile(index)
  clearOutcome(path)
  const { [path]: _removed, ...rest } = expandedRows.value
  expandedRows.value = rest
}

const onRetryFile = (file: SelectedFile) => {
  if (isTranslating.value) return
  void startTranslation([file.path])
}

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

let autoExtractScheduled = false
const maybeAutoExtractContext = () => {
  if (autoExtractScheduled || isExtractingContext.value || isTranslating.value) return
  const settings = getSettings()
  if (!settings?.autoExtractContext) return
  if (contextProfile.value) return
  const file = selectedFiles.value.find(
    f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0 && !f.loading
  )
  if (!file) return
  if (!hasUsableApiConfig(settings)) return
  autoExtractScheduled = true
  void onExtractContext().finally(() => { autoExtractScheduled = false })
}

watch(selectedFiles, maybeAutoExtractContext, { deep: true })

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
  clearAllOutcomes()
  expandedRows.value = {}
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

const preventDefaults = (e: Event) => {
  e.preventDefault()
}

let cleanupDragDrop: (() => void) | null = null

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
  outline: 2px solid var(--focus-ring);
  outline-offset: 2px;
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
  background: var(--bg-base);
  overflow: hidden;
}

.app-shell::before {
  content: '';
  position: absolute;
  inset: 0;
  background-image: var(--theme-bg-image);
  background-size: cover;
  background-position: center;
  opacity: var(--theme-bg-opacity);
  pointer-events: none;
  z-index: 0;
}

.app-shell > * {
  position: relative;
  z-index: 1;
}

/* ---- Header ---- */
.app-header {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 0 16px;
  height: 54px;
  border-bottom: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  -webkit-app-region: drag;
  flex-shrink: 0;
}

.app-header :deep(button),
.app-header :deep(.engine-chip) {
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
  width: 30px;
  height: 30px;
  border-radius: var(--radius-sm);
  background: var(--accent);
  color: var(--accent-contrast);
}

.brand-eyebrow {
  margin: 0;
  font-size: 10px;
  color: var(--text-muted);
  letter-spacing: 0.05em;
}

.brand-text h1 {
  margin: 0;
  font-size: 15px;
  font-weight: 800;
  color: var(--text-primary);
  letter-spacing: -0.02em;
  line-height: 1.1;
}

.engine-chip {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: 999px;
  background: var(--bg-inset);
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: border-color 120ms ease;
}

.engine-chip:hover {
  border-color: var(--border-strong);
}

.engine-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--warning);
}

.engine-chip.ready .engine-dot {
  background: var(--success);
}

.engine-popover {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 220px;
}

.engine-row {
  font-size: 12.5px;
}

.engine-label {
  color: var(--text-secondary);
}

.engine-error {
  color: var(--danger);
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: auto;
}

.language-menu-wrap {
  position: relative;
}

.language-menu {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  min-width: 180px;
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-pop);
  padding: 6px;
  z-index: 50;
}

.language-menu-item {
  display: block;
  width: 100%;
  padding: 8px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  text-align: left;
  cursor: pointer;
}

.language-menu-item:hover {
  background: var(--hover-overlay);
  color: var(--text-primary);
}

.language-menu-item.active {
  color: var(--accent);
  font-weight: 700;
}

/* ---- Main ---- */
.app-main {
  flex: 1;
  display: flex;
  min-height: 0;
}

.workspace {
  flex: 1;
  display: flex;
  gap: 16px;
  padding: 16px;
  min-height: 0;
}

.queue-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.queue-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-subtle);
  flex-wrap: wrap;
}

.queue-toolbar-text {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.queue-title {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: var(--text-primary);
}

.queue-hint {
  display: flex;
  align-items: center;
  gap: 5px;
  margin: 0;
  font-size: 11.5px;
  color: var(--text-muted);
}

.queue-toolbar-actions {
  display: flex;
  align-items: center;
  gap: 6px;
}

.ffmpeg-alert {
  margin: 12px 16px 0;
}

.queue-scroll {
  flex: 1;
  min-height: 0;
}

.queue-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px 16px 16px;
}

/* ---- Side panel ---- */
.side-panel {
  width: 320px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 14px;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: 16px;
  overflow-y: auto;
}

.side-title {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: var(--text-primary);
}

.side-field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.side-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--text-secondary);
}

.side-collapse {
  margin-top: 4px;
}

.collapse-header {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1px;
}

.collapse-hint {
  font-size: 11px;
  font-weight: 400;
  color: var(--text-muted);
}

/* ---- Footer ---- */
.app-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 10px 16px;
  border-top: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  flex-shrink: 0;
}

.footer-stats {
  display: flex;
  align-items: center;
  gap: 14px;
  font-size: 12.5px;
  color: var(--text-secondary);
  min-width: 0;
}

.stat-accent { color: var(--accent); font-weight: 600; }
.stat-success { color: var(--success); font-weight: 600; }
.stat-danger { color: var(--danger); font-weight: 600; }

.footer-reason {
  color: var(--text-muted);
}

.footer-action {
  display: flex;
  align-items: center;
  gap: 14px;
  flex-shrink: 0;
}

.footer-progress {
  width: 180px;
}

/* ---- Drop overlay ---- */
.drop-overlay {
  position: absolute;
  inset: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--bg-base) 72%, transparent);
  backdrop-filter: blur(3px);
  pointer-events: none;
}

.drop-overlay-box {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 32px 48px;
  border: 2px dashed var(--accent);
  border-radius: var(--radius-lg);
  color: var(--accent);
  font-size: 15px;
  font-weight: 700;
}

.drop-overlay-box p {
  margin: 0;
}

/* ---- Language setup overlay ---- */
.setup-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  background: color-mix(in srgb, var(--bg-base) 82%, transparent);
  backdrop-filter: blur(4px);
}

.setup-card {
  width: min(460px, calc(100vw - 32px));
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  box-shadow: var(--shadow-pop);
  padding: 28px;
}

.setup-eyebrow {
  margin: 0 0 4px;
  font-size: 11px;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.setup-title {
  margin: 0 0 8px;
  font-size: 20px;
  font-weight: 800;
  color: var(--text-primary);
}

.setup-description {
  margin: 0 0 20px;
  font-size: 13.5px;
  line-height: 1.6;
  color: var(--text-secondary);
}

.language-choice-grid {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 20px;
}

.language-choice {
  padding: 12px 16px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-inset);
  color: var(--text-primary);
  font-size: 14px;
  text-align: left;
  cursor: pointer;
  transition: border-color 120ms ease, background 120ms ease;
}

.language-choice:hover {
  border-color: var(--border-strong);
}

.language-choice.active {
  border-color: var(--accent);
  background: var(--accent-soft);
  font-weight: 700;
}
</style>
