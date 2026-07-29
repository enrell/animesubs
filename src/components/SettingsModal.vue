<template>
  <n-modal
    v-model:show="showModal"
    preset="card"
    class="settings-modal"
    :style="{ width: 'min(860px, calc(100vw - 28px))', height: '680px', maxHeight: '680px', display: 'flex', flexDirection: 'column' }"
    :title="t('settings.title')"
    :bordered="false"
    :auto-focus="false"
    :transition-name="''"
    :show-icon="false"
    size="huge"
    :segmented="{ content: true, footer: 'soft' }"
  >
    <n-tabs type="line" class="settings-tabs">
      <n-tab-pane name="interface" :tab="t('settings.interfaceTab')">
        <div class="settings-stack">
          <n-form-item :label="t('settings.interfaceLanguage')" label-placement="left">
            <n-select
              v-model:value="settings.interfaceLanguage"
              :options="interfaceLanguageSelectOptions"
            />
          </n-form-item>
        </div>
      </n-tab-pane>

      <n-tab-pane name="api" :tab="t('settings.apiTab')">
        <div class="settings-stack">
          <n-form-item :label="t('settings.provider')" label-placement="left">
            <n-select
              v-model:value="settings.provider"
              :options="providerOptions"
              @update:value="onProviderChange"
            />
          </n-form-item>

          <n-form-item :label="t('settings.apiEndpoint')" label-placement="left">
            <n-input
              v-model:value="settings.apiEndpoint"
              :placeholder="getEndpointPlaceholder()"
              clearable
            />
          </n-form-item>

          <n-form-item v-if="settings.provider !== 'ollama'" :label="t('settings.apiKey')" label-placement="left">
            <n-input
              v-model:value="settings.apiKey"
              type="password"
              show-password-on="click"
              :placeholder="getApiKeyPlaceholder()"
              clearable
            />
          </n-form-item>

          <n-form-item :label="t('settings.tmdbApiKey')" label-placement="left">
            <n-input
              v-model:value="settings.tmdbApiKey"
              type="password"
              show-password-on="click"
              :placeholder="t('settings.optional')"
              clearable
            />
          </n-form-item>

          <n-form-item :label="t('settings.model')" label-placement="left">
            <n-input-group>
              <n-select
                v-model:value="settings.selectedModel"
                :options="modelOptions"
                :loading="loadingModels"
                :placeholder="t('settings.selectModel')"
                filterable
                tag
                style="flex: 1"
              />
              <n-button
                type="primary"
                ghost
                :loading="loadingModels"
                @click="fetchModels"
              >
                <template #icon>
                  <n-icon><refresh-outline /></n-icon>
                </template>
              </n-button>
            </n-input-group>
          </n-form-item>

          <n-collapse>
            <n-collapse-item :title="t('settings.providerPresets')" name="presets">
              <div class="settings-stack compact">
                <n-text depth="3" style="font-size: 12px;">
                  {{ t('settings.providerPresetsDescription') }}
                </n-text>
                <div class="provider-preset-grid">
                  <n-button size="small" @click="setPreset('openai')">OpenAI</n-button>
                  <n-button size="small" @click="setPreset('gemini')">Gemini</n-button>
                  <n-button size="small" @click="setPreset('ollama')">Ollama</n-button>
                  <n-button size="small" @click="setPreset('lmstudio')">LM Studio</n-button>
                  <n-button size="small" @click="setPreset('llamacpp')">llama.cpp</n-button>
                  <n-button size="small" @click="setPreset('openrouter')">OpenRouter</n-button>
                  <n-button size="small" @click="setPreset('nvidia')">NVIDIA NIM</n-button>
                </div>
              </div>
            </n-collapse-item>
          </n-collapse>
        </div>
      </n-tab-pane>

      <n-tab-pane name="translation" :tab="t('settings.translationTab')">
        <div class="settings-stack">
          <n-form-item :label="t('settings.sourceLanguage')" label-placement="left">
            <n-select
              v-model:value="settings.sourceLanguage"
              :options="languageOptions"
              :placeholder="t('settings.autoDetect')"
            />
          </n-form-item>

          <n-form-item :label="t('settings.targetLanguage')" label-placement="left">
            <n-select
              v-model:value="settings.targetLanguage"
              :options="languageOptions"
              :placeholder="t('settings.selectTargetLanguage')"
            />
          </n-form-item>

          <n-divider />

          <n-form-item :label="t('settings.batchSize')" label-placement="left">
            <n-input-number
              v-model:value="settings.batchSize"
              :min="10"
              :max="2000"
              :step="10"
              style="width: 100%"
            />
            <template #feedback>
              <n-text depth="3" style="font-size: 11px;">
                {{ t('settings.batchSizeHint') }}
              </n-text>
            </template>
          </n-form-item>

          <n-form-item :label="t('settings.concurrency')" label-placement="left">
            <n-input-number
              v-model:value="settings.concurrency"
              :min="1"
              :max="20"
              :step="1"
              style="width: 100%"
            />
            <template #feedback>
              <n-text depth="3" style="font-size: 11px;">
                {{ t('settings.concurrencyHint') }}
              </n-text>
            </template>
          </n-form-item>

          <n-form-item :label="t('settings.autoExtractContext')" label-placement="left">
            <n-switch v-model:value="settings.autoExtractContext" />
            <template #feedback>
              <n-text depth="3" style="font-size: 11px;">
                {{ t('settings.autoExtractContextHint') }}
              </n-text>
            </template>
          </n-form-item>

          <n-collapse>
            <n-collapse-item :title="t('settings.systemPromptPreview')" name="prompt">
              <n-input
                :value="systemPromptModel"
                @update:value="onSystemPromptChange"
                type="textarea"
                :rows="12"
                style="font-family: var(--font-mono, monospace); font-size: 12px;"
              />
              <n-text depth="3" style="font-size: 11px; margin-top: 4px; display: block;">
                {{ t('settings.customSystemPromptHint') }}
              </n-text>
            </n-collapse-item>
          </n-collapse>
        </div>
      </n-tab-pane>

      <n-tab-pane name="output" :tab="t('settings.outputTab')">
        <div class="settings-stack">
          <n-form-item :label="t('settings.outputDirectory')" label-placement="left">
            <n-input-group>
              <n-input
                v-model:value="settings.outputDirectory"
                :placeholder="t('settings.sameAsInput')"
                readonly
              />
              <n-button type="primary" ghost @click="selectOutputDir">
                <template #icon>
                  <n-icon><folder-open-outline /></n-icon>
                </template>
              </n-button>
            </n-input-group>
          </n-form-item>

          <n-form-item :label="t('settings.outputFormat')" label-placement="left">
            <n-select
              v-model:value="settings.outputFormat"
              :options="formatOptions"
            />
          </n-form-item>

          <n-form-item :label="t('settings.ffmpegPath')" label-placement="left">
            <n-input-group>
              <n-input
                v-model:value="settings.ffmpegPath"
                :placeholder="t('settings.ffmpegPathPlaceholder')"
                clearable
              />
              <n-button type="primary" ghost @click="selectFfmpegPath">
                <template #icon>
                  <n-icon><folder-open-outline /></n-icon>
                </template>
              </n-button>
            </n-input-group>
          </n-form-item>

          <n-divider />

          <n-form-item :label="t('settings.backupSettings')" label-placement="top">
            <div class="settings-stack compact">
              <n-checkbox v-model:checked="settings.autoBackup">
                {{ t('settings.autoBackup') }}
              </n-checkbox>
              <n-checkbox v-model:checked="settings.keepOriginalTrack">
                {{ t('settings.keepOriginalTrack') }}
              </n-checkbox>
            </div>
          </n-form-item>
        </div>
      </n-tab-pane>
    </n-tabs>

    <template #footer>
      <div class="settings-footer">
        <n-button @click="resetSettings">{{ t('settings.reset') }}</n-button>
        <n-button type="primary" @click="saveSettings">
          <template #icon>
            <n-icon><save-outline /></n-icon>
          </template>
          {{ t('settings.saveSettings') }}
        </n-button>
      </div>
    </template>
  </n-modal>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  NModal,
  NTabs,
  NTabPane,
  NFormItem,
  NInput,
  NInputGroup,
  NInputNumber,
  NSelect,
  NButton,
  NIcon,
  NCollapse,
  NCollapseItem,
  NText,
  NCheckbox,
  NSwitch,
  NDivider,
  useMessage
} from 'naive-ui'
import {
  RefreshOutline,
  FolderOpenOutline,
  SaveOutline
} from '@vicons/ionicons5'
import { open } from '@tauri-apps/plugin-dialog'
import { loadApiKey, saveApiKey, fetchModels as invokeFetchModels } from '../api/animesubs'
import {
  defaultSettings,
  normalizeSettings,
  providerRequiresApiKey,
  SETTINGS_STORAGE_KEY,
  settingsForStorage,
  sharedLanguageOptions,
  type Settings
} from '../config/settings'
import {
  interfaceLanguageOptions,
  translationLanguageKey
} from '../i18n'

const props = defineProps<{
  show: boolean
}>()

const emit = defineEmits<{
  (e: 'update:show', value: boolean): void
}>()

const message = useMessage()
const { t } = useI18n()

const showModal = computed({
  get: () => props.show,
  set: (value) => emit('update:show', value)
})

const loadingModels = ref(false)
const modelOptions = ref<{ label: string; value: string }[]>([])

const settings = reactive<Settings>({ ...defaultSettings })

const providerOptions = computed(() => [
  { label: 'OpenAI', value: 'openai' },
  { label: 'Google Gemini', value: 'gemini' },
  { label: t('settings.providerLocal', { provider: 'Ollama' }), value: 'ollama' },
  { label: t('settings.providerLocal', { provider: 'LM Studio' }), value: 'lmstudio' },
  { label: t('settings.providerLocal', { provider: 'llama.cpp' }), value: 'llamacpp' },
  { label: 'OpenRouter', value: 'openrouter' },
  { label: 'NVIDIA NIM', value: 'nvidia' },
  { label: t('settings.minimaxTokenPlan'), value: 'minimax' },
  { label: t('settings.customOpenAICompatible'), value: 'custom' }
])

const providerPresets: Record<string, { endpoint: string; models: string[] }> = {
  openai: {
    endpoint: 'https://api.openai.com/v1',
    models: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'gpt-3.5-turbo']
  },
  gemini: {
    endpoint: 'https://generativelanguage.googleapis.com/v1beta/openai',
    models: ['gemini-2.0-flash-exp', 'gemini-1.5-pro', 'gemini-1.5-flash', 'gemini-1.5-flash-8b']
  },
  ollama: {
    endpoint: 'http://localhost:11434/v1',
    models: ['llama3.2', 'llama3.1', 'mistral', 'qwen2.5', 'gemma2']
  },
  lmstudio: {
    endpoint: 'http://localhost:1234/v1',
    models: []
  },
  llamacpp: {
    endpoint: 'http://localhost:8080/v1',
    models: ['local-model']
  },
  openrouter: {
    endpoint: 'https://openrouter.ai/api/v1',
    models: ['anthropic/claude-3.5-sonnet', 'openai/gpt-4o', 'google/gemini-pro-1.5', 'meta-llama/llama-3.1-70b-instruct']
  },
  nvidia: {
    endpoint: 'https://integrate.api.nvidia.com/v1',
    models: ['google/gemma-4-31b-it', 'minimaxai/minimax-m2.7', 'moonshotai/kimi-k2.5', 'moonshotai/kimi-k2-thinking', 'nvidia/llama-3.1-nemotron-70b-instruct']
  },
  minimax: {
    endpoint: 'https://api.minimax.io/v1',
    models: ['MiniMax-M2.7', 'MiniMax-M2.5', 'MiniMax-M1']
  },
  custom: {
    endpoint: '',
    models: []
  }
}

const languageOptions = computed(() => {
  return sharedLanguageOptions.map(option => ({
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

const formatOptions = computed(() => [
  { label: t('formats.auto'), value: '' },
  { label: t('formats.srt'), value: 'srt' },
  { label: t('formats.ass'), value: 'ass' },
  { label: t('formats.vtt'), value: 'vtt' }
])

const getSystemPrompt = (): string => {
  const langName = languageOptions.value.find(l => l.value === settings.targetLanguage)?.label
    || settings.targetLanguage
  const context = settings.sourceLanguage
    ? `Source language: ${languageOptions.value.find(l => l.value === settings.sourceLanguage)?.label || settings.sourceLanguage}`
    : 'Detect the source language automatically.'

  return `You are a professional subtitle translator. Translate the following subtitle lines to ${langName}.

Guidelines:
- Be faithful to the original meaning while producing natural, fluent translations that read like native dialogue in the target language
- Adapt idioms, jokes, wordplay, and cultural references to their closest natural equivalent so the audience experiences the same intent and emotion
- Preserve the tone, register, and emotional weight of each line: a formal character stays formal, a comedic line stays comedic, a tender moment stays tender
- Keep character names in their original form unless a well-known localized version exists. Preserve Japanese honorifics (-san, -kun, -chan, -sama, -sensei, -senpai) and culturally specific terms that lack direct equivalents
- Maintain subtitle readability: keep lines concise, preserve line breaks, and ensure the text can be read in the time it appears on screen
- If context metadata is provided (characters, terminology, synopsis), use it to keep names, terms, and tone consistent across episodes
- Do NOT add explanations, notes, or commentary, only the translated text

${context}`
}

const systemPromptModel = computed({
  get: () => settings.customSystemPrompt || getSystemPrompt(),
  set: (value: string) => {
    settings.customSystemPrompt = value
  }
})

const onSystemPromptChange = (value: string) => {
  settings.customSystemPrompt = value
}

const getEndpointPlaceholder = (): string => {
  return providerPresets[settings.provider]?.endpoint || 'https://api.example.com/v1'
}

const getApiKeyPlaceholder = (): string => {
  const placeholders: Record<string, string> = {
    openai: 'sk-...',
    gemini: 'AIza...',
    openrouter: 'sk-or-...',
    nvidia: 'nvapi-...',
    minimax: t('settings.bearerToken'),
    lmstudio: t('settings.optional'),
    llamacpp: t('settings.optional'),
    custom: t('settings.apiKeyPlaceholder')
  }
  return placeholders[settings.provider] || t('settings.apiKeyPlaceholder')
}

const loadProviderApiKey = async (provider: string) => {
  try {
    const result = await loadApiKey(provider)
    settings.apiKey = result.data || ''
  } catch (e) {
    console.error('Failed to load API key:', e)
    settings.apiKey = ''
  }
}

const onProviderChange = async (provider: string) => {
  const preset = providerPresets[provider]
  if (preset) {
    settings.apiEndpoint = preset.endpoint
    if (preset.models.length > 0) {
      modelOptions.value = preset.models.map(m => ({ label: m, value: m }))
    }
  }
  if (provider !== 'ollama') {
    await loadProviderApiKey(provider)
  }
}

const fetchModels = async () => {
  if (!settings.apiEndpoint) {
    message.warning(t('settings.enterApiEndpointFirst'))
    return
  }
  if (providerRequiresApiKey(settings.provider) && !settings.apiKey) {
    message.warning(t('settings.enterApiKeyFirst'))
    return
  }

  loadingModels.value = true
  try {
    const models = await invokeFetchModels(settings.apiEndpoint, settings.apiKey)
    modelOptions.value = models || []
    if (models.length > 0) {
      message.success(t('settings.loadedModels', { count: models.length }))
    }
  } catch (e: any) {
    message.error(t('settings.failedToFetchModels', { error: e?.message || 'Unknown' }))
  } finally {
    loadingModels.value = false
  }
}

const selectOutputDir = async () => {
  const selected = await open({ directory: true, multiple: false })
  if (selected && typeof selected === 'string') {
    settings.outputDirectory = selected
  }
}

const selectFfmpegPath = async () => {
  const selected = await open({
    multiple: false,
    title: t('settings.selectFfmpegExecutable'),
    filters: [{ name: 'FFmpeg', extensions: ['ffmpeg', 'exe', ''] }]
  })
  if (selected && typeof selected === 'string') {
    settings.ffmpegPath = selected
  }
}

const setPreset = (provider: string) => {
  const preset = providerPresets[provider]
  if (preset) {
    settings.provider = provider
    settings.apiEndpoint = preset.endpoint
    if (preset.models.length > 0) {
      modelOptions.value = preset.models.map(m => ({ label: m, value: m }))
      settings.selectedModel = preset.models[0]
    }
  }
}

const loadSettings = () => {
  try {
    const stored = localStorage.getItem(SETTINGS_STORAGE_KEY)
    if (stored) {
      const parsed = JSON.parse(stored)
      Object.assign(settings, normalizeSettings(parsed))
    }
  } catch (e) {
    console.error('Failed to load settings:', e)
  }
}

const saveSettings = () => {
  try {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settingsForStorage(settings)))
    if (providerRequiresApiKey(settings.provider) && settings.apiKey) {
      saveApiKey(settings.provider, settings.apiKey)
    }
    message.success(t('settings.settingsSaved'))
    showModal.value = false
  } catch (e) {
    console.error('Failed to save settings:', e)
    message.error('Failed to save settings')
  }
}

const resetSettings = () => {
  Object.assign(settings, defaultSettings)
  modelOptions.value = []
  message.info(t('settings.settingsReset'))
}

const saveCurrentSettings = () => {
  localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settingsForStorage(settings)))
}

watch(settings, () => {
  saveCurrentSettings()
}, { deep: true })

onMounted(() => {
  loadSettings()
  if (settings.provider && settings.provider !== 'ollama') {
    loadProviderApiKey(settings.provider)
  }
  const preset = providerPresets[settings.provider]
  if (preset?.models.length) {
    modelOptions.value = preset.models.map(m => ({ label: m, value: m }))
  }
})

defineExpose({ settings, getSystemPrompt })
</script>

<style scoped>
.settings-modal {
  font-family: var(--font-sans);
}

.settings-modal.n-card,
.settings-modal :deep(.n-card) {
  border-radius: var(--radius-lg);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  box-shadow: var(--shadow-pop);
}

.settings-modal :deep(.n-card-header) {
  border-bottom: 1px solid var(--border-subtle);
}

.settings-modal :deep(.n-card-header__main) {
  color: var(--text-primary);
  font-size: 17px;
  font-weight: 800;
  letter-spacing: -0.01em;
}

.settings-modal :deep(.n-card__content) {
  padding: 24px !important;
  height: 520px !important;
  min-height: 520px !important;
  max-height: 520px !important;
  overflow-y: auto !important;
  box-sizing: border-box !important;
}

.settings-modal :deep(.n-card__footer) {
  border-top: 1px solid var(--border-subtle);
}

.settings-stack {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

.settings-stack.compact {
  gap: 10px;
}

.provider-preset-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.settings-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
}

.settings-tabs :deep(.n-tabs-tab__label) {
  font-size: 13px;
  font-weight: 600;
}

.settings-tabs :deep(.n-tabs-bar) {
  background: var(--accent);
}

.settings-modal :deep(.n-form-item-label__text),
.settings-modal :deep(.n-collapse-item__header-main) {
  font-size: 12.5px;
  font-weight: 600;
}

@media (max-width: 720px) {
  .settings-modal :deep(.n-card__content) {
    padding: 16px;
  }
}
</style>
