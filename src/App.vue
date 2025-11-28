<template>
  <n-config-provider :theme="theme" :theme-overrides="themeOverrides">
    <n-message-provider>
      <n-notification-provider>
        <n-dialog-provider>
          <div class="app-container">
            <!-- Header -->
            <header class="app-header">
              <div class="header-left">
                <div class="logo">
                  <n-icon size="28" color="var(--primary-color)">
                    <language-outline />
                  </n-icon>
                  <span class="logo-text">AnimeSubs</span>
                </div>
              </div>
              <div class="header-right">
                <n-tooltip trigger="hover">
                  <template #trigger>
                    <n-button quaternary circle @click="toggleTheme">
                      <template #icon>
                        <n-icon size="20">
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
                    <n-button quaternary circle @click="showSettings = true">
                      <template #icon>
                        <n-icon size="20">
                          <settings-outline />
                        </n-icon>
                      </template>
                    </n-button>
                  </template>
                  Settings
                </n-tooltip>
              </div>
            </header>

            <!-- Main Content -->
            <main class="app-main">
              <div class="main-content">
                <!-- FFmpeg Status Alert -->
                <n-alert 
                  v-if="ffmpegStatus && !ffmpegStatus.success" 
                  type="warning" 
                  title="FFmpeg Not Found"
                  closable
                >
                  FFmpeg is required for subtitle extraction. Please install FFmpeg or configure its path in Settings.
                </n-alert>

                <!-- Drop Zone -->
                <n-card 
                  class="drop-zone" 
                  :class="{ 'drop-zone-active': isDragging }"
                  @dragover.prevent="isDragging = true"
                  @dragleave.prevent="isDragging = false"
                  @drop.prevent="handleDrop"
                >
                  <div class="drop-zone-content">
                    <img src="/icon.png" alt="AnimeSubs" class="drop-icon" />
                    <h2>Drop video files or folder here</h2>
                    <p>Supports MKV, MP4, WebM, AVI, and other FFmpeg formats</p>
                    <n-space>
                      <n-button type="primary" size="large" @click="selectFiles" :loading="loadingFiles">
                        <template #icon>
                          <n-icon><document-outline /></n-icon>
                        </template>
                        Select Files
                      </n-button>
                      <n-button size="large" @click="selectFolder" :loading="loadingFiles">
                        <template #icon>
                          <n-icon><folder-open-outline /></n-icon>
                        </template>
                        Select Folder
                      </n-button>
                    </n-space>
                  </div>
                </n-card>

                <!-- Selected Files List -->
                <n-card v-if="selectedFiles.length > 0" class="files-card">
                  <template #header>
                    <n-space align="center" justify="space-between" style="width: 100%">
                      <span>Selected Files ({{ selectedFiles.length }})</span>
                      <n-button text type="error" @click="clearFiles">
                        <template #icon>
                          <n-icon><trash-outline /></n-icon>
                        </template>
                        Clear All
                      </n-button>
                    </n-space>
                  </template>
                  
                  <n-scrollbar style="max-height: 400px">
                    <div class="file-list">
                      <div v-for="(file, index) in selectedFiles" :key="file.path" class="file-item">
                        <div class="file-header">
                          <n-space align="center">
                            <n-icon size="20" color="var(--primary-color)">
                              <videocam-outline />
                            </n-icon>
                            <span class="file-name">{{ file.name }}</span>
                            <n-spin v-if="file.loading" size="small" />
                            <n-tag v-if="file.videoInfo" size="small" :bordered="false" type="info">
                              {{ file.videoInfo.subtitle_tracks.length }} subs
                            </n-tag>
                            <n-tag v-if="file.backups.length > 0" size="small" :bordered="false" type="success">
                              {{ file.backups.length }} backups
                            </n-tag>
                          </n-space>
                          <n-button text type="error" @click="removeFile(index)">
                            <template #icon>
                              <n-icon><close-outline /></n-icon>
                            </template>
                          </n-button>
                        </div>
                        
                        <!-- Subtitle Tracks -->
                        <div v-if="file.videoInfo && file.videoInfo.subtitle_tracks.length > 0" class="subtitle-tracks">
                          <div 
                            v-for="track in file.videoInfo.subtitle_tracks" 
                            :key="track.index" 
                            class="subtitle-track"
                          >
                            <n-space align="center" justify="space-between" style="width: 100%">
                              <n-space align="center" :size="8">
                                <n-tag size="small" :type="track.default ? 'primary' : 'default'">
                                  {{ track.language || 'und' }}
                                </n-tag>
                                <span class="track-info">
                                  {{ track.title || `Track ${track.index}` }} 
                                  <span class="track-codec">({{ track.codec }})</span>
                                </span>
                                <n-tag v-if="track.forced" size="tiny" type="warning">Forced</n-tag>
                              </n-space>
                              <n-space :size="4">
                                <n-tooltip trigger="hover">
                                  <template #trigger>
                                    <n-button 
                                      size="tiny" 
                                      quaternary 
                                      @click="extractSubtitle(file, track.index)"
                                      :loading="extractingSubtitle === file.path"
                                    >
                                      <template #icon>
                                        <n-icon><download-outline /></n-icon>
                                      </template>
                                    </n-button>
                                  </template>
                                  Extract subtitle
                                </n-tooltip>
                                <n-tooltip trigger="hover">
                                  <template #trigger>
                                    <n-button 
                                      size="tiny" 
                                      quaternary 
                                      type="success"
                                      @click="backupSubtitle(file, track.index)"
                                      :loading="backingUp === file.path"
                                    >
                                      <template #icon>
                                        <n-icon><shield-checkmark-outline /></n-icon>
                                      </template>
                                    </n-button>
                                  </template>
                                  Backup subtitle
                                </n-tooltip>
                              </n-space>
                            </n-space>
                          </div>
                        </div>

                        <!-- No subtitles warning -->
                        <div v-else-if="file.videoInfo && file.videoInfo.subtitle_tracks.length === 0" class="no-subs-warning">
                          <n-icon size="16" color="var(--warning-color)">
                            <information-circle-outline />
                          </n-icon>
                          <span>No subtitle tracks found</span>
                        </div>

                        <!-- Error -->
                        <div v-if="file.error" class="file-error">
                          <n-text type="error" depth="3">{{ file.error }}</n-text>
                        </div>

                        <!-- Backups -->
                        <div v-if="file.backups.length > 0" class="backups-section">
                          <n-divider style="margin: 8px 0; font-size: 12px;">Backups</n-divider>
                          <div v-for="backup in file.backups" :key="backup.backup_path" class="backup-item">
                            <n-space align="center" justify="space-between" style="width: 100%">
                              <n-space align="center" :size="8">
                                <n-icon size="14" color="var(--success-color)">
                                  <shield-checkmark-outline />
                                </n-icon>
                                <span class="backup-name">Track {{ backup.track_index }} - {{ backup.created_at }}</span>
                                <n-tag size="tiny">{{ backup.format }}</n-tag>
                              </n-space>
                              <n-space :size="4">
                                <n-popconfirm @positive-click="restoreBackup(file, backup)">
                                  <template #trigger>
                                    <n-button size="tiny" quaternary type="warning">
                                      <template #icon>
                                        <n-icon><arrow-undo-outline /></n-icon>
                                      </template>
                                    </n-button>
                                  </template>
                                  Restore this backup? This will replace the current subtitle track.
                                </n-popconfirm>
                                <n-popconfirm @positive-click="deleteBackup(file, backup)">
                                  <template #trigger>
                                    <n-button size="tiny" quaternary type="error">
                                      <template #icon>
                                        <n-icon><trash-outline /></n-icon>
                                      </template>
                                    </n-button>
                                  </template>
                                  Delete this backup?
                                </n-popconfirm>
                              </n-space>
                            </n-space>
                          </div>
                        </div>
                      </div>
                    </div>
                  </n-scrollbar>
                </n-card>

                <!-- Options Card -->
                <n-card v-if="selectedFiles.length > 0" class="options-card">
                  <template #header>Translation Options</template>
                  
                  <n-space vertical size="large">
                    <n-grid :cols="2" :x-gap="24" :y-gap="16">
                      <n-gi>
                        <n-form-item label="Target Language">
                          <n-select
                            v-model:value="translationOptions.targetLanguage"
                            :options="languageOptions"
                          />
                        </n-form-item>
                      </n-gi>
                      <n-gi>
                        <n-form-item label="Subtitle Track">
                          <n-select
                            v-model:value="translationOptions.subtitleTrack"
                            :options="subtitleTrackOptions"
                            placeholder="Auto-detect (first available)"
                          />
                        </n-form-item>
                      </n-gi>
                    </n-grid>

                    <n-space>
                      <n-checkbox v-model:checked="translationOptions.embedSubtitles">
                        <n-space align="center" :size="4">
                          <n-icon><layers-outline /></n-icon>
                          Embed translated subtitles in video
                        </n-space>
                      </n-checkbox>
                      
                      <n-checkbox 
                        v-model:checked="translationOptions.useMkvmerge"
                        :disabled="!translationOptions.embedSubtitles"
                      >
                        <n-space align="center" :size="4">
                          <n-icon><layers-outline /></n-icon>
                          Use mkvmerge for embedding
                        </n-space>
                      </n-checkbox>
                    </n-space>

                    <n-collapse>
                      <n-collapse-item title="Advanced Options" name="advanced">
                        <n-grid :cols="2" :x-gap="24" :y-gap="16">
                          <n-gi>
                            <n-form-item label="Batch Size">
                              <n-input-number
                                v-model:value="translationOptions.batchSize"
                                :min="1"
                                :max="1000"
                                placeholder="Lines per API call"
                              />
                            </n-form-item>
                          </n-gi>
                          <n-gi>
                            <n-form-item label="Delay Between Requests (ms)">
                              <n-input-number
                                v-model:value="translationOptions.requestDelay"
                                :min="0"
                                :max="5000"
                                :step="100"
                              />
                            </n-form-item>
                          </n-gi>
                        </n-grid>
                        
                        <n-form-item label="Custom Prompt (Optional)">
                          <n-input
                            v-model:value="translationOptions.customPrompt"
                            type="textarea"
                            placeholder="Add custom instructions for the translation..."
                            :rows="3"
                          />
                        </n-form-item>
                      </n-collapse-item>
                    </n-collapse>
                  </n-space>
                </n-card>

                <!-- Action Button -->
                <div v-if="selectedFiles.length > 0" class="action-bar">
                  <n-button
                    type="primary"
                    size="large"
                    :loading="isTranslating"
                    :disabled="!canStartTranslation"
                    @click="startTranslation"
                  >
                    <template #icon>
                      <n-icon><play-outline /></n-icon>
                    </template>
                    {{ isTranslating ? 'Translating...' : 'Start Translation' }}
                  </n-button>
                </div>

                <!-- Progress Section -->
                <n-card v-if="isTranslating || translationProgress > 0" class="progress-card">
                  <template #header>
                    <n-space align="center">
                      <span>Translation Progress</span>
                    </n-space>
                  </template>
                  
                  <n-space vertical size="large">
                    <n-progress
                      type="line"
                      :percentage="translationProgress"
                      :status="translationProgress === 100 ? 'success' : 'default'"
                      :show-indicator="true"
                    />
                    
                    <n-space justify="space-between">
                      <span class="progress-status">{{ currentStatus }}</span>
                      <span class="progress-eta">{{ estimatedTime }}</span>
                    </n-space>
                  </n-space>
                </n-card>
              </div>
            </main>
          </div>

          <!-- Settings Modal -->
          <SettingsModal v-model:show="showSettings" ref="settingsRef" />
        </n-dialog-provider>
      </n-notification-provider>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, onUnmounted, watch, triggerRef } from 'vue'
import {
  NConfigProvider,
  NMessageProvider,
  NNotificationProvider,
  NDialogProvider,
  NButton,
  NCard,
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
  NText,
  darkTheme,
  type GlobalThemeOverrides
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
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
import { getCurrentWindow, type DragDropEvent } from '@tauri-apps/api/window'
import SettingsModal, { type Settings } from './components/SettingsModal.vue'

// Types
interface SubtitleTrack {
  index: number
  stream_index: number
  codec: string
  language: string | null
  title: string | null
  default: boolean
  forced: boolean
}

interface VideoInfo {
  path: string
  filename: string
  duration: number | null
  subtitle_tracks: SubtitleTrack[]
}

interface BackupInfo {
  original_path: string
  backup_path: string
  track_index: number
  format: string
  created_at: string
}

interface OperationResult {
  success: boolean
  message: string
  data: string | null
}

interface DialogLine {
  index: number
  start: string
  end: string
  text: string
  style: string | null
  name: string | null
}

interface SubtitleData {
  format: string
  line_count: number
  lines: DialogLine[]
  source_path: string | null
  ass_header: string | null
}

// Theme
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

const isDark = ref(getInitialTheme())
const theme = computed(() => isDark.value ? darkTheme : null)

const themeOverrides = computed<GlobalThemeOverrides>(() => ({
  common: {
    primaryColor: '#6b7cff',
    primaryColorHover: '#7f8dff',
    primaryColorPressed: '#5567e6',
    primaryColorSuppl: '#6b7cff',
    borderRadius: '8px',
    ...(isDark.value ? {
      bodyColor: '#0d1016',
      cardColor: '#121620',
      modalColor: '#0f121a',
      popoverColor: '#121620',
      tableColor: '#121620',
      inputColor: '#0c0f17',
      actionColor: '#182032',
      hoverColor: 'rgba(107, 124, 255, 0.12)',
      borderColor: '#1f2633',
      dividerColor: '#1f2633',
      textColor1: '#f5f7fb',
      textColor2: '#c4cad4',
      textColor3: '#8d95a6'
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

// Settings
const showSettings = ref(false)
const settingsRef = ref<InstanceType<typeof SettingsModal> | null>(null)
const cachedSettings = ref<Settings | null>(null)

// Load settings from localStorage
const loadCachedSettings = () => {
  try {
    const saved = localStorage.getItem('animesubs-settings')
    if (saved) {
      cachedSettings.value = JSON.parse(saved)
    }
  } catch (e) {
    console.error('Failed to load settings:', e)
  }
}

// Refresh cache when settings modal closes
watch(showSettings, (isOpen, wasOpen) => {
  if (wasOpen && !isOpen) {
    // Modal just closed, refresh settings cache
    loadCachedSettings()
  }
})

// FFmpeg status
const ffmpegStatus = ref<OperationResult | null>(null)

const checkFFmpeg = async () => {
  try {
    const settings = getSettings()
    ffmpegStatus.value = await invoke<OperationResult>('check_ffmpeg', {
      ffmpegPath: settings?.ffmpegPath || null
    })
  } catch (e) {
    ffmpegStatus.value = {
      success: false,
      message: `Error: ${e}`,
      data: null
    }
  }
}

onMounted(async () => {
  // Load settings first (synchronous)
  loadCachedSettings()
  // Load translation options
  loadTranslationOptions()
  // Then check FFmpeg with loaded settings
  await checkFFmpeg()

  const preventDefaults = (e: Event) => {
    e.preventDefault()
    e.stopPropagation()
  }

  window.addEventListener('dragover', preventDefaults)
  window.addEventListener('drop', preventDefaults)

  // Better drag/drop via Tauri window helper
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
          await addFiles(paths)
        } finally {
          loadingFiles.value = false
        }
      }
    }
  })

  onUnmounted(() => {
    unlistenDragDrop()
    window.removeEventListener('dragover', preventDefaults)
    window.removeEventListener('drop', preventDefaults)
  })
})

// File selection
interface SelectedFile {
  name: string
  path: string
  size: number
  videoInfo: VideoInfo | null
  backups: BackupInfo[]
  loading: boolean
  error: string | null
}

const selectedFiles = ref<SelectedFile[]>([])
const isDragging = ref(false)
const loadingFiles = ref(false)

const selectFiles = async () => {
  const selected = await open({
    multiple: true,
    filters: [{
      name: 'Video Files',
      extensions: ['mkv', 'mp4', 'webm', 'avi', 'mov', 'wmv', 'flv', 'm4v']
    }]
  })
  
  if (selected) {
    const files = Array.isArray(selected) ? selected : [selected]
    await addFiles(files)
  }
}

const selectFolder = async () => {
  const selected = await open({
    directory: true,
    multiple: false
  })
  
  if (selected) {
    loadingFiles.value = true
    try {
      const videos = await invoke<string[]>('scan_folder_for_videos', {
        folderPath: selected
      })
      await addFiles(videos)
    } catch (e) {
      console.error('Failed to scan folder:', e)
    } finally {
      loadingFiles.value = false
    }
  }
}

const addFiles = async (paths: string[]) => {
  const settings = getSettings()
  
  for (const path of paths) {
    // Skip if already added
    if (selectedFiles.value.some(f => f.path === path)) continue
    
    const file: SelectedFile = {
      name: path.split('/').pop() || path,
      path,
      size: 0,
      videoInfo: null,
      backups: [],
      loading: true,
      error: null
    }
    
    selectedFiles.value.push(file)
    // Force reactivity update after push
    triggerRef(selectedFiles)
    
    // Find the index we just pushed to for reactive updates
    const fileIndex = selectedFiles.value.length - 1
    
    // Load video info in background
    try {
      const videoInfo = await invoke<VideoInfo>('get_video_info', {
        videoPath: path,
        ffmpegPath: settings?.ffmpegPath || null
      })
      
      // Load backups
      const backups = await invoke<BackupInfo[]>('list_backups', {
        videoPath: path
      })
      
      // Update reactively by replacing the object
      selectedFiles.value[fileIndex] = {
        ...selectedFiles.value[fileIndex],
        videoInfo,
        backups,
        loading: false
      }
      // Force reactivity update
      triggerRef(selectedFiles)
    } catch (e) {
      // Update reactively by replacing the object
      selectedFiles.value[fileIndex] = {
        ...selectedFiles.value[fileIndex],
        error: `${e}`,
        loading: false
      }
      // Force reactivity update
      triggerRef(selectedFiles)
    }
  }
  
  updateSubtitleTrackOptions()
}

const handleDrop = async (e: DragEvent) => {
  e.preventDefault()
  isDragging.value = false
  const dt = e.dataTransfer
  if (!dt) return

  const paths: string[] = []
  if (dt.files?.length) {
    for (const f of Array.from(dt.files)) {
      const path = (f as any).path || f.name
      if (path) paths.push(path)
    }
  } else if (dt.items?.length) {
    for (const item of Array.from(dt.items)) {
      const f = item.getAsFile()
      if (f) {
        const path = (f as any).path || f.name
        if (path) paths.push(path)
      }
    }
  }

  if (paths.length > 0) {
    loadingFiles.value = true
    try {
      await addFiles(paths)
    } finally {
      loadingFiles.value = false
    }
  }
}

const removeFile = (index: number) => {
  selectedFiles.value.splice(index, 1)
  updateSubtitleTrackOptions()
}

const clearFiles = () => {
  selectedFiles.value = []
  subtitleTrackOptions.value = [{ label: 'Auto-detect (first available)', value: '' }]
}

// Translation options
const translationOptions = reactive({
  targetLanguage: 'en',
  subtitleTrack: '' as string,
  embedSubtitles: false,
  useMkvmerge: true,
  batchSize: 100,
  requestDelay: 500,
  customPrompt: ''
})

// Load translation options from localStorage
const loadTranslationOptions = () => {
  try {
    const saved = localStorage.getItem('animesubs-translation-options')
    if (saved) {
      const parsed = JSON.parse(saved)
      Object.assign(translationOptions, parsed)
    }
  } catch (e) {
    console.error('Failed to load translation options:', e)
  }
}

// Save translation options to localStorage
const saveTranslationOptions = () => {
  try {
    localStorage.setItem('animesubs-translation-options', JSON.stringify(translationOptions))
  } catch (e) {
    console.error('Failed to save translation options:', e)
  }
}

// Watch for changes and auto-save
watch(translationOptions, () => {
  saveTranslationOptions()
}, { deep: true })

const languageOptions = [
  { label: 'English', value: 'en' },
  { label: 'Japanese', value: 'ja' },
  { label: 'Chinese (Simplified)', value: 'zh-CN' },
  { label: 'Chinese (Traditional)', value: 'zh-TW' },
  { label: 'Korean', value: 'ko' },
  { label: 'Spanish', value: 'es' },
  { label: 'French', value: 'fr' },
  { label: 'German', value: 'de' },
  { label: 'Portuguese', value: 'pt' },
  { label: 'Russian', value: 'ru' },
  { label: 'Italian', value: 'it' },
  { label: 'Arabic', value: 'ar' }
]

const normalizeLanguageKey = (value?: string | null) => {
  if (!value) return ''
  return value.toLowerCase().replace(/_/g, '-').trim()
}

const ffmpegLanguageMap: Record<string, string> = {
  und: 'und',
  en: 'eng',
  eng: 'eng',
  'en-us': 'eng',
  ja: 'jpn',
  jpn: 'jpn',
  'zh-cn': 'zho',
  zh: 'zho',
  'zh-tw': 'zho',
  ko: 'kor',
  kor: 'kor',
  es: 'spa',
  spa: 'spa',
  fr: 'fra',
  fra: 'fra',
  de: 'deu',
  deu: 'deu',
  pt: 'por',
  'pt-br': 'por',
  por: 'por',
  ru: 'rus',
  rus: 'rus',
  it: 'ita',
  ita: 'ita',
  ar: 'ara',
  ara: 'ara'
}

const toFfmpegLangCode = (value?: string | null): string => {
  if (!value) return 'und'
  const normalized = normalizeLanguageKey(value)
  if (ffmpegLanguageMap[normalized]) return ffmpegLanguageMap[normalized]
  const base = normalized.split('-')[0]
  if (ffmpegLanguageMap[base]) return ffmpegLanguageMap[base]
  if (normalized.length === 3) return normalized
  if (base.length === 2 && ffmpegLanguageMap[base]) return ffmpegLanguageMap[base]
  if (base.length === 2) {
    return `${base}${base.slice(-1) || 'x'}${base.slice(-1) || 'x'}`.slice(0, 3)
  }
  return 'und'
}

const sanitizeLangCodeForFilename = (value?: string | null): string => {
  const cleaned = normalizeLanguageKey(value).replace(/[^a-z0-9-]/g, '')
  return cleaned || 'und'
}

const subtitleTrackOptions = ref([
  { label: 'Auto-detect (first available)', value: '' }
])

const updateSubtitleTrackOptions = () => {
  const options = [{ label: 'Auto-detect (first available)', value: '' }]
  
  // Collect all unique subtitle tracks from all files
  const tracks = new Map<string, string>()
  for (const file of selectedFiles.value) {
    if (file.videoInfo) {
      for (const track of file.videoInfo.subtitle_tracks) {
        const key = `${track.index}`
        const lang = track.language || 'und'
        const title = track.title || `Track ${track.index}`
        const label = `${title} (${lang}) - ${track.codec}`
        if (!tracks.has(key)) {
          tracks.set(key, label)
        }
      }
    }
  }
  
  tracks.forEach((label, value) => {
    options.push({ label, value })
  })
  
  subtitleTrackOptions.value = options
}

// Subtitle operations
const extractingSubtitle = ref<string | null>(null)
const backingUp = ref<string | null>(null)

const extractSubtitle = async (file: SelectedFile, trackIndex: number) => {
  if (!file.videoInfo) return
  
  extractingSubtitle.value = file.path
  const settings = getSettings()
  
  try {
    const result = await invoke<{ success: boolean; output_path: string | null; error: string | null }>('extract_subtitle', {
      videoPath: file.path,
      trackIndex,
      outputPath: null,
      format: settings?.outputFormat || 'srt',
      ffmpegPath: settings?.ffmpegPath || null
    })
    
    if (result.success) {
      console.log('Subtitle extracted to:', result.output_path)
    } else {
      console.error('Failed to extract subtitle:', result.error)
    }
  } catch (e) {
    console.error('Extract error:', e)
  } finally {
    extractingSubtitle.value = null
  }
}

const backupSubtitle = async (file: SelectedFile, trackIndex: number) => {
  if (!file.videoInfo) return
  
  backingUp.value = file.path
  const settings = getSettings()
  
  try {
    const backupInfo = await invoke<BackupInfo>('backup_subtitle', {
      videoPath: file.path,
      trackIndex,
      ffmpegPath: settings?.ffmpegPath || null
    })
    
    file.backups.push(backupInfo)
    console.log('Backup created:', backupInfo.backup_path)
  } catch (e) {
    console.error('Backup error:', e)
  } finally {
    backingUp.value = null
  }
}

const restoreBackup = async (file: SelectedFile, backup: BackupInfo) => {
  const settings = getSettings()
  
  try {
    const result = await invoke<OperationResult>('restore_subtitle', {
      videoPath: file.path,
      backupPath: backup.backup_path,
      trackIndex: backup.track_index,
      ffmpegPath: settings?.ffmpegPath || null
    })
    
    if (result.success) {
      // Reload video info
      const videoInfo = await invoke<VideoInfo>('get_video_info', {
        videoPath: file.path,
        ffmpegPath: settings?.ffmpegPath || null
      })
      file.videoInfo = videoInfo
      console.log('Backup restored successfully')
    } else {
      console.error('Restore failed:', result.message)
    }
  } catch (e) {
    console.error('Restore error:', e)
  }
}

const deleteBackup = async (file: SelectedFile, backup: BackupInfo) => {
  try {
    const result = await invoke<OperationResult>('delete_backup', {
      backupPath: backup.backup_path,
      videoPath: file.path
    })
    
    if (result.success) {
      file.backups = file.backups.filter(b => b.backup_path !== backup.backup_path)
    }
  } catch (e) {
    console.error('Delete backup error:', e)
  }
}

// Translation state
const isTranslating = ref(false)
const translationProgress = ref(0)
const currentStatus = ref('')
const estimatedTime = ref('')
const currentFileIndex = ref(0)

const setProgress = (value: number) => {
  const clamped = Math.min(100, Math.max(0, Number.isFinite(value) ? value : 0))
  translationProgress.value = clamped
}

// Helper to get settings from ref or cached value
const getSettings = (): Settings | null => {
  if (settingsRef.value?.settings) {
    return settingsRef.value.settings
  }
  return cachedSettings.value
}

const canStartTranslation = computed(() => {
  // Use cachedSettings directly for reactivity
  const settings = cachedSettings.value
  const hasApiConfig = settings?.provider === 'ollama' || (settings?.apiKey && settings?.selectedModel)
  const hasFiles = selectedFiles.value.length > 0
  const filesReady = selectedFiles.value.some(f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0)
  return hasApiConfig && hasFiles && filesReady && ffmpegStatus.value?.success
})

const startTranslation = async () => {
  if (!canStartTranslation.value) {
    if (!ffmpegStatus.value?.success) {
      showSettings.value = true
    } else if (!getSettings()?.apiKey) {
      showSettings.value = true
    }
    return
  }
  
  const settings = getSettings()
  if (!settings) return
  
  isTranslating.value = true
  setProgress(0)
  currentFileIndex.value = 0
  
  const filesToProcess = selectedFiles.value.filter(
    f => f.videoInfo && f.videoInfo.subtitle_tracks.length > 0
  )
  
  try {
    for (let i = 0; i < filesToProcess.length; i++) {
      const file = filesToProcess[i]
      currentFileIndex.value = i
      currentStatus.value = `Processing ${file.name} (${i + 1}/${filesToProcess.length})`
      
      // Select track - use the one from options or find first suitable
      const trackIndex = translationOptions.subtitleTrack 
        ? parseInt(translationOptions.subtitleTrack) 
        : 0
      
      const track = file.videoInfo!.subtitle_tracks[trackIndex]
      if (!track) {
        console.error(`Track ${trackIndex} not found in ${file.name}`)
        continue
      }
      
      // Extract subtitle to temp file
      currentStatus.value = `Extracting subtitles from ${file.name}...`
      setProgress(((i / filesToProcess.length) * 100) + (5 / filesToProcess.length))
      
      // Determine format based on settings or codec
      let format = settings.outputFormat || 'srt'
      
      // If output format is not specified or is 'ass', try to keep original format if it's ASS
      if ((!settings.outputFormat || settings.outputFormat === 'ass') && (track.codec.includes('ass') || track.codec.includes('ssa'))) {
        format = 'ass'
      } else if (settings.outputFormat === 'srt') {
        format = 'srt'
      } else if (settings.outputFormat === 'vtt') {
        format = 'vtt'
      } else {
        // Fallback logic
        format = track.codec.includes('ass') || track.codec.includes('ssa') ? 'ass' : 
                 track.codec.includes('subrip') || track.codec.includes('srt') ? 'srt' : 
                 track.codec.includes('webvtt') ? 'vtt' : 'srt'
      }
      
      const extractResult = await invoke<{ success: boolean; output_path: string | null; error: string | null }>('extract_subtitle', {
        videoPath: file.path,
        trackIndex,
        outputPath: null,
        format,
        ffmpegPath: settings?.ffmpegPath || null
      })
      
      if (!extractResult.success || !extractResult.output_path) {
        console.error(`Failed to extract subtitle from ${file.name}:`, extractResult.error)
        continue
      }
      
      const extractedPath = extractResult.output_path
      
      // Parse the extracted subtitle file
      currentStatus.value = `Parsing subtitles from ${file.name}...`
      setProgress(((i / filesToProcess.length) * 100) + (10 / filesToProcess.length))
      
      const subtitleData = await invoke<SubtitleData>('parse_subtitle_file', {
        filePath: extractedPath
      })
      
      if (!subtitleData || subtitleData.lines.length === 0) {
        console.error(`No dialog found in ${file.name}`)
        continue
      }
      
      // Get system prompt from settings modal
      const systemPrompt = settingsRef.value?.getSystemPrompt?.() || 
        `You are a professional subtitle translator. Translate the following subtitle lines to ${settings.targetLanguage}. Keep translations natural and contextually appropriate for anime dialogue.`
      
      // Build LLM config
      const llmConfig = {
        provider: settings.provider,
        api_key: settings.apiKey,
        endpoint: settings.apiEndpoint,
        model: settings.selectedModel || '',
        system_prompt: systemPrompt
      }
      
      // Translate subtitles
      currentStatus.value = `Translating ${file.name} (${subtitleData.lines.length} lines)...`
      setProgress(((i / filesToProcess.length) * 100) + (20 / filesToProcess.length))
      
      const translatedData = await invoke<SubtitleData>('translate_subtitles', {
        subtitleData,
        config: llmConfig,
        sourceLang: settings.sourceLanguage || 'auto',
        targetLang: settings.targetLanguage,
        batchSize: translationOptions.batchSize
      })
      
      if (!translatedData) {
        console.error(`Translation failed for ${file.name}`)
        continue
      }
      
      // Generate output path
      const timestamp = new Date().toISOString().replace(/[:.]/g, '').slice(0, 15)
      const baseName = file.path.replace(/\.[^.]+$/, '')
      const targetLangValue = settings.targetLanguage || track.language || 'und'
      const filenameLangCode = sanitizeLangCodeForFilename(targetLangValue)
      const ffmpegLangCode = toFfmpegLangCode(targetLangValue)
      const outputPath = `${baseName}_${filenameLangCode}_${timestamp}_track${trackIndex}.${format}`
      
      // Save translated subtitles
      currentStatus.value = `Saving translated subtitles for ${file.name}...`
      setProgress(((i / filesToProcess.length) * 100) + (80 / filesToProcess.length))
      
      const saveResult = await invoke<OperationResult>('save_translated_subtitles', {
        translatedData,
        outputPath,
        originalFilePath: extractedPath
      })
      
      if (!saveResult.success) {
        console.error(`Failed to save translated subtitles for ${file.name}:`, saveResult.message)
        continue
      }
      
      // Embed subtitles if enabled
      if (translationOptions.embedSubtitles) {
        currentStatus.value = `Embedding translated subtitles in ${file.name}...`
        setProgress(((i / filesToProcess.length) * 100) + (90 / filesToProcess.length))
        
        // Refresh video info to check for existing translated tracks
        const currentInfo = await invoke<VideoInfo>('get_video_info', {
          videoPath: file.path,
          ffmpegPath: settings?.ffmpegPath || null
        })
        
        // Remove existing translated tracks to prevent duplicates
        const translatedTitle = `Translated (${filenameLangCode})`
        const tracksToRemove = currentInfo.subtitle_tracks
          .filter(t => {
            if (t.title === translatedTitle || t.title?.startsWith('Translated (')) return true
            const trackLangIso = toFfmpegLangCode(t.language)
            return trackLangIso === ffmpegLangCode && t.index !== trackIndex
          })
          .sort((a, b) => b.index - a.index) // Remove from end to start
          
        for (const t of tracksToRemove) {
          currentStatus.value = `Removing existing translated track ${t.index} from ${file.name}...`
          await invoke('remove_subtitle_track', {
            videoPath: file.path,
            trackIndex: t.index,
            ffmpegPath: settings?.ffmpegPath || null
          })
        }
        
        await invoke<OperationResult>('embed_subtitle', {
          videoPath: file.path,
          subtitlePath: outputPath,
          language: ffmpegLangCode,
          title: translatedTitle,
          setDefault: true,
          ffmpegPath: settings?.ffmpegPath || null,
          useMkvmerge: translationOptions.useMkvmerge
        })
      }
      
      setProgress((((i + 1) / filesToProcess.length) * 100))
    }
    
    currentStatus.value = 'Translation complete!'
  } catch (e) {
    console.error('Translation error:', e)
    currentStatus.value = `Error: ${e}`
  } finally {
    isTranslating.value = false
  }
}
</script>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body, #app {
  height: 100%;
  overflow: hidden;
}

body {
  font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
  background: #0d1016;
}

/* Dark mode specific overrides */
.n-config-provider {
  height: 100%;
}
</style>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: v-bind('isDark ? "#0d1016" : "#f8f9fa"');
  transition: background 0.3s ease;
}

.app-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 24px;
  background: v-bind('isDark ? "#101522" : "#ffffff"');
  border-bottom: 1px solid v-bind('isDark ? "#1f2633" : "#e5e7eb"');
  -webkit-app-region: drag;
  transition: background 0.3s ease, border-color 0.3s ease;
}

.app-header :deep(button) {
  -webkit-app-region: no-drag;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 16px;
}

.header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

.logo {
  display: flex;
  align-items: center;
  gap: 10px;
}

.logo-text {
  font-size: 18px;
  font-weight: 600;
  background: linear-gradient(135deg, #6366f1, #8b5cf6);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
}

.app-main {
  flex: 1;
  overflow-y: auto;
  padding: 16px;
}

.main-content {
  width: min(960px, 100%);
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding-bottom: 24px;
}

.drop-zone {
  border: 2px dashed var(--n-border-color);
  border-radius: 12px;
  transition: all 0.3s ease;
  cursor: pointer;
}

.drop-zone:hover,
.drop-zone-active {
  border-color: var(--n-primary-color);
  background: var(--n-color-hover);
}

.drop-zone-content {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 36px 24px;
  text-align: center;
  gap: 16px;
}

.drop-zone-content h2 {
  font-size: 20px;
  font-weight: 600;
  color: var(--n-text-color-1);
}

.drop-zone-content p {
  color: var(--n-text-color-3);
  margin-bottom: 8px;
}

.drop-icon {
  width: 88px;
  height: 88px;
  object-fit: contain;
  image-rendering: -webkit-optimize-contrast;
  image-rendering: optimizeQuality;
}

.files-card,
.options-card,
.progress-card {
  border-radius: 12px;
}

.file-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.file-item {
  padding: 12px;
  border-radius: 8px;
  background: v-bind('isDark ? "rgba(255,255,255,0.03)" : "rgba(0,0,0,0.02)"');
  border: 1px solid v-bind('isDark ? "#2a2a4a" : "#e5e7eb"');
}

.file-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.file-name {
  font-weight: 500;
  color: var(--n-text-color-1);
}

.subtitle-tracks {
  margin-top: 8px;
  padding-left: 28px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.subtitle-track {
  padding: 6px 10px;
  border-radius: 6px;
  background: v-bind('isDark ? "rgba(255, 255, 255, 0.05)" : "rgba(0, 0, 0, 0.03)"');
}

.track-info {
  font-size: 13px;
  color: var(--n-text-color-2);
}

.track-codec {
  font-size: 11px;
  color: var(--n-text-color-3);
}

.no-subs-warning {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 8px;
  padding-left: 28px;
  font-size: 12px;
  color: var(--warning-color);
}

.file-error {
  margin-top: 8px;
  padding-left: 28px;
  font-size: 12px;
}

.backups-section {
  margin-top: 8px;
  padding-left: 28px;
}

.backup-item {
  padding: 4px 8px;
  border-radius: 4px;
  background: v-bind('isDark ? "rgba(82, 196, 26, 0.1)" : "rgba(82, 196, 26, 0.05)"');
  margin-bottom: 4px;
}

.backup-name {
  font-size: 12px;
  color: var(--n-text-color-2);
}

.action-bar {
  display: flex;
  justify-content: center;
  padding: 8px 0;
}

.progress-status {
  color: var(--n-text-color-2);
}

.progress-eta {
  color: var(--n-text-color-3);
  font-size: 13px;
}

@media (max-width: 960px) {
  .main-content {
    width: 100%;
    padding: 0 8px 24px;
  }
}

@media (max-width: 720px) {
  .app-header {
    padding: 10px 16px;
  }

  .drop-zone-content {
    padding: 20px 10px;
  }

  .drop-zone-content h2 {
    font-size: 18px;
  }

  .drop-zone-content p {
    font-size: 13px;
  }

  .drop-icon {
    width: 64px;
    height: 64px;
  }

  :deep(.options-card .n-grid) {
    grid-template-columns: 1fr !important;
    gap: 12px !important;
  }

  .action-bar {
    padding: 4px 0;
  }
}
</style>
