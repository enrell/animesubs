<template>
  <section class="log-drawer" :class="{ open: isOpen }">
    <button type="button" class="log-toggle" @click="isOpen = !isOpen">
      <n-icon size="14" class="chevron" :class="{ 'chevron-open': isOpen }">
        <chevron-up-outline />
      </n-icon>
      <span class="log-title">{{ t('log.title') }}</span>
      <span v-if="errorCount > 0" class="log-badge">{{ errorCount }}</span>
      <span class="log-spacer" />
      <n-button v-if="isOpen && entries.length > 0" text size="tiny" @click.stop="clear">
        {{ t('log.clear') }}
      </n-button>
    </button>
    <div v-if="isOpen" class="log-body">
      <n-scrollbar class="log-scroll">
        <p v-if="entries.length === 0" class="log-empty">{{ t('log.empty') }}</p>
        <div v-for="entry in entries" :key="entry.id" class="log-entry">
          <span class="log-time">{{ entry.time }}</span>
          <span class="log-level" :class="`level-${entry.level}`" />
          <span class="log-message">{{ entry.message }}</span>
        </div>
      </n-scrollbar>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { NButton, NIcon, NScrollbar } from 'naive-ui'
import { ChevronUpOutline } from '@vicons/ionicons5'
import { useActivityLog } from '../composables/useActivityLog'

const { t } = useI18n()
const { entries, clear } = useActivityLog()

const isOpen = ref(false)

const errorCount = computed(() => entries.value.filter(e => e.level === 'error').length)
</script>

<style scoped>
.log-drawer {
  border-top: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  flex-shrink: 0;
}

.log-toggle {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 7px 16px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
  text-align: left;
}

.log-toggle:hover {
  background: var(--hover-overlay);
  color: var(--text-primary);
}

.chevron {
  transition: transform 150ms ease;
}

.chevron-open {
  transform: rotate(180deg);
}

.log-title {
  font-weight: 600;
}

.log-badge {
  background: var(--danger);
  color: #fff;
  font-family: var(--font-mono);
  font-size: 10px;
  font-weight: 700;
  border-radius: 999px;
  padding: 1px 7px;
}

.log-spacer {
  flex: 1;
}

.log-body {
  height: 160px;
  border-top: 1px solid var(--border-subtle);
}

.log-scroll {
  height: 100%;
  padding: 8px 16px;
}

.log-empty {
  margin: 8px 0;
  font-size: 12px;
  color: var(--text-muted);
}

.log-entry {
  display: flex;
  align-items: baseline;
  gap: 10px;
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 1.8;
}

.log-time {
  color: var(--text-muted);
  flex-shrink: 0;
}

.log-level {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
  align-self: center;
}

.level-info { background: var(--text-muted); }
.level-success { background: var(--success); }
.level-warning { background: var(--warning); }
.level-error { background: var(--danger); }

.log-message {
  color: var(--text-secondary);
  word-break: break-word;
}

.level-error + .log-message { color: var(--danger); }
.level-success + .log-message { color: var(--success); }
</style>
