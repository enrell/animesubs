<template>
  <n-config-provider :theme="theme" :theme-overrides="themeOverrides">
    <n-message-provider>
      <n-notification-provider>
        <n-dialog-provider>
          <div class="app-shell" :class="{ 'is-dragging': isDragging }">
            <header class="wired-header">
              <div class="identity-block">
                <div>
                  <p class="eyebrow">animesubs://wired</p>
                  <h1>subtitle protocol</h1>
                </div>
              </div>

              <div class="status-strip" aria-label="Runtime status">
                <span class="status-pill">provider {{ providerLabel }}</span>
                <span class="status-pill truncate">model {{ modelLabel }}</span>
                <span class="status-pill" :class="ffmpegStatusClass">ffmpeg {{ ffmpegStatusLabel }}</span>
              </div>

              <div class="header-actions">
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button quaternary circle class="icon-button" @click="toggleTheme">
                      <template #icon>
                        <n-icon size="18">
                          <sunny-outline v-if="isDark" />
                          <moon-outline v-else />
                        </n-icon>
                      </template>
                    </n-button>
                  </template>
                  {{ isDark ? 'Light mode' : 'Dark mode' }}
                </n-tooltip>
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button quaternary circle class="icon-button" @click="showSettings = true">
                      <template #icon>
                        <n-icon size="18"><settings-outline /></n-icon>
                      </template>
                    </n-button>
                  </template>
                  Settings
                </n-tooltip>
              </div>
            </header>

            <main class="wired-main">
              <section
                class="hero-port"
                :class="{ 'drop-zone-active': isDragging }"
                @dragover.prevent="isDragging = true"
                @dragleave.prevent="isDragging = false"
                @drop.prevent="handleDrop"
              >
                <div class="hero-copy">
                  <p class="terminal-line">connect_media_packet</p>
                  <h2>Drop video files into the Wired.</h2>
                  <p>
                    Extract, translate, backup, and embed subtitle tracks without leaving the node.
                  </p>
                  <div class="hero-actions">
                    <n-button type="primary" size="large" class="primary-command" @click="selectFiles" :loading="loadingFiles">
                      <template #icon><n-icon><document-outline /></n-icon></template>
                      SELECT FILES
                    </n-button>
                    <n-button size="large" class="secondary-command" @click="selectFolder" :loading="loadingFiles">
                      <template #icon><n-icon><folder-open-outline /></n-icon></template>
                      SCAN FOLDER
                    </n-button>
                  </div>
                </div>

              </section>

              <n-alert
                v-if="ffmpegStatus && !ffmpegStatus.success"
                type="warning"
                title="FFmpeg signal missing"
                closable
                class="wired-alert"
              >
                FFmpeg is required for subtitle extraction. Install FFmpeg or configure its path in Settings.
              </n-alert>

              <section v-if="selectedFiles.length > 0" class="workspace-grid">
                <div class="queue-panel wired-panel">
                  <div class="panel-heading">
                    <div>
                      <p class="eyebrow">media queue</p>
                      <h3>{{ selectedFiles.length }} packets attached</h3>
                    </div>
                    <n-button text type="error" class="clear-command" @click="clearFiles">
                      <template #icon><n-icon><trash-outline /></n-icon></template>
                      CLEAR
                    </n-button>
                  </div>

                  <div class="queue-stats">
                    <div><strong>{{ readyFileCount }}</strong><span>ready</span></div>
                    <div><strong>{{ totalSubtitleTracks }}</strong><span>tracks</span></div>
                    <div><strong>{{ totalBackups }}</strong><span>backups</span></div>
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
                              {{ file.videoInfo.subtitle_tracks.length }} subs
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
                              <span>{{ track.title || `Track ${track.index}` }}</span>
                              <span class="track-codec">{{ track.codec }}</span>
                              <span v-if="track.default" class="track-flag">default</span>
                              <span v-if="track.forced" class="track-flag warn">forced</span>
                            </div>
                            <div class="track-actions">
                              <n-tooltip trigger="hover">
                                <template #trigger>
                                  <n-button size="tiny" quaternary @click="extractSubtitle(file, track.index)" :loading="extractingSubtitle === file.path">
                                    <template #icon><n-icon><download-outline /></n-icon></template>
                                  </n-button>
                                </template>
                                Extract subtitle
                              </n-tooltip>
                              <n-tooltip trigger="hover">
                                <template #trigger>
                                  <n-button size="tiny" quaternary type="success" @click="backupSubtitle(file, track.index)" :loading="backingUp === file.path">
                                    <template #icon><n-icon><shield-checkmark-outline /></n-icon></template>
                                  </n-button>
                                </template>
                                Backup subtitle
                              </n-tooltip>
                            </div>
                          </div>
                        </div>

                        <div v-else-if="file.videoInfo && file.videoInfo.subtitle_tracks.length === 0" class="no-subs-warning">
                          <n-icon size="16"><information-circle-outline /></n-icon>
                          <span>No subtitle tracks found</span>
                        </div>

                        <div v-if="file.error" class="file-error">
                          <n-text type="error" depth="3">{{ file.error }}</n-text>
                        </div>

                        <div v-if="file.backups.length > 0" class="backups-section">
                          <n-divider class="wired-divider">Backups</n-divider>
                          <div v-for="backup in file.backups" :key="backup.backup_path" class="backup-item">
                            <span>track {{ backup.track_index }} / {{ backup.format }} / {{ backup.created_at }}</span>
                            <div class="backup-actions">
                              <n-popconfirm @positive-click="restoreBackup(file, backup)">
                                <template #trigger>
                                  <n-button size="tiny" quaternary type="warning">
                                    <template #icon><n-icon><arrow-undo-outline /></n-icon></template>
                                  </n-button>
                                </template>
                                Restore this backup? This will replace the current subtitle track.
                              </n-popconfirm>
                              <n-popconfirm @positive-click="deleteBackup(file, backup)">
                                <template #trigger>
                                  <n-button size="tiny" quaternary type="error">
                                    <template #icon><n-icon><trash-outline /></n-icon></template>
                                  </n-button>
                                </template>
                                Delete this backup?
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
                      <p class="eyebrow">translation protocol</p>
                      <h3>{{ targetLanguageLabel }}</h3>
                    </div>
                    <span class="status-dot" :class="{ online: canStartTranslation }"></span>
                  </div>

                  <div class="protocol-form">
                    <n-form-item label="Target Language">
                      <n-input v-model:value="targetLanguageModel" placeholder="e.g. pt, en, es, ja" />
                    </n-form-item>
                    <n-form-item label="Subtitle Track">
                      <n-select v-model:value="translationOptions.subtitleTrack" :options="subtitleTrackOptions" placeholder="Auto-detect first available" />
                    </n-form-item>

                    <div class="switch-stack">
                      <n-checkbox v-model:checked="translationOptions.embedSubtitles">
                        <n-space align="center" :size="4">
                          <n-icon><layers-outline /></n-icon>
                          Embed translated subtitles
                        </n-space>
                      </n-checkbox>
                      <n-checkbox v-model:checked="translationOptions.useMkvmerge" :disabled="!translationOptions.embedSubtitles">
                        <n-space align="center" :size="4">
                          <n-icon><layers-outline /></n-icon>
                          Route through mkvmerge
                        </n-space>
                      </n-checkbox>
                    </div>

                    <n-collapse class="wired-collapse">
                      <n-collapse-item title="Advanced signal controls" name="advanced">
                        <n-grid :cols="3" :x-gap="12" :y-gap="12" responsive="screen">
                          <n-gi>
                            <n-form-item label="Batch">
                              <n-input-number v-model:value="translationOptions.batchSize" :min="1" :max="1000" />
                            </n-form-item>
                          </n-gi>
                          <n-gi>
                            <n-form-item label="Parallel">
                              <n-input-number v-model:value="translationOptions.concurrency" :min="1" :max="10" />
                            </n-form-item>
                          </n-gi>
                          <n-gi>
                            <n-form-item label="Delay">
                              <n-input-number v-model:value="translationOptions.requestDelay" :min="0" :max="5000" :step="100" />
                            </n-form-item>
                          </n-gi>
                        </n-grid>
                        <n-form-item label="Custom Prompt">
                          <n-input v-model:value="translationOptions.customPrompt" type="textarea" placeholder="Add temporary protocol instructions..." :rows="4" />
                        </n-form-item>
                      </n-collapse-item>
                    </n-collapse>
                  </div>

                  <div class="execute-block">
                    <n-button type="primary" size="large" block class="execute-command" :loading="isTranslating" :disabled="!canStartTranslation" @click="startTranslation">
                      <template #icon><n-icon><play-outline /></n-icon></template>
                      {{ isTranslating ? 'TRANSLATING SIGNAL...' : 'INITIATE TRANSLATION' }}
                    </n-button>
                    <p v-if="!canStartTranslation" class="disabled-hint">Attach media with subtitle tracks and verify provider/FFmpeg settings.</p>
                  </div>

                  <div v-if="isTranslating || translationProgress > 0" class="progress-console">
                    <div class="progress-head">
                      <span>sync {{ Math.round(translationProgress) }}%</span>
                      <span>{{ estimatedTime }}</span>
                    </div>
                    <n-progress type="line" :percentage="translationProgress" :status="translationProgress === 100 ? 'success' : 'default'" :show-indicator="false" />
                    <p class="progress-status">{{ currentStatus || 'awaiting packet response...' }}</p>
                  </div>
                </aside>
              </section>
            </main>
          </div>

          <SettingsModal v-model:show="showSettings" ref="settingsRef" />
        </n-dialog-provider>
      </n-notification-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, onUnmounted, ref } from 'vue'
import {
  NConfigProvider,
  NMessageProvider,
  NNotificationProvider,
  NDialogProvider,
  NButton,
  NIcon,
  NSpace,
  NTooltip,
  NScrollbar,
  NFormItem,
  NSelect,
  NCheckbox,
  NCollapse,
  NCollapseItem,
  NGrid,
  NGi,
  NInputNumber,
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
import { useAppTheme } from './composables/useAppTheme'
import { useSettingsState, type SettingsModalExpose } from './composables/useSettingsState'
import { useTranslationOptions } from './composables/useTranslationOptions'
import { useVideoFiles } from './composables/useVideoFiles'
import { useTranslationJob } from './composables/useTranslationJob'

const SettingsModal = defineAsyncComponent(() => import('./components/SettingsModal.vue'))

const { isDark, theme, themeOverrides, toggleTheme } = useAppTheme()

const showSettings = ref(false)
const settingsRef = ref<SettingsModalExpose | null>(null)

const {
  cachedSettings,
  ffmpegStatus,
  loadCachedSettings,
  getSettings,
  targetLanguageModel,
  checkFFmpeg
} = useSettingsState(showSettings, settingsRef)

const {
  translationOptions,
  loadTranslationOptions
} = useTranslationOptions()

const languageOptions = sharedLanguageOptions.filter(option => option.value)

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
} = useVideoFiles(getSettings)

const providerLabel = computed(() => cachedSettings.value?.provider || 'unconfigured')
const modelLabel = computed(() => cachedSettings.value?.selectedModel || 'no-model')
const targetLanguageLabel = computed(() => {
  const target = cachedSettings.value?.targetLanguage || targetLanguageModel.value
  return languageOptions.find(option => option.value === target)?.label || target || 'Target unknown'
})
const readyFileCount = computed(() => selectedFiles.value.filter(file => file.videoInfo && file.videoInfo.subtitle_tracks.length > 0).length)
const totalSubtitleTracks = computed(() => selectedFiles.value.reduce((total, file) => total + (file.videoInfo?.subtitle_tracks.length || 0), 0))
const totalBackups = computed(() => selectedFiles.value.reduce((total, file) => total + file.backups.length, 0))
const ffmpegStatusLabel = computed(() => {
  if (!ffmpegStatus.value) return 'checking'
  return ffmpegStatus.value.success ? 'online' : 'missing'
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
  getSettings
})

const clearFiles = () => {
  clearSelectedFiles()
  resetProgress()
}

let cleanupDragDrop: (() => void) | null = null

const preventDefaults = (e: Event) => {
  e.preventDefault()
  e.stopPropagation()
}

onMounted(async () => {
  await loadCachedSettings()
  loadTranslationOptions()
  await checkFFmpeg()

  window.addEventListener('dragover', preventDefaults)
  window.addEventListener('drop', preventDefaults)

  const unlistenDragDrop = await getCurrentWindow().onDragDropEvent(async (event) => {
    const payload = event.payload as DragDropEvent
    if (payload.type === 'enter') {
      isDragging.value = true
      return
    }
    if (payload.type === 'leave') {
      isDragging.value = false
      return
    }
    if (payload.type === 'drop') {
      isDragging.value = false
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
  --wired-black: #030303;
  --wired-void: #070606;
  --wired-panel: #121013;
  --wired-panel-2: #1c151d;
  --wired-panel-3: #241a23;
  --wired-paper: #c8bd98;
  --wired-paper-bright: #e0d4a8;
  --wired-muted: #8d8064;
  --wired-faint: #5d5342;
  --wired-pink: #c99a86;
  --wired-red: #b54438;
  --wired-red-dark: #4d1716;
  --wired-border: rgba(200, 189, 152, 0.22);
  --wired-border-strong: rgba(224, 212, 168, 0.42);
  --wired-glow: rgba(201, 154, 134, 0.18);
  --wired-shadow: 0 24px 80px rgba(0, 0, 0, 0.64);
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
  background:
    radial-gradient(circle at 14% 18%, rgba(181, 68, 56, 0.18), transparent 26rem),
    radial-gradient(circle at 88% 8%, rgba(201, 154, 134, 0.1), transparent 22rem),
    linear-gradient(135deg, var(--wired-black), var(--wired-void) 46%, #0d080b);
  isolation: isolate;
}

.app-shell::before,
.app-shell::after {
  position: absolute;
  inset: 0;
  z-index: -1;
  pointer-events: none;
  content: "";
}

.app-shell::before {
  opacity: 0.32;
  background-image:
    radial-gradient(circle, rgba(201, 154, 134, 0.18) 1px, transparent 1px);
  background-size: 4px 4px;
}

.app-shell::after {
  opacity: 0.18;
  background: repeating-linear-gradient(to bottom, transparent 0 3px, rgba(224, 212, 168, 0.09) 3px 4px);
  mix-blend-mode: screen;
}

.wired-header {
  display: flex;
  align-items: center;
  gap: 18px;
  padding: 12px 20px;
  border-bottom: 1px solid var(--wired-border);
  background: rgba(3, 3, 3, 0.74);
  -webkit-app-region: drag;
}

.wired-header :deep(button) {
  -webkit-app-region: no-drag;
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
  background: rgba(18, 16, 19, 0.82);
}

.status-pill.online {
  color: var(--wired-paper-bright);
  border-color: rgba(200, 189, 152, 0.5);
}

.status-pill.offline {
  color: var(--wired-pink);
  border-color: rgba(181, 68, 56, 0.56);
}

.header-actions {
  display: flex;
  gap: 6px;
  margin-left: auto;
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
  background: repeating-linear-gradient(
    135deg,
    rgba(200, 189, 152, 0.02) 0 1px,
    transparent 1px 12px
  );
  transition: border-color 160ms ease, box-shadow 160ms ease;
}

.hero-port.drop-zone-active {
  border-color: var(--wired-paper-bright);
  box-shadow: 0 0 0 1px rgba(224, 212, 168, 0.2), inset 0 0 60px rgba(201, 154, 134, 0.06);
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
  background:
    linear-gradient(180deg, rgba(28, 21, 29, 0.9), rgba(12, 9, 12, 0.94)),
    radial-gradient(circle at 20% 0%, rgba(201, 154, 134, 0.11), transparent 18rem);
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
  border: 1px solid rgba(200, 189, 152, 0.14);
  background: rgba(3, 3, 3, 0.34);
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
  padding: 14px;
  border: 1px solid rgba(200, 189, 152, 0.14);
  background:
    linear-gradient(90deg, rgba(3, 3, 3, 0.46), rgba(28, 21, 29, 0.36)),
    repeating-linear-gradient(135deg, rgba(200, 189, 152, 0.035) 0 1px, transparent 1px 8px);
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
  border: 1px solid rgba(200, 189, 152, 0.1);
  background: rgba(3, 3, 3, 0.34);
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
  background: rgba(200, 189, 152, 0.13);
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
  border: 1px solid rgba(201, 154, 134, 0.14);
  background: rgba(181, 68, 56, 0.08);
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
  box-shadow: 0 0 18px rgba(181, 68, 56, 0.36);
}

.status-dot.online {
  border-color: var(--wired-paper-bright);
  background: var(--wired-paper);
  box-shadow: 0 0 20px rgba(224, 212, 168, 0.26);
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
  border: 1px solid rgba(200, 189, 152, 0.12);
  background: rgba(3, 3, 3, 0.32);
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
  border: 1px solid rgba(200, 189, 152, 0.16);
  background: #050505;
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
  box-shadow: 6px 6px 0 var(--wired-red-dark);
}

:deep(.n-button--primary-type:hover) {
  background: var(--wired-paper-bright) !important;
  border-color: var(--wired-paper-bright) !important;
}

.secondary-command,
.icon-button {
  color: var(--wired-paper) !important;
  border: 1px solid var(--wired-border) !important;
  background: rgba(3, 3, 3, 0.3) !important;
}

:deep(.n-base-selection),
:deep(.n-input),
:deep(.n-input-number),
:deep(.n-input-number .n-input) {
  border-radius: 0 !important;
  background: rgba(3, 3, 3, 0.42) !important;
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
  background: linear-gradient(90deg, var(--wired-red), var(--wired-paper)) !important;
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
