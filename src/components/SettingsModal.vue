<template>
  <n-modal
    v-model:show="showModal"
    preset="card"
    class="settings-modal"
    :style="{ width: 'min(860px, calc(100vw - 28px))' }"
    :title="t('settings.title')"
    :bordered="false"
    size="huge"
    :segmented="{ content: true, footer: 'soft' }"
  >
    <n-tabs type="line" animated class="settings-tabs">
      <n-tab-pane name="interface" :tab="t('settings.interfaceTab')">
        <n-space vertical size="large">
          <n-form-item :label="t('settings.interfaceLanguage')" label-placement="left">
            <n-select
              v-model:value="settings.interfaceLanguage"
              :options="interfaceLanguageSelectOptions"
            />
          </n-form-item>
        </n-space>
      </n-tab-pane>

      <n-tab-pane name="api" :tab="t('settings.apiTab')">
        <n-space vertical size="large">
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
              <n-space vertical>
                <n-text depth="3" style="font-size: 12px;">
                  {{ t('settings.providerPresetsDescription') }}
                </n-text>
                <n-space>
                  <n-button size="small" @click="setPreset('openai')">OpenAI</n-button>
                  <n-button size="small" @click="setPreset('gemini')">Gemini</n-button>
                  <n-button size="small" @click="setPreset('ollama')">Ollama</n-button>
                  <n-button size="small" @click="setPreset('lmstudio')">LM Studio</n-button>
                  <n-button size="small" @click="setPreset('llamacpp')">llama.cpp</n-button>
                  <n-button size="small" @click="setPreset('openrouter')">OpenRouter</n-button>
                  <n-button size="small" @click="setPreset('nvidia')">NVIDIA NIM</n-button>
                </n-space>
              </n-space>
            </n-collapse-item>
          </n-collapse>
        </n-space>
      </n-tab-pane>

      <n-tab-pane name="translation" :tab="t('settings.translationTab')">
        <n-space vertical size="large">
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

          <n-form-item :label="t('settings.translationStyle')" label-placement="left">
            <n-select
              v-model:value="settings.translationStyle"
              :options="styleOptions"
            />
          </n-form-item>

          <n-collapse>
            <n-collapse-item :title="t('settings.systemPromptPreview')" name="prompt">
              <n-input
                :value="getSystemPrompt()"
                type="textarea"
                readonly
                :rows="8"
                style="font-family: monospace; font-size: 12px;"
              />
            </n-collapse-item>
          </n-collapse>
        </n-space>
      </n-tab-pane>

      <n-tab-pane name="output" :tab="t('settings.outputTab')">
        <n-space vertical size="large">
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
            <n-space vertical>
              <n-checkbox v-model:checked="settings.autoBackup">
                {{ t('settings.autoBackup') }}
              </n-checkbox>
              <n-checkbox v-model:checked="settings.keepOriginalTrack">
                {{ t('settings.keepOriginalTrack') }}
              </n-checkbox>
            </n-space>
          </n-form-item>
        </n-space>
      </n-tab-pane>
    </n-tabs>

    <template #footer>
      <n-space justify="end">
        <n-button @click="resetSettings">{{ t('settings.reset') }}</n-button>
        <n-button type="primary" @click="saveSettings">
          <template #icon>
            <n-icon><save-outline /></n-icon>
          </template>
          {{ t('settings.saveSettings') }}
        </n-button>
      </n-space>
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
  NSpace,
  NFormItem,
  NInput,
  NInputGroup,
  NSelect,
  NButton,
  NIcon,
  NCollapse,
  NCollapseItem,
  NText,
  NCheckbox,
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
  setInterfaceLocale,
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

const styleOptions = computed(() => [
  { label: t('styles.natural'), value: 'natural' },
  { label: t('styles.literal'), value: 'literal' },
  { label: t('styles.localized'), value: 'localized' },
  { label: t('styles.formal'), value: 'formal' },
  { label: t('styles.casual'), value: 'casual' },
  { label: t('styles.honorifics'), value: 'honorifics' }
])

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
    ? t('prompts.sourceLanguage', {
      language: languageOptions.value.find(l => l.value === settings.sourceLanguage)?.label
        || settings.sourceLanguage
    })
    : t('prompts.detectSourceLanguage')
  
  const promptKey = `prompts.${settings.translationStyle}`
  return t(promptKey, { targetLang: langName, context })
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
  await loadProviderApiKey(provider)
}

const setPreset = async (provider: string) => {
  settings.provider = provider
  await onProviderChange(provider)
  message.info(t('settings.configuredFor', {
    provider: providerOptions.value.find(p => p.value === provider)?.label || provider
  }))
}

// Load settings from localStorage on mount
const loadSettings = async () => {
  const saved = localStorage.getItem(SETTINGS_STORAGE_KEY)
  if (saved) {
    try {
      const parsed = JSON.parse(saved)
      Object.assign(settings, normalizeSettings({ ...parsed, apiKey: '' }))
      // Load cached models for provider
      const cachedModels = localStorage.getItem(`animesubs-models-${settings.provider}`)
      if (cachedModels) {
        modelOptions.value = JSON.parse(cachedModels)
      }
    } catch (e) {
      console.error('Failed to load settings:', e)
    }
  }
  await loadProviderApiKey(settings.provider)
}

onMounted(() => {
  void loadSettings()
})

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
    // Use Tauri backend to fetch models (bypasses CORS for local/custom APIs)
    const models = await invokeFetchModels(
      settings.apiEndpoint,
      settings.apiKey,
      settings.provider
    )
    modelOptions.value = models
    localStorage.setItem(`animesubs-models-${settings.provider}`, JSON.stringify(models))
    message.success(t('settings.loadedModels', { count: models.length }))
  } catch (error) {
    message.error(t('settings.failedToFetchModels', { error }))
    // Fall back to preset models
    const preset = providerPresets[settings.provider]
    if (preset?.models.length) {
      modelOptions.value = preset.models.map(m => ({ label: m, value: m }))
    }
  } finally {
    loadingModels.value = false
  }
}

const selectOutputDir = async () => {
  const selected = await open({
    directory: true,
    multiple: false,
    title: t('settings.selectOutputDirectory')
  })
  
  if (selected) {
    settings.outputDirectory = selected as string
  }
}

const selectFfmpegPath = async () => {
  const selected = await open({
    multiple: false,
    title: t('settings.selectFfmpegExecutable')
  })
  
  if (selected) {
    settings.ffmpegPath = selected as string
  }
}

const saveSettings = async () => {
  settings.hasSelectedInterfaceLanguage = true
  await saveApiKey(settings.provider, settings.apiKey)
  localStorage.setItem(
    SETTINGS_STORAGE_KEY,
    JSON.stringify(settingsForStorage(settings))
  )
  message.success(t('settings.settingsSaved'))
  showModal.value = false
}

const resetSettings = () => {
  Object.assign(settings, defaultSettings)
  modelOptions.value = []
  message.info(t('settings.settingsReset'))
}

watch(() => settings.interfaceLanguage, (language) => {
  setInterfaceLocale(language)
})

defineExpose({ settings, getSystemPrompt })
</script>

<style scoped>
.settings-modal {
  font-family: var(--font-body, "Avenir Next", "Segoe UI", sans-serif);
}

.settings-modal :deep(.n-card) {
  color: var(--wired-paper, #c8bd98);
  border: 1px solid var(--wired-border-strong, rgba(224, 212, 168, 0.42));
  border-radius: 0;
  background:
    linear-gradient(180deg, rgba(28, 21, 29, 0.98), rgba(6, 5, 6, 0.98)),
    radial-gradient(circle at 12% 0%, rgba(181, 68, 56, 0.18), transparent 20rem);
  box-shadow: 0 30px 90px rgba(0, 0, 0, 0.72);
}

.settings-modal :deep(.n-card-header) {
  border-bottom: 1px solid var(--wired-border, rgba(200, 189, 152, 0.22));
  background:
    repeating-linear-gradient(90deg, rgba(200, 189, 152, 0.04) 0 1px, transparent 1px 12px),
    rgba(3, 3, 3, 0.36);
}

.settings-modal :deep(.n-card-header__main) {
  color: var(--wired-paper-bright, #e0d4a8);
  font-family: var(--font-wired, ui-monospace, monospace);
  font-size: 14px;
  letter-spacing: 0.14em;
  text-transform: uppercase;
}

.settings-modal :deep(.n-card__content) {
  padding: 22px;
}

.settings-modal :deep(.n-card__footer) {
  border-top: 1px solid var(--wired-border, rgba(200, 189, 152, 0.22));
  background: rgba(3, 3, 3, 0.26);
}

.settings-tabs :deep(.n-tabs-tab__label),
.settings-modal :deep(.n-form-item-label__text),
.settings-modal :deep(.n-collapse-item__header-main) {
  color: var(--wired-muted, #8d8064);
  font-family: var(--font-wired, ui-monospace, monospace);
  font-size: 11px;
  letter-spacing: 0.08em;
  text-transform: uppercase;
}

.settings-tabs :deep(.n-tabs-tab.n-tabs-tab--active .n-tabs-tab__label) {
  color: var(--wired-paper-bright, #e0d4a8);
}

.settings-tabs :deep(.n-tabs-bar) {
  background: var(--wired-red, #b54438);
}

.settings-modal :deep(.n-button) {
  border-radius: 0;
  font-family: var(--font-wired, ui-monospace, monospace);
  letter-spacing: 0.05em;
}

.settings-modal :deep(.n-button--primary-type) {
  color: var(--wired-black, #030303) !important;
  font-weight: 900;
  background: var(--wired-paper, #c8bd98) !important;
  border-color: var(--wired-paper, #c8bd98) !important;
  box-shadow: 5px 5px 0 var(--wired-red-dark, #4d1716);
}

.settings-modal :deep(.n-base-selection),
.settings-modal :deep(.n-input),
.settings-modal :deep(.n-input-number),
.settings-modal :deep(.n-input-number .n-input) {
  border-radius: 0 !important;
  background: rgba(3, 3, 3, 0.42) !important;
}

.settings-modal :deep(.n-base-selection .n-base-selection-label),
.settings-modal :deep(.n-input-wrapper),
.settings-modal :deep(.n-input__textarea-el),
.settings-modal :deep(.n-input__input-el) {
  color: var(--wired-paper, #c8bd98) !important;
  font-family: var(--font-wired, ui-monospace, monospace) !important;
}

.settings-modal :deep(.n-collapse),
.settings-modal :deep(.n-collapse-item) {
  border-color: var(--wired-border, rgba(200, 189, 152, 0.22));
}

.settings-modal :deep(.n-checkbox__label),
.settings-modal :deep(.n-text) {
  color: var(--wired-paper, #c8bd98) !important;
  font-family: var(--font-wired, ui-monospace, monospace);
}

.settings-modal :deep(.n-divider) {
  --n-color: var(--wired-border, rgba(200, 189, 152, 0.22));
}

@media (max-width: 720px) {
  .settings-modal :deep(.n-card__content) {
    padding: 16px;
  }
}
</style>
