<template>
  <div class="empty-state">
    <div class="empty-hero">
      <h2 class="empty-title">{{ t('empty.title') }}</h2>
      <p class="empty-description">{{ t('empty.description') }}</p>
      <div class="empty-actions">
        <n-button type="primary" size="large" :loading="loading" @click="emit('select-files')">
          <template #icon><n-icon><document-outline /></n-icon></template>
          {{ t('empty.selectFiles') }}
        </n-button>
        <n-button size="large" :loading="loading" @click="emit('select-folder')">
          <template #icon><n-icon><folder-open-outline /></n-icon></template>
          {{ t('empty.scanFolder') }}
        </n-button>
      </div>
    </div>

    <div v-if="needsSetup" class="setup-checklist">
      <p class="checklist-title">{{ t('empty.checklistTitle') }}</p>

      <div class="checklist-item" :class="{ done: providerReady }">
        <n-icon size="18" class="check-icon">
          <checkmark-circle-outline v-if="providerReady" />
          <ellipse-outline v-else />
        </n-icon>
        <div class="check-text">
          <span class="check-label">{{ t('empty.providerStep') }}</span>
          <span class="check-detail">
            {{ providerReady ? t('empty.providerDone', { provider: providerName }) : t('empty.providerTodo') }}
          </span>
        </div>
        <n-button v-if="!providerReady" size="small" tertiary @click="emit('open-settings')">
          {{ t('empty.fixInSettings') }}
        </n-button>
      </div>

      <div class="checklist-item" :class="{ done: ffmpegReady }">
        <n-icon size="18" class="check-icon">
          <checkmark-circle-outline v-if="ffmpegReady" />
          <ellipse-outline v-else />
        </n-icon>
        <div class="check-text">
          <span class="check-label">{{ t('empty.ffmpegStep') }}</span>
          <span class="check-detail">
            {{ ffmpegReady ? t('empty.ffmpegDone') : t('empty.ffmpegTodo') }}
          </span>
        </div>
        <n-button v-if="!ffmpegReady" size="small" tertiary @click="emit('open-settings')">
          {{ t('empty.fixInSettings') }}
        </n-button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { NButton, NIcon } from 'naive-ui'
import {
  CheckmarkCircleOutline,
  DocumentOutline,
  EllipseOutline,
  FolderOpenOutline
} from '@vicons/ionicons5'

const props = defineProps<{
  loading: boolean
  providerReady: boolean
  providerName: string
  ffmpegReady: boolean
}>()

const emit = defineEmits<{
  (e: 'select-files'): void
  (e: 'select-folder'): void
  (e: 'open-settings'): void
}>()

const { t } = useI18n()

const needsSetup = computed(() => !props.providerReady || !props.ffmpegReady)
</script>

<style scoped>
.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 32px;
  padding: 48px 24px;
  min-height: 0;
  overflow-y: auto;
}

.empty-hero {
  text-align: center;
  max-width: 520px;
}

.empty-title {
  margin: 0 0 12px;
  font-size: 28px;
  font-weight: 800;
  letter-spacing: -0.02em;
  color: var(--text-primary);
}

.empty-description {
  margin: 0 0 24px;
  font-size: 14.5px;
  line-height: 1.6;
  color: var(--text-secondary);
}

.empty-actions {
  display: flex;
  justify-content: center;
  gap: 12px;
  flex-wrap: wrap;
}

.setup-checklist {
  width: min(440px, 100%);
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: 16px 18px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.checklist-title {
  margin: 0;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.checklist-item {
  display: flex;
  align-items: center;
  gap: 12px;
}

.check-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}

.checklist-item.done .check-icon {
  color: var(--success);
}

.check-text {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.check-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.check-detail {
  font-size: 12px;
  color: var(--text-secondary);
}
</style>
