<template>
  <article class="queue-row" :class="[`state-${state}`, { expanded }]">
    <div class="row-main">
      <div class="row-status" aria-hidden="true">
        <n-spin v-if="state === 'probing'" size="small" />
        <n-icon v-else-if="state === 'translating'" size="20" class="status-accent">
          <sync-outline class="rotating" />
        </n-icon>
        <n-icon v-else-if="state === 'done'" size="20" class="status-success">
          <checkmark-circle-outline />
        </n-icon>
        <n-icon v-else-if="state === 'failed'" size="20" class="status-danger">
          <close-circle-outline />
        </n-icon>
        <n-icon v-else-if="state === 'no-subs'" size="20" class="status-warning">
          <alert-circle-outline />
        </n-icon>
        <n-icon v-else-if="state === 'queued'" size="18" class="status-muted">
          <time-outline />
        </n-icon>
        <span v-else class="ready-dot" />
      </div>

      <div class="row-body">
        <div class="row-title-line">
          <h4 class="row-title">{{ file.name }}</h4>
          <span v-if="state === 'queued'" class="row-badge muted">
            {{ t('queue.inQueue', { n: queuePos }) }}
          </span>
        </div>
        <p class="row-summary">{{ summary }}</p>
        <div v-if="state === 'translating'" class="row-progress">
          <n-progress
            type="line"
            :percentage="fileProgress"
            :show-indicator="false"
            :height="4"
            border-radius="2px"
          />
        </div>
        <p v-if="file.path" class="row-path">{{ file.path }}</p>
      </div>

      <div class="row-actions">
        <n-button
          v-if="state === 'failed' && !file.error && isConfigError"
          size="small"
          type="warning"
          ghost
          @click="emit('open-settings')"
        >
          {{ t('footer.openSettings') }}
        </n-button>
        <n-button
          v-else-if="state === 'failed' && !file.error"
          size="small"
          type="primary"
          ghost
          :loading="false"
          @click="emit('retry', file)"
        >
          <template #icon><n-icon><refresh-outline /></n-icon></template>
          {{ t('queue.retry') }}
        </n-button>
        <n-button
          v-if="state === 'done'"
          size="small"
          quaternary
          @click="emit('retranslate', file)"
        >
          <template #icon><n-icon><refresh-outline /></n-icon></template>
          {{ t('queue.retranslate') }}
        </n-button>
        <n-button
          quaternary
          size="small"
          :title="expanded ? t('queue.collapse') : t('queue.expand')"
          @click="emit('toggle-expand')"
        >
          <template #icon>
            <n-icon :class="{ 'chevron-open': expanded }" class="chevron">
              <chevron-down-outline />
            </n-icon>
          </template>
        </n-button>
        <n-button
          v-if="state !== 'translating'"
          quaternary
          size="small"
          type="error"
          :title="t('queue.remove')"
          @click="emit('remove')"
        >
          <template #icon><n-icon><close-outline /></n-icon></template>
        </n-button>
      </div>
    </div>

    <div v-if="expanded" class="row-details">
      <template v-if="file.videoInfo && file.videoInfo.subtitle_tracks.length > 0">
        <div v-for="track in file.videoInfo.subtitle_tracks" :key="track.index" class="track-row">
          <div class="track-meta">
            <span class="track-lang">{{ track.language || 'und' }}</span>
            <span class="track-title">{{ track.title || t('track.title', { index: track.index }) }}</span>
            <span class="track-codec">{{ t('queue.trackMeta', { index: track.index, codec: track.codec }) }}</span>
            <n-tag v-if="track.default" size="tiny" round :bordered="false" type="success">
              {{ t('queue.default') }}
            </n-tag>
            <n-tag v-if="track.forced" size="tiny" round :bordered="false" type="warning">
              {{ t('queue.forced') }}
            </n-tag>
          </div>
          <div class="track-actions">
            <n-button
              size="tiny"
              quaternary
              :title="t('queue.extractSubtitle')"
              :loading="extracting"
              @click="emit('extract', track.index)"
            >
              <template #icon><n-icon><download-outline /></n-icon></template>
            </n-button>
            <n-button
              size="tiny"
              quaternary
              type="success"
              :title="t('queue.backupSubtitle')"
              :loading="backingUp"
              @click="emit('backup', track.index)"
            >
              <template #icon><n-icon><shield-checkmark-outline /></n-icon></template>
            </n-button>
          </div>
        </div>
      </template>
      <p v-else-if="file.videoInfo" class="no-subs-note">{{ t('queue.noSubs') }}</p>

      <div v-if="file.backups.length > 0" class="backups-block">
        <p class="backups-title">{{ t('queue.backupsTitle') }}</p>
        <div v-for="backup in file.backups" :key="backup.backup_path" class="backup-item">
          <span class="backup-meta">
            {{ t('queue.backupMeta', { track: backup.track_index, format: backup.format, date: backup.created_at }) }}
          </span>
          <div class="backup-actions">
            <n-popconfirm @positive-click="emit('restore', backup)">
              <template #trigger>
                <n-button size="tiny" quaternary type="warning">
                  <template #icon><n-icon><arrow-undo-outline /></n-icon></template>
                </n-button>
              </template>
              {{ t('queue.restoreBackupConfirm') }}
            </n-popconfirm>
            <n-popconfirm @positive-click="emit('delete-backup', backup)">
              <template #trigger>
                <n-button size="tiny" quaternary type="error">
                  <template #icon><n-icon><trash-outline /></n-icon></template>
                </n-button>
              </template>
              {{ t('queue.deleteBackupConfirm') }}
            </n-popconfirm>
          </div>
        </div>
      </div>
    </div>
  </article>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { NButton, NIcon, NProgress, NSpin, NTag, NPopconfirm } from 'naive-ui'
import {
  AlertCircleOutline,
  ArrowUndoOutline,
  CheckmarkCircleOutline,
  ChevronDownOutline,
  CloseCircleOutline,
  CloseOutline,
  DownloadOutline,
  RefreshOutline,
  ShieldCheckmarkOutline,
  SyncOutline,
  TimeOutline,
  TrashOutline
} from '@vicons/ionicons5'
import type { BackupInfo, SelectedFile } from '../types/domain'
import type { QueueRowState } from '../composables/useQueueStates'
import type { FileOutcome, TranslationPhase } from '../composables/useTranslationJob'

const props = defineProps<{
  file: SelectedFile
  state: QueueRowState
  queuePos: number
  outcome?: FileOutcome
  phase: TranslationPhase
  lines: { done: number; total: number } | null
  expanded: boolean
  extracting: boolean
  backingUp: boolean
}>()

const emit = defineEmits<{
  (e: 'remove'): void
  (e: 'retry', file: SelectedFile): void
  (e: 'retranslate', file: SelectedFile): void
  (e: 'toggle-expand'): void
  (e: 'extract', trackIndex: number): void
  (e: 'backup', trackIndex: number): void
  (e: 'restore', backup: BackupInfo): void
  (e: 'delete-backup', backup: BackupInfo): void
  (e: 'open-settings'): void
}>()

const { t } = useI18n()

const primaryTrack = computed(() => {
  const tracks = props.file.videoInfo?.subtitle_tracks
  if (!tracks || tracks.length === 0) return null
  return tracks.find(track => track.default) || tracks[0]
})

const summary = computed(() => {
  const { state, file, outcome } = props
  if (state === 'probing') return t('queue.reading')
  if (state === 'failed') {
    return outcome?.detail || file.error || t('queue.probeFailed')
  }
  if (state === 'no-subs') return t('queue.noSubs')
  if (state === 'done') {
    return outcome?.embedded ? t('queue.doneEmbedded') : t('queue.doneSaved')
  }
  if (state === 'translating') {
    if (props.phase === 'translate' && props.lines && props.lines.total > 0) {
      return t('queue.phaseTranslate', { done: props.lines.done, total: props.lines.total })
    }
    if (props.phase === 'extract') return t('queue.phaseExtract')
    if (props.phase === 'write') return t('queue.phaseWrite')
    return t('queue.phaseOther')
  }
  const track = primaryTrack.value
  if (!track) return ''
  const lang = track.language || 'und'
  const suffix = track.default ? ` (${t('queue.default')})` : ''
  return `${t('queue.trackSummary', { language: lang })}${suffix} · ${track.codec}`
})

const fileProgress = computed(() => {
  if (props.lines && props.lines.total > 0) {
    return Math.round((props.lines.done / props.lines.total) * 100)
  }
  return props.phase === 'write' ? 95 : 5
})

const isConfigError = computed(() =>
  /api key|access denied|invalid|unauthorized|401|403/i.test(props.outcome?.detail || '')
)
</script>

<style scoped>
.queue-row {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  transition: border-color 120ms ease, background 120ms ease;
}

.queue-row:hover {
  border-color: var(--border-strong);
}

.queue-row.state-translating {
  border-color: var(--accent);
}

.queue-row.state-failed {
  border-color: var(--danger);
}

.queue-row.state-done {
  border-color: var(--success);
}

.row-main {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 12px 14px;
}

.row-status {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  margin-top: 2px;
}

.ready-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--success);
}

.status-success { color: var(--success); }
.status-danger { color: var(--danger); }
.status-warning { color: var(--warning); }
.status-accent { color: var(--accent); }
.status-muted { color: var(--text-muted); }

.rotating {
  animation: spin 1.4s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.row-body {
  flex: 1;
  min-width: 0;
}

.row-title-line {
  display: flex;
  align-items: center;
  gap: 8px;
}

.row-title {
  margin: 0;
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-badge {
  font-family: var(--font-mono);
  font-size: 11px;
  padding: 2px 8px;
  border-radius: 999px;
  flex-shrink: 0;
}

.row-badge.muted {
  color: var(--text-muted);
  background: var(--hover-overlay);
}

.row-summary {
  margin: 2px 0 0;
  font-size: 12.5px;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.state-failed .row-summary { color: var(--danger); }
.state-no-subs .row-summary { color: var(--warning); }

.row-progress {
  margin-top: 8px;
}

.row-path {
  margin: 4px 0 0;
  font-family: var(--font-mono);
  font-size: 10.5px;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.row-actions {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}

.chevron {
  transition: transform 150ms ease;
}

.chevron-open {
  transform: rotate(180deg);
}

.row-details {
  border-top: 1px solid var(--border-subtle);
  padding: 10px 14px 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.track-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 6px 8px;
  border-radius: var(--radius-sm);
  background: var(--bg-inset);
}

.track-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  font-size: 12px;
}

.track-lang {
  font-family: var(--font-mono);
  font-weight: 700;
  color: var(--accent);
  text-transform: uppercase;
}

.track-title {
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.track-codec {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-muted);
}

.track-actions {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
}

.no-subs-note {
  margin: 0;
  font-size: 12px;
  color: var(--warning);
}

.backups-block {
  border-top: 1px dashed var(--border-subtle);
  padding-top: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.backups-title {
  margin: 0;
  font-size: 11px;
  font-weight: 700;
  color: var(--text-muted);
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.backup-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.backup-meta {
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--text-secondary);
}

.backup-actions {
  display: flex;
  gap: 2px;
}
</style>
