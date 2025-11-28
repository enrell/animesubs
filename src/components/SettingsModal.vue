<template>
  <n-modal
    v-model:show="showModal"
    preset="card"
    :style="{ width: '600px' }"
    title="Settings"
    :bordered="false"
    size="huge"
    :segmented="{ content: true, footer: 'soft' }"
  >
    <n-tabs type="line" animated>
      <n-tab-pane name="api" tab="API Configuration">
        <n-space vertical size="large">
          <n-form-item label="Provider" label-placement="left">
            <n-select
              v-model:value="settings.provider"
              :options="providerOptions"
              @update:value="onProviderChange"
            />
          </n-form-item>

          <n-form-item label="API Endpoint" label-placement="left">
            <n-input
              v-model:value="settings.apiEndpoint"
              :placeholder="getEndpointPlaceholder()"
              clearable
            />
          </n-form-item>
          
          <n-form-item v-if="settings.provider !== 'ollama'" label="API Key" label-placement="left">
            <n-input
              v-model:value="settings.apiKey"
              type="password"
              show-password-on="click"
              :placeholder="getApiKeyPlaceholder()"
              clearable
            />
          </n-form-item>

          <n-form-item label="Model" label-placement="left">
            <n-input-group>
              <n-select
                v-model:value="settings.selectedModel"
                :options="modelOptions"
                :loading="loadingModels"
                placeholder="Select a model"
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
            <n-collapse-item title="Provider Presets" name="presets">
              <n-space vertical>
                <n-text depth="3" style="font-size: 12px;">
                  Click to quickly configure popular providers:
                </n-text>
                <n-space>
                  <n-button size="small" @click="setPreset('openai')">OpenAI</n-button>
                  <n-button size="small" @click="setPreset('gemini')">Gemini</n-button>
                  <n-button size="small" @click="setPreset('ollama')">Ollama</n-button>
                  <n-button size="small" @click="setPreset('lmstudio')">LM Studio</n-button>
                  <n-button size="small" @click="setPreset('openrouter')">OpenRouter</n-button>
                </n-space>
              </n-space>
            </n-collapse-item>
          </n-collapse>
        </n-space>
      </n-tab-pane>

      <n-tab-pane name="translation" tab="Translation">
        <n-space vertical size="large">
          <n-form-item label="Source Language" label-placement="left">
            <n-select
              v-model:value="settings.sourceLanguage"
              :options="languageOptions"
              placeholder="Auto-detect"
            />
          </n-form-item>

          <n-form-item label="Target Language" label-placement="left">
            <n-select
              v-model:value="settings.targetLanguage"
              :options="languageOptions"
              placeholder="Select target language"
            />
          </n-form-item>

          <n-form-item label="Translation Style" label-placement="left">
            <n-select
              v-model:value="settings.translationStyle"
              :options="styleOptions"
            />
          </n-form-item>

          <n-form-item label="Context Lines" label-placement="left">
            <n-input-number
              v-model:value="settings.contextLines"
              :min="0"
              :max="10"
              placeholder="Lines of context for better translation"
            />
          </n-form-item>

          <n-collapse>
            <n-collapse-item title="System Prompt Preview" name="prompt">
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

      <n-tab-pane name="output" tab="Output">
        <n-space vertical size="large">
          <n-form-item label="Output Directory" label-placement="left">
            <n-input-group>
              <n-input
                v-model:value="settings.outputDirectory"
                placeholder="Same as input"
                readonly
              />
              <n-button type="primary" ghost @click="selectOutputDir">
                <template #icon>
                  <n-icon><folder-open-outline /></n-icon>
                </template>
              </n-button>
            </n-input-group>
          </n-form-item>

          <n-form-item label="Output Format" label-placement="left">
            <n-select
              v-model:value="settings.outputFormat"
              :options="formatOptions"
            />
          </n-form-item>

          <n-form-item label="FFmpeg Path" label-placement="left">
            <n-input-group>
              <n-input
                v-model:value="settings.ffmpegPath"
                placeholder="ffmpeg (uses PATH)"
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

          <n-form-item label="Backup Settings" label-placement="top">
            <n-space vertical>
              <n-checkbox v-model:checked="settings.autoBackup">
                Automatically backup subtitles before translation
              </n-checkbox>
              <n-checkbox v-model:checked="settings.keepOriginalTrack">
                Keep original subtitle track in video
              </n-checkbox>
            </n-space>
          </n-form-item>
        </n-space>
      </n-tab-pane>
    </n-tabs>

    <template #footer>
      <n-space justify="end">
        <n-button @click="resetSettings">Reset</n-button>
        <n-button type="primary" @click="saveSettings">
          <template #icon>
            <n-icon><save-outline /></n-icon>
          </template>
          Save Settings
        </n-button>
      </n-space>
    </template>
  </n-modal>
</template>

<script setup lang="ts">
import { ref, reactive, computed } from 'vue'
import {
  NModal,
  NTabs,
  NTabPane,
  NSpace,
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
  NDivider,
  useMessage
} from 'naive-ui'
import {
  RefreshOutline,
  FolderOpenOutline,
  SaveOutline
} from '@vicons/ionicons5'
import { open } from '@tauri-apps/plugin-dialog'

export interface Settings {
  provider: string
  apiEndpoint: string
  apiKey: string
  selectedModel: string | null
  sourceLanguage: string
  targetLanguage: string
  translationStyle: string
  contextLines: number
  outputDirectory: string
  outputFormat: string
  ffmpegPath: string
  autoBackup: boolean
  keepOriginalTrack: boolean
}

const props = defineProps<{
  show: boolean
}>()

const emit = defineEmits<{
  (e: 'update:show', value: boolean): void
}>()

const message = useMessage()

const showModal = computed({
  get: () => props.show,
  set: (value) => emit('update:show', value)
})

const loadingModels = ref(false)
const modelOptions = ref<{ label: string; value: string }[]>([])

const settings = reactive<Settings>({
  provider: 'openai',
  apiEndpoint: 'https://api.openai.com/v1',
  apiKey: '',
  selectedModel: null,
  sourceLanguage: '',
  targetLanguage: 'en',
  translationStyle: 'natural',
  contextLines: 2,
  outputDirectory: '',
  outputFormat: 'srt',
  ffmpegPath: '',
  autoBackup: true,
  keepOriginalTrack: true
})

const providerOptions = [
  { label: 'OpenAI', value: 'openai' },
  { label: 'Google Gemini', value: 'gemini' },
  { label: 'Ollama (Local)', value: 'ollama' },
  { label: 'LM Studio (Local)', value: 'lmstudio' },
  { label: 'OpenRouter', value: 'openrouter' },
  { label: 'Custom OpenAI-compatible', value: 'custom' }
]

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
  openrouter: {
    endpoint: 'https://openrouter.ai/api/v1',
    models: ['anthropic/claude-3.5-sonnet', 'openai/gpt-4o', 'google/gemini-pro-1.5', 'meta-llama/llama-3.1-70b-instruct']
  },
  custom: {
    endpoint: '',
    models: []
  }
}

const languageOptions = [
  { label: 'Auto-detect', value: '' },
  { label: 'Japanese', value: 'ja' },
  { label: 'English', value: 'en' },
  { label: 'Chinese (Simplified)', value: 'zh-CN' },
  { label: 'Chinese (Traditional)', value: 'zh-TW' },
  { label: 'Korean', value: 'ko' },
  { label: 'Spanish', value: 'es' },
  { label: 'French', value: 'fr' },
  { label: 'German', value: 'de' },
  { label: 'Portuguese', value: 'pt' },
  { label: 'Russian', value: 'ru' },
  { label: 'Italian', value: 'it' },
  { label: 'Arabic', value: 'ar' },
  { label: 'Thai', value: 'th' },
  { label: 'Vietnamese', value: 'vi' },
  { label: 'Indonesian', value: 'id' },
  { label: 'Polish', value: 'pl' },
  { label: 'Turkish', value: 'tr' }
]

const styleOptions = [
  { label: 'Natural & Fluent', value: 'natural' },
  { label: 'Literal Translation', value: 'literal' },
  { label: 'Localized (Cultural Adaptation)', value: 'localized' },
  { label: 'Formal', value: 'formal' },
  { label: 'Casual', value: 'casual' },
  { label: 'Honorifics Preserved', value: 'honorifics' }
]

const formatOptions = [
  { label: 'SRT (.srt)', value: 'srt' },
  { label: 'ASS/SSA (.ass)', value: 'ass' },
  { label: 'WebVTT (.vtt)', value: 'vtt' }
]

const systemPrompts: Record<string, string> = {
  natural: `You are an expert anime subtitle translator. Translate the following subtitle lines to {targetLang}.

Guidelines:
- Provide natural, fluent translations that sound like native speech
- Preserve the emotional tone and intent of the original dialogue
- Adapt idioms and expressions to their closest natural equivalent
- Keep character names in their original form unless there's a well-known localized version
- Maintain the pacing suitable for subtitle reading
- Do NOT add explanations or notes, only provide the translation

{context}`,

  literal: `You are a precise subtitle translator. Translate the following subtitle lines to {targetLang}.

Guidelines:
- Translate as literally as possible while maintaining grammatical correctness
- Preserve the original sentence structure when feasible
- Keep all names and terms in their original form
- Do not add or remove information from the original
- Do NOT add explanations or notes, only provide the translation

{context}`,

  localized: `You are a localization expert for anime subtitles. Translate and adapt the following lines to {targetLang}.

Guidelines:
- Adapt cultural references to equivalents the target audience will understand
- Convert measurements, currencies, and cultural concepts appropriately
- Rewrite jokes and wordplay to work in the target language
- Make dialogue feel natural for the target culture
- Preserve the overall story meaning and character relationships
- Do NOT add explanations or notes, only provide the translation

{context}`,

  formal: `You are a professional subtitle translator. Translate the following lines to {targetLang} using formal language.

Guidelines:
- Use formal register and polite language
- Avoid slang, contractions, and casual expressions
- Maintain professional and respectful tone
- Suitable for educational or professional contexts
- Do NOT add explanations or notes, only provide the translation

{context}`,

  casual: `You are a subtitle translator specializing in casual dialogue. Translate to {targetLang}.

Guidelines:
- Use casual, conversational language
- Include appropriate slang and colloquialisms
- Use contractions and informal expressions
- Match the relaxed tone of casual conversation
- Do NOT add explanations or notes, only provide the translation

{context}`,

  honorifics: `You are an anime subtitle translator who preserves Japanese honorifics. Translate to {targetLang}.

Guidelines:
- Keep Japanese honorifics (-san, -kun, -chan, -sama, -sensei, -senpai, etc.)
- Preserve name order (family name first if appropriate)
- Keep certain untranslatable terms (onii-chan, kawaii, etc.) with context clues
- Maintain the social relationship nuances through honorific usage
- Translate the rest naturally and fluently
- Do NOT add explanations or notes, only provide the translation

{context}`
}

const getSystemPrompt = (): string => {
  const langName = languageOptions.find(l => l.value === settings.targetLanguage)?.label || settings.targetLanguage
  const context = settings.sourceLanguage 
    ? `Source language: ${languageOptions.find(l => l.value === settings.sourceLanguage)?.label || settings.sourceLanguage}`
    : 'Detect the source language automatically.'
  
  return (systemPrompts[settings.translationStyle] || systemPrompts.natural)
    .replace('{targetLang}', langName)
    .replace('{context}', context)
}

const getEndpointPlaceholder = (): string => {
  return providerPresets[settings.provider]?.endpoint || 'https://api.example.com/v1'
}

const getApiKeyPlaceholder = (): string => {
  const placeholders: Record<string, string> = {
    openai: 'sk-...',
    gemini: 'AIza...',
    openrouter: 'sk-or-...',
    lmstudio: '(optional)',
    custom: 'API key'
  }
  return placeholders[settings.provider] || 'API key'
}

const onProviderChange = (provider: string) => {
  const preset = providerPresets[provider]
  if (preset) {
    settings.apiEndpoint = preset.endpoint
    if (preset.models.length > 0) {
      modelOptions.value = preset.models.map(m => ({ label: m, value: m }))
    }
  }
}

const setPreset = (provider: string) => {
  settings.provider = provider
  onProviderChange(provider)
  message.info(`Configured for ${providerOptions.find(p => p.value === provider)?.label}`)
}

// Load settings from localStorage on mount
const loadSettings = () => {
  const saved = localStorage.getItem('animesubs-settings')
  if (saved) {
    try {
      const parsed = JSON.parse(saved)
      Object.assign(settings, parsed)
      // Load cached models for provider
      const cachedModels = localStorage.getItem(`animesubs-models-${settings.provider}`)
      if (cachedModels) {
        modelOptions.value = JSON.parse(cachedModels)
      }
    } catch (e) {
      console.error('Failed to load settings:', e)
    }
  }
}

loadSettings()

const fetchModels = async () => {
  if (!settings.apiEndpoint) {
    message.warning('Please enter API endpoint first')
    return
  }

  if (settings.provider !== 'ollama' && !settings.apiKey) {
    message.warning('Please enter API key first')
    return
  }

  loadingModels.value = true
  try {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json'
    }
    
    if (settings.apiKey) {
      headers['Authorization'] = `Bearer ${settings.apiKey}`
    }

    // Handle different providers' model list endpoints
    let url = `${settings.apiEndpoint}/models`
    
    if (settings.provider === 'gemini') {
      // Gemini uses native API for listing models (not OpenAI-compatible endpoint)
      url = `https://generativelanguage.googleapis.com/v1beta/models?key=${settings.apiKey}`
      delete headers['Authorization']
    }

    const response = await fetch(url, { headers })
    
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${await response.text()}`)
    }
    
    const data = await response.json()
    
    // Handle different response formats
    let models: { label: string; value: string }[] = []
    
    if (data.data && Array.isArray(data.data)) {
      // OpenAI format
      models = data.data.map((m: any) => ({
        label: m.id,
        value: m.id
      }))
    } else if (data.models && Array.isArray(data.models)) {
      // Ollama format
      models = data.models.map((m: any) => ({
        label: m.name || m.model,
        value: m.name || m.model
      }))
    } else if (Array.isArray(data)) {
      // Gemini format
      models = data
        .filter((m: any) => m.name?.includes('gemini'))
        .map((m: any) => ({
          label: m.name.replace('models/', ''),
          value: m.name.replace('models/', '')
        }))
    }

    models.sort((a, b) => a.label.localeCompare(b.label))
    modelOptions.value = models
    
    // Cache models for this provider
    localStorage.setItem(`animesubs-models-${settings.provider}`, JSON.stringify(models))
    
    message.success(`Loaded ${models.length} models`)
  } catch (error) {
    message.error(`Failed to fetch models: ${error}`)
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
    title: 'Select Output Directory'
  })
  
  if (selected) {
    settings.outputDirectory = selected as string
  }
}

const selectFfmpegPath = async () => {
  const selected = await open({
    multiple: false,
    title: 'Select FFmpeg Executable'
  })
  
  if (selected) {
    settings.ffmpegPath = selected as string
  }
}

const saveSettings = () => {
  localStorage.setItem('animesubs-settings', JSON.stringify(settings))
  message.success('Settings saved')
  showModal.value = false
}

const resetSettings = () => {
  Object.assign(settings, {
    provider: 'openai',
    apiEndpoint: 'https://api.openai.com/v1',
    apiKey: '',
    selectedModel: null,
    sourceLanguage: '',
    targetLanguage: 'en',
    translationStyle: 'natural',
    contextLines: 2,
    outputDirectory: '',
    outputFormat: 'srt',
    ffmpegPath: '',
    autoBackup: true,
    keepOriginalTrack: true
  })
  modelOptions.value = []
  message.info('Settings reset to defaults')
}

defineExpose({ settings, getSystemPrompt })
</script>
