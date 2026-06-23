import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, triggerRef } from 'vue'
import {
  backupSubtitle as backupSubtitleCommand,
  deleteBackup as deleteBackupCommand,
  extractSubtitle as extractSubtitleCommand,
  getVideoInfo,
  listBackups,
  restoreSubtitle as restoreSubtitleCommand,
  scanFolderForVideos
} from '../api/animesubs'
import type { BackupInfo, SelectedFile } from '../types/domain'
import type { Settings } from '../config/settings'
import { localizeBackendMessage } from '../i18n'

type TranslateFn = (key: string, named?: Record<string, unknown>) => string

export const useVideoFiles = (getSettings: () => Settings | null, t: TranslateFn) => {
  const selectedFiles = ref<SelectedFile[]>([])
  const isDragging = ref(false)
  const loadingFiles = ref(false)
  const extractingSubtitle = ref<string | null>(null)
  const backingUp = ref<string | null>(null)

  const subtitleTrackOptions = computed(() => {
    const options = [{ label: t('app.autoDetectFirstAvailable'), value: '' }]
    const tracks = new Map<string, string>()

    for (const file of selectedFiles.value) {
      if (!file.videoInfo) continue

      for (const track of file.videoInfo.subtitle_tracks) {
        const key = `${track.index}`
        const lang = track.language || 'und'
        const title = track.title || t('track.title', { index: track.index })
        const label = `${title} (${lang}) - ${track.codec}`
        if (!tracks.has(key)) {
          tracks.set(key, label)
        }
      }
    }

    tracks.forEach((label, value) => {
      options.push({ label, value })
    })

    return options
  })

  const addFiles = async (paths: string[]) => {
    const settings = getSettings()

    for (const path of paths) {
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
      triggerRef(selectedFiles)

      const fileIndex = selectedFiles.value.length - 1

      try {
        const videoInfo = await getVideoInfo(path, settings?.ffmpegPath || null)
        const backups = await listBackups(path)

        selectedFiles.value[fileIndex] = {
          ...selectedFiles.value[fileIndex],
          videoInfo,
          backups,
          loading: false
        }
        triggerRef(selectedFiles)
      } catch (e) {
        selectedFiles.value[fileIndex] = {
          ...selectedFiles.value[fileIndex],
          error: localizeBackendMessage(`${e}`, t),
          loading: false
        }
        triggerRef(selectedFiles)
      }
    }
  }

  const selectFiles = async () => {
    const selected = await open({
      multiple: true,
      filters: [{
        name: t('dialogs.videoFiles'),
        extensions: ['mkv', 'mp4', 'webm', 'avi', 'mov', 'wmv', 'flv', 'm4v']
      }]
    })

    if (selected) {
      const files = Array.isArray(selected) ? selected : [selected]
      await addFiles(files)
    }
  }

  const selectFolder = async () => {
    const selected = await open({ directory: true, multiple: false })

    if (selected) {
      loadingFiles.value = true
      try {
        const videos = await scanFolderForVideos(selected)
        await addFiles(videos)
      } catch (e) {
        console.error('Failed to scan folder:', e)
      } finally {
        loadingFiles.value = false
      }
    }
  }

  const handleDrop = async (e: DragEvent) => {
    e.preventDefault()
    isDragging.value = false
    const dt = e.dataTransfer
    if (!dt) return

    const paths: string[] = []
    if (dt.files?.length) {
      for (const f of Array.from(dt.files)) {
        const path = (f as File & { path?: string }).path || f.name
        if (path) paths.push(path)
      }
    } else if (dt.items?.length) {
      for (const item of Array.from(dt.items)) {
        const file = item.getAsFile()
        if (file) {
          const path = (file as File & { path?: string }).path || file.name
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
  }

  const clearFiles = () => {
    selectedFiles.value = []
  }

  const extractSubtitle = async (file: SelectedFile, trackIndex: number) => {
    if (!file.videoInfo) return

    extractingSubtitle.value = file.path
    const settings = getSettings()

    try {
      const result = await extractSubtitleCommand({
        videoPath: file.path,
        trackIndex,
        outputPath: null,
        format: settings?.outputFormat || 'srt',
        temporary: false,
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
      const backupInfo = await backupSubtitleCommand(file.path, trackIndex, settings?.ffmpegPath || null)
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
      const result = await restoreSubtitleCommand({
        videoPath: file.path,
        backupPath: backup.backup_path,
        trackIndex: backup.track_index,
        ffmpegPath: settings?.ffmpegPath || null
      })

      if (result.success) {
        file.videoInfo = await getVideoInfo(file.path, settings?.ffmpegPath || null)
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
      const result = await deleteBackupCommand(backup.backup_path, file.path)
      if (result.success) {
        file.backups = file.backups.filter(b => b.backup_path !== backup.backup_path)
      }
    } catch (e) {
      console.error('Delete backup error:', e)
    }
  }

  return {
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
    clearFiles,
    extractSubtitle,
    backupSubtitle,
    restoreBackup,
    deleteBackup
  }
}
