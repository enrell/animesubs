<template>
  <div class="context-panel">
    <div class="context-header">
      <div>
        <p class="eyebrow">{{ t('context.eyebrow') }}</p>
        <h4>{{ t('context.title') }}</h4>
      </div>
      <n-checkbox
        :checked="contextEnabled"
        :disabled="!contextProfile"
        @update:checked="toggleContext"
      >
        <span class="checkbox-label-content">
          <n-icon><bulb-outline /></n-icon>
          {{ t('context.enable') }}
        </span>
      </n-checkbox>
    </div>

    <div v-if="!contextProfile" class="context-empty">
      <p class="context-empty-hint">{{ t('context.noProfile') }}</p>
      <div class="context-actions">
        <n-button
          size="small"
          type="primary"
          ghost
          :loading="isExtracting"
          :disabled="!canExtract"
          @click="onExtractFromFirstFile"
        >
          <template #icon><n-icon><sparkles-outline /></n-icon></template>
          {{ t('context.extractFromMedia') }}
        </n-button>
        <n-button
          size="small"
          quaternary
          @click="onLoadFromFile"
        >
          <template #icon><n-icon><folder-open-outline /></n-icon></template>
          {{ t('context.loadFromFile') }}
        </n-button>
      </div>
    </div>

    <div v-else class="context-content">
      <div class="context-actions">
        <n-button
          size="small"
          quaternary
          @click="showEditor = !showEditor"
        >
          <template #icon><n-icon><create-outline /></n-icon></template>
          {{ showEditor ? t('context.hideEditor') : t('context.edit') }}
        </n-button>
        <n-button
          size="small"
          quaternary
          :loading="isExtracting"
          :disabled="!canExtract"
          @click="onExtractFromFirstFile"
        >
          <template #icon><n-icon><refresh-outline /></n-icon></template>
          {{ t('context.reextract') }}
        </n-button>
        <n-button size="small" quaternary @click="onSaveToFile">
          <template #icon><n-icon><save-outline /></n-icon></template>
          {{ t('context.saveToFile') }}
        </n-button>
        <n-popconfirm @positive-click="clearProfile">
          <template #trigger>
            <n-button size="small" quaternary type="error">
              <template #icon><n-icon><trash-outline /></n-icon></template>
            </n-button>
          </template>
          {{ t('context.clearConfirm') }}
        </n-popconfirm>
      </div>

      <div v-if="!showEditor" class="context-summary">
        <div v-if="contextProfile.media_match" class="summary-field media-match-field">
          <span class="field-label">{{ t('context.fields.verifiedMatch') }}</span>
          <div class="media-match-info">
            <span class="media-title">{{ contextProfile.media_match.title_display }}</span>
            <n-tag v-if="contextProfile.media_match.format" size="tiny" :bordered="false" class="wired-tag">{{ contextProfile.media_match.format }}</n-tag>
            <n-tag v-if="contextProfile.media_match.season_year" size="tiny" :bordered="false" class="wired-tag">{{ contextProfile.media_match.season }} {{ contextProfile.media_match.season_year }}</n-tag>
            <span class="media-provider">{{ contextProfile.media_match.provider }} #{{ contextProfile.media_match.source_id }}</span>
          </div>
          <div v-if="contextProfile.media_match.genres.length > 0" class="tag-list">
            <n-tag v-for="genre in contextProfile.media_match.genres" :key="genre" size="tiny" :bordered="false" class="wired-tag">{{ genre }}</n-tag>
          </div>
          <p v-if="contextProfile.media_match.synopsis" class="plot-text">{{ contextProfile.media_match.synopsis.slice(0, 300) }}{{ contextProfile.media_match.synopsis.length > 300 ? '...' : '' }}</p>
        </div>

        <div v-if="contextProfile.title" class="summary-field">
          <span class="field-label">{{ t('context.fields.title') }}:</span>
          <span>{{ contextProfile.title }}</span>
        </div>
        <div v-if="contextProfile.characters.length > 0" class="summary-field">
          <span class="field-label">{{ t('context.fields.characters') }}:</span>
          <div class="tag-list">
            <n-tag v-for="char in contextProfile.characters" :key="char.name" size="small" :bordered="false" class="wired-tag" :class="{ 'verified-tag': char.verified }">
              {{ char.name }}<template v-if="char.role"> ({{ char.role }})</template>
              <template v-if="char.verified"> ✓</template>
            </n-tag>
          </div>
        </div>
        <div v-if="contextProfile.locations.length > 0" class="summary-field">
          <span class="field-label">{{ t('context.fields.locations') }}:</span>
          <div class="tag-list">
            <n-tag v-for="loc in contextProfile.locations" :key="loc" size="small" :bordered="false" class="wired-tag">
              {{ loc }}
            </n-tag>
          </div>
        </div>
        <div v-if="contextProfile.terminology.length > 0" class="summary-field">
          <span class="field-label">{{ t('context.fields.terminology') }}:</span>
          <div class="terminology-list">
            <div v-for="term in contextProfile.terminology" :key="term.source_term" class="term-row">
              <span class="term-source">{{ term.source_term }}</span>
              <span class="term-arrow">→</span>
              <span class="term-translated">{{ term.translated_term }}</span>
            </div>
          </div>
        </div>
        <div v-if="contextProfile.tone_notes" class="summary-field">
          <span class="field-label">{{ t('context.fields.tone') }}:</span>
          <span>{{ contextProfile.tone_notes }}</span>
        </div>
        <div v-if="contextProfile.plot_summary" class="summary-field">
          <span class="field-label">{{ t('context.fields.plot') }}:</span>
          <p class="plot-text">{{ contextProfile.plot_summary }}</p>
        </div>
      </div>

      <div v-else class="context-editor">
        <n-form-item :label="t('context.fields.title')">
          <n-input
            :value="contextProfile.title || ''"
            @update:value="(v: string) => updateField('title', v || null)"
            :placeholder="t('context.placeholders.title')"
          />
        </n-form-item>

        <n-divider class="wired-divider">{{ t('context.fields.characters') }}</n-divider>
        <div v-for="(char, idx) in contextProfile.characters" :key="idx" class="char-edit-row">
          <n-input
            :value="char.name"
            :placeholder="t('context.placeholders.name')"
            @update:value="(v: string) => updateCharacter(idx, 'name', v)"
          />
          <n-input
            :value="char.role || ''"
            :placeholder="t('context.placeholders.role')"
            @update:value="(v: string) => updateCharacter(idx, 'role', v || null)"
          />
          <n-input
            :value="char.description || ''"
            type="textarea"
            :rows="2"
            :placeholder="t('context.placeholders.description')"
            @update:value="(v: string) => updateCharacter(idx, 'description', v || null)"
          />
          <n-button quaternary type="error" size="small" @click="removeCharacter(idx)">
            <template #icon><n-icon><close-outline /></n-icon></template>
          </n-button>
        </div>
        <n-button size="small" dashed block @click="addCharacter">
          <template #icon><n-icon><add-outline /></n-icon></template>
          {{ t('context.addCharacter') }}
        </n-button>

        <n-divider class="wired-divider">{{ t('context.fields.terminology') }}</n-divider>
        <div v-for="(term, idx) in contextProfile.terminology" :key="idx" class="term-edit-row">
          <n-input
            :value="term.source_term"
            :placeholder="t('context.placeholders.sourceTerm')"
            @update:value="(v: string) => updateTerminology(idx, 'source_term', v)"
          />
          <n-input
            :value="term.translated_term"
            :placeholder="t('context.placeholders.translatedTerm')"
            @update:value="(v: string) => updateTerminology(idx, 'translated_term', v)"
          />
          <n-button quaternary type="error" size="small" @click="removeTerminology(idx)">
            <template #icon><n-icon><close-outline /></n-icon></template>
          </n-button>
        </div>
        <n-button size="small" dashed block @click="addTerminology">
          <template #icon><n-icon><add-outline /></n-icon></template>
          {{ t('context.addTerm') }}
        </n-button>

        <n-divider class="wired-divider">{{ t('context.fields.tone') }}</n-divider>
        <n-input
          :value="contextProfile.tone_notes || ''"
          type="textarea"
          :rows="2"
          :placeholder="t('context.placeholders.tone')"
          @update:value="(v: string) => updateField('tone_notes', v || null)"
        />

        <n-divider class="wired-divider">{{ t('context.fields.plot') }}</n-divider>
        <n-input
          :value="contextProfile.plot_summary || ''"
          type="textarea"
          :rows="4"
          :placeholder="t('context.placeholders.plot')"
          @update:value="(v: string) => updateField('plot_summary', v || null)"
        />

        <n-divider class="wired-divider">{{ t('context.fields.customInstructions') }}</n-divider>
        <n-input
          :value="contextProfile.custom_instructions || ''"
          type="textarea"
          :rows="3"
          :placeholder="t('context.placeholders.customInstructions')"
          @update:value="(v: string) => updateField('custom_instructions', v || null)"
        />
      </div>
    </div>

    <div v-if="isExtracting" class="context-extracting">
      <n-spin size="small" />
      <span>{{ extractionStatus || t('context.extracting') }}</span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  NButton,
  NIcon,
  NCheckbox,
  NTag,
  NInput,
  NFormItem,
  NDivider,
  NSpin,
  NPopconfirm
} from 'naive-ui'
import {
  BulbOutline,
  SparklesOutline,
  FolderOpenOutline,
  CreateOutline,
  RefreshOutline,
  SaveOutline,
  TrashOutline,
  CloseOutline,
  AddOutline
} from '@vicons/ionicons5'
import { open } from '@tauri-apps/plugin-dialog'
import type { CharacterProfile, ContextProfile, TerminologyEntry } from '../types/domain'

const props = defineProps<{
  contextProfile: ContextProfile | null
  isExtracting: boolean
  extractionStatus: string
  contextEnabled: boolean
  canExtract: boolean
}>()

const emit = defineEmits<{
  (e: 'toggle', value: boolean): void
  (e: 'clear'): void
  (e: 'update', profile: ContextProfile): void
  (e: 'extract'): void
  (e: 'save-file'): void
  (e: 'load-file', path: string): void
}>()

const { t } = useI18n()
const showEditor = ref(false)

const toggleContext = (value: boolean) => emit('toggle', value)
const clearProfile = () => emit('clear')
const onExtractFromFirstFile = () => emit('extract')
const onSaveToFile = () => emit('save-file')

const onLoadFromFile = async () => {
  const selected = await open({
    multiple: false,
    title: t('context.loadFromFile'),
    filters: [{ name: 'JSON', extensions: ['json'] }]
  })
  if (selected && typeof selected === 'string') {
    emit('load-file', selected)
  }
}

const updateField = (field: keyof ContextProfile, value: string | null) => {
  if (!props.contextProfile) return
  emit('update', { ...props.contextProfile, [field]: value })
}

const updateCharacter = (idx: number, field: keyof CharacterProfile, value: string | null) => {
  if (!props.contextProfile) return
  const characters = [...props.contextProfile.characters]
  characters[idx] = { ...characters[idx], [field]: value }
  emit('update', { ...props.contextProfile, characters })
}

const removeCharacter = (idx: number) => {
  if (!props.contextProfile) return
  const characters = props.contextProfile.characters.filter((_, i) => i !== idx)
  emit('update', { ...props.contextProfile, characters })
}

const addCharacter = () => {
  if (!props.contextProfile) return
  const newChar: CharacterProfile = {
    name: '',
    role: null,
    description: null,
    relationships: null,
    verified: false
  }
  emit('update', {
    ...props.contextProfile,
    characters: [...props.contextProfile.characters, newChar]
  })
}

const updateTerminology = (idx: number, field: keyof TerminologyEntry, value: string) => {
  if (!props.contextProfile) return
  const terminology = [...props.contextProfile.terminology]
  terminology[idx] = { ...terminology[idx], [field]: value }
  emit('update', { ...props.contextProfile, terminology })
}

const removeTerminology = (idx: number) => {
  if (!props.contextProfile) return
  const terminology = props.contextProfile.terminology.filter((_, i) => i !== idx)
  emit('update', { ...props.contextProfile, terminology })
}

const addTerminology = () => {
  if (!props.contextProfile) return
  const newTerm: TerminologyEntry = {
    source_term: '',
    translated_term: '',
    notes: null
  }
  emit('update', {
    ...props.contextProfile,
    terminology: [...props.contextProfile.terminology, newTerm]
  })
}
</script>

<style scoped>
.context-panel {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.context-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 12px;
}

.context-header h4 {
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.01em;
  line-height: 1.2;
}

.context-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
}

.context-empty {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.context-empty-hint {
  font-size: 13px;
  color: var(--text-secondary);
  margin: 0;
  line-height: 1.5;
}

.context-content {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.context-summary {
  display: flex;
  flex-direction: column;
  gap: 12px;
  font-size: 13px;
}

.summary-field {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.field-label {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.terminology-list {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.term-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
}

.term-source {
  color: var(--text-muted);
}

.term-arrow {
  color: var(--text-primary);
  font-weight: 700;
}

.term-translated {
  color: var(--text-primary);
  font-weight: 700;
}

.plot-text {
  margin: 0;
  font-size: 13px;
  line-height: 1.5;
  color: var(--text-secondary);
}

.context-editor {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.char-edit-row,
.term-edit-row {
  display: flex;
  gap: 8px;
  align-items: flex-start;
}

.term-edit-row {
  align-items: center;
}

.verified-tag {
  border: 1px solid var(--success) !important;
}

.media-match-field {
  padding: 12px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-inset);
}

.media-match-info {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
}

.media-title {
  font-weight: 700;
  font-size: 14px;
  color: var(--text-primary);
  letter-spacing: -0.01em;
}

.media-provider {
  font-size: 10.5px;
  color: var(--text-muted);
  font-family: var(--font-mono);
}

.context-extracting {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  color: var(--text-secondary);
}

.checkbox-label-content {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}

.eyebrow {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--text-muted);
  margin: 0 0 4px 0;
}
</style>
