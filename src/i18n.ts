import { createI18n } from 'vue-i18n'

export const interfaceLocales = ['en', 'pt-BR'] as const

export type InterfaceLocale = typeof interfaceLocales[number]

export const defaultInterfaceLanguage: InterfaceLocale = 'en'

export const interfaceLanguageOptions = [
  { labelKey: 'languages.english', value: 'en' },
  { labelKey: 'languages.portugueseBrazil', value: 'pt-BR' }
] satisfies { labelKey: string; value: InterfaceLocale }[]

const messages = {
  en: {
    app: {
      eyebrow: 'animesubs://wired',
      title: 'subtitle protocol',
      runtimeStatus: 'Runtime status',
      provider: 'provider {provider}',
      model: 'model {model}',
      ffmpeg: 'ffmpeg {status}',
      unconfigured: 'unconfigured',
      noModel: 'no-model',
      targetUnknown: 'Target unknown',
      checking: 'checking',
      online: 'online',
      missing: 'missing',
      lightMode: 'Light mode',
      darkMode: 'Dark mode',
      settings: 'Settings',
      language: 'Language',
      terminalLine: 'connect_media_packet',
      heroTitle: 'Drop video files into the Wired.',
      heroDescription: 'Extract, translate, backup, and embed subtitle tracks without leaving the node.',
      selectFiles: 'SELECT FILES',
      scanFolder: 'SCAN FOLDER',
      ffmpegMissingTitle: 'FFmpeg signal missing',
      ffmpegMissingDescription: 'FFmpeg is required for subtitle extraction. Install FFmpeg or configure its path in Settings.',
      mediaQueue: 'media queue',
      packetsAttached: '{count} packets attached',
      clear: 'CLEAR',
      ready: 'ready',
      tracks: 'tracks',
      backups: 'backups',
      subs: '{count} subs',
      default: 'default',
      forced: 'forced',
      extractSubtitle: 'Extract subtitle',
      backupSubtitle: 'Backup subtitle',
      noSubtitleTracks: 'No subtitle tracks found',
      backupDivider: 'Backups',
      backupMeta: 'track {track} / {format} / {date}',
      restoreBackupConfirm: 'Restore this backup? This will replace the current subtitle track.',
      deleteBackupConfirm: 'Delete this backup?',
      translationProtocol: 'translation protocol',
      targetLanguage: 'Target Language',
      targetLanguagePlaceholder: 'e.g. pt, en, es, ja',
      subtitleTrack: 'Subtitle Track',
      autoDetectFirstAvailable: 'Auto-detect first available',
      embedTranslatedSubtitles: 'Embed translated subtitles',
      routeThroughMkvmerge: 'Route through mkvmerge',
      advancedSignalControls: 'Advanced signal controls',
      customPrompt: 'Custom Prompt',
      customPromptPlaceholder: 'Add temporary protocol instructions...',
      translatingSignal: 'TRANSLATING SIGNAL...',
      initiateTranslation: 'INITIATE TRANSLATION',
      disabledHint: 'Attach media with subtitle tracks and verify provider/FFmpeg settings.',
      sync: 'sync {progress}%',
      awaitingPacketResponse: 'awaiting packet response...'
    },
    setup: {
      title: 'Choose interface language',
      eyebrow: 'initial setup',
      description: 'Select the language AnimeSubs should use for menus, messages, and controls.',
      continue: 'Continue'
    },
    settings: {
      title: 'wired settings',
      interfaceTab: 'Interface',
      apiTab: 'API Configuration',
      translationTab: 'Translation',
      outputTab: 'Output',
      interfaceLanguage: 'Interface Language',
      provider: 'Provider',
      apiEndpoint: 'API Endpoint',
      apiKey: 'API Key',
      model: 'Model',
      selectModel: 'Select a model',
      providerPresets: 'Provider Presets',
      providerPresetsDescription: 'Click to quickly configure popular providers:',
      sourceLanguage: 'Source Language',
      autoDetect: 'Auto-detect',
      targetLanguage: 'Target Language',
      selectTargetLanguage: 'Select target language',
      translationStyle: 'Translation Style',
      systemPromptPreview: 'System Prompt Preview',
      outputDirectory: 'Output Directory',
      sameAsInput: 'Same as input',
      outputFormat: 'Output Format',
      ffmpegPath: 'FFmpeg Path',
      ffmpegPathPlaceholder: 'ffmpeg (uses PATH)',
      backupSettings: 'Backup Settings',
      autoBackup: 'Automatically backup subtitles before translation',
      keepOriginalTrack: 'Keep original subtitle track in video',
      providerLocal: '{provider} (Local)',
      minimaxTokenPlan: 'MiniMax (Token Plan)',
      customOpenAICompatible: 'Custom OpenAI-compatible',
      reset: 'Reset',
      saveSettings: 'Save Settings',
      configuredFor: 'Configured for {provider}',
      enterApiEndpointFirst: 'Please enter API endpoint first',
      enterApiKeyFirst: 'Please enter API key first',
      loadedModels: 'Loaded {count} models',
      failedToFetchModels: 'Failed to fetch models: {error}',
      selectOutputDirectory: 'Select Output Directory',
      selectFfmpegExecutable: 'Select FFmpeg Executable',
      settingsSaved: 'Settings saved',
      settingsReset: 'Settings reset to defaults',
      bearerToken: 'Bearer token from MiniMax Token Plan',
      optional: '(optional)',
      apiKeyPlaceholder: 'API key'
    },
    styles: {
      natural: 'Natural & Fluent',
      literal: 'Literal Translation',
      localized: 'Localized (Cultural Adaptation)',
      formal: 'Formal',
      casual: 'Casual',
      honorifics: 'Honorifics Preserved'
    },
    formats: {
      auto: 'Auto-detect (match source track)',
      srt: 'SRT (.srt)',
      ass: 'ASS/SSA (.ass)',
      vtt: 'WebVTT (.vtt)'
    },
    languages: {
      auto: 'Auto-detect',
      japanese: 'Japanese',
      english: 'English',
      chineseSimplified: 'Chinese (Simplified)',
      chineseTraditional: 'Chinese (Traditional)',
      korean: 'Korean',
      spanish: 'Spanish',
      french: 'French',
      german: 'German',
      persian: 'Persian',
      portuguese: 'Portuguese',
      portugueseBrazil: 'Portuguese (Brazil)',
      russian: 'Russian',
      italian: 'Italian',
      arabic: 'Arabic',
      thai: 'Thai',
      vietnamese: 'Vietnamese',
      indonesian: 'Indonesian',
      polish: 'Polish',
      turkish: 'Turkish'
    },
    track: {
      title: 'Track {index}'
    },
    dialogs: {
      videoFiles: 'Video Files'
    },
    status: {
      invalidApiKey: 'Invalid API key. Please check your credentials in Settings.',
      accessDenied: 'Access denied. Check API key permissions.',
      rateLimited: 'Rate limited. Please wait and try again.',
      apiError: 'API error ({status}): {details}',
      connectionTimeout: 'Connection timeout. Check endpoint URL and network.',
      cannotConnect: 'Cannot connect to API. Check endpoint URL.',
      connectionFailed: 'Connection failed: {message}',
      unknownConnectionError: 'Unknown connection error',
      validatingApi: 'Validating API connection...',
      apiValidationFailed: 'API validation failed',
      translatingLines: 'Translating {translated}/{total} lines...',
      translationComplete: 'Translation complete!',
      translationFailed: 'Translation failed: {failure}',
      translationFinishedWithErrors: 'Translation finished with errors ({completed}/{total}): {failure}',
      error: 'Error: {error}',
      processingFile: 'Processing {file} ({current}/{total})',
      extractingSubtitlesFrom: 'Extracting subtitles from {file}...',
      parsingSubtitlesFrom: 'Parsing subtitles from {file}...',
      translatingFileLines: 'Translating {file} ({lines} lines)...',
      savingTranslatedSubtitlesFor: 'Saving translated subtitles for {file}...',
      embeddingTranslatedSubtitlesIn: 'Embedding translated subtitles in {file}...',
      finishedFile: 'Finished {file}',
      errorInFile: 'Error in {file}: {reason}',
      translatingAllLines: 'Translating all {total} lines...',
      translatingChunk: 'Translating chunk {current}/{total} ({lines} lines)...',
      noVideoFilesSelected: 'No video files selected',
      trackNotFound: 'Track {track} not found',
      failedToExtractSubtitleTrack: 'Failed to extract subtitle track',
      subtitleExtractionNoOutput: 'Subtitle extraction returned no output path',
      noDialogLinesExtracted: 'No dialog lines found in extracted subtitle',
      noDialogLinesToTranslate: 'No dialog lines to translate',
      cannotReconstructAss: 'Cannot reconstruct ASS without original file or header',
      unsupportedFormat: 'Unsupported format: {format}',
      unsupportedSubtitleFormat: 'Unsupported subtitle format: {format}',
      outputPathRequired: 'output_path is required when temporary save is disabled',
      savedTranslatedSubtitles: 'Saved translated subtitles to {path}',
      ffmpegNotFound: 'FFmpeg not found. Please install FFmpeg or specify its path.',
      fileAlreadyRemoved: 'File already removed',
      fileDeleted: 'File deleted successfully',
      apiKeyLoaded: 'API key loaded',
      apiKeySaved: 'API key saved',
      subtitleEmbedded: 'Subtitle embedded successfully',
      subtitleEmbeddedMkvmerge: 'Subtitle embedded successfully (mkvmerge)',
      subtitleTrackRemoved: 'Subtitle track removed successfully',
      subtitleRestored: 'Subtitle restored successfully',
      backupDeleted: 'Backup deleted successfully',
      backupFileNotFound: 'Backup file not found',
      subtitleTrackNotFound: 'Subtitle track not found',
      unknownError: 'Unknown error'
    },
    prompts: {
      sourceLanguage: 'Source language: {language}',
      detectSourceLanguage: 'Detect the source language automatically.',
      fallbackSystemPrompt: 'You are a professional subtitle translator. Translate the following subtitle lines to {targetLanguage}. Keep translations natural and contextually appropriate for anime dialogue.',
      natural: 'You are an expert anime subtitle translator. Translate the following subtitle lines to {targetLang}.\n\nGuidelines:\n- Provide natural, fluent translations that sound like native speech\n- Preserve the emotional tone and intent of the original dialogue\n- Adapt idioms and expressions to their closest natural equivalent\n- Keep character names in their original form unless there is a well-known localized version\n- Maintain the pacing suitable for subtitle reading\n- Do NOT add explanations or notes, only provide the translation\n\n{context}',
      literal: 'You are a precise subtitle translator. Translate the following subtitle lines to {targetLang}.\n\nGuidelines:\n- Translate as literally as possible while maintaining grammatical correctness\n- Preserve the original sentence structure when feasible\n- Keep all names and terms in their original form\n- Do not add or remove information from the original\n- Do NOT add explanations or notes, only provide the translation\n\n{context}',
      localized: 'You are a localization expert for anime subtitles. Translate and adapt the following lines to {targetLang}.\n\nGuidelines:\n- Adapt cultural references to equivalents the target audience will understand\n- Convert measurements, currencies, and cultural concepts appropriately\n- Rewrite jokes and wordplay to work in the target language\n- Make dialogue feel natural for the target culture\n- Preserve the overall story meaning and character relationships\n- Do NOT add explanations or notes, only provide the translation\n\n{context}',
      formal: 'You are a professional subtitle translator. Translate the following lines to {targetLang} using formal language.\n\nGuidelines:\n- Use formal register and polite language\n- Avoid slang, contractions, and casual expressions\n- Maintain professional and respectful tone\n- Suitable for educational or professional contexts\n- Do NOT add explanations or notes, only provide the translation\n\n{context}',
      casual: 'You are a subtitle translator specializing in casual dialogue. Translate to {targetLang}.\n\nGuidelines:\n- Use casual, conversational language\n- Include appropriate slang and colloquialisms\n- Use contractions and informal expressions\n- Match the relaxed tone of casual conversation\n- Do NOT add explanations or notes, only provide the translation\n\n{context}',
      honorifics: 'You are an anime subtitle translator who preserves Japanese honorifics. Translate to {targetLang}.\n\nGuidelines:\n- Keep Japanese honorifics (-san, -kun, -chan, -sama, -sensei, -senpai, etc.)\n- Preserve name order (family name first if appropriate)\n- Keep certain untranslatable terms (onii-chan, kawaii, etc.) with context clues\n- Maintain the social relationship nuances through honorific usage\n- Translate the rest naturally and fluently\n- Do NOT add explanations or notes, only provide the translation\n\n{context}'
    }
  },
  'pt-BR': {
    app: {
      eyebrow: 'animesubs://wired',
      title: 'protocolo de legendas',
      runtimeStatus: 'Status de execução',
      provider: 'provedor {provider}',
      model: 'modelo {model}',
      ffmpeg: 'ffmpeg {status}',
      unconfigured: 'não configurado',
      noModel: 'sem modelo',
      targetUnknown: 'Destino desconhecido',
      checking: 'verificando',
      online: 'online',
      missing: 'ausente',
      lightMode: 'Modo claro',
      darkMode: 'Modo escuro',
      settings: 'Configurações',
      language: 'Idioma',
      terminalLine: 'connect_media_packet',
      heroTitle: 'Solte arquivos de vídeo na Wired.',
      heroDescription: 'Extraia, traduza, faça backup e incorpore faixas de legenda sem sair do nó.',
      selectFiles: 'SELECIONAR ARQUIVOS',
      scanFolder: 'VARRER PASTA',
      ffmpegMissingTitle: 'Sinal do FFmpeg ausente',
      ffmpegMissingDescription: 'O FFmpeg é necessário para extrair legendas. Instale o FFmpeg ou configure o caminho em Configurações.',
      mediaQueue: 'fila de mídia',
      packetsAttached: '{count} pacotes anexados',
      clear: 'LIMPAR',
      ready: 'prontos',
      tracks: 'faixas',
      backups: 'backups',
      subs: '{count} legendas',
      default: 'padrão',
      forced: 'forçada',
      extractSubtitle: 'Extrair legenda',
      backupSubtitle: 'Fazer backup da legenda',
      noSubtitleTracks: 'Nenhuma faixa de legenda encontrada',
      backupDivider: 'Backups',
      backupMeta: 'faixa {track} / {format} / {date}',
      restoreBackupConfirm: 'Restaurar este backup? Isso substituirá a faixa de legenda atual.',
      deleteBackupConfirm: 'Excluir este backup?',
      translationProtocol: 'protocolo de tradução',
      targetLanguage: 'Idioma de destino',
      targetLanguagePlaceholder: 'ex.: pt, en, es, ja',
      subtitleTrack: 'Faixa de legenda',
      autoDetectFirstAvailable: 'Detectar automaticamente a primeira disponível',
      embedTranslatedSubtitles: 'Incorporar legendas traduzidas',
      routeThroughMkvmerge: 'Roteirizar pelo mkvmerge',
      advancedSignalControls: 'Controles avançados de sinal',
      customPrompt: 'Prompt personalizado',
      customPromptPlaceholder: 'Adicione instruções temporárias ao protocolo...',
      translatingSignal: 'TRADUZINDO SINAL...',
      initiateTranslation: 'INICIAR TRADUÇÃO',
      disabledHint: 'Anexe mídia com faixas de legenda e verifique as configurações de provedor/FFmpeg.',
      sync: 'sync {progress}%',
      awaitingPacketResponse: 'aguardando resposta do pacote...'
    },
    setup: {
      title: 'Escolha o idioma da interface',
      eyebrow: 'setup inicial',
      description: 'Selecione o idioma que o AnimeSubs deve usar em menus, mensagens e controles.',
      continue: 'Continuar'
    },
    settings: {
      title: 'configurações wired',
      interfaceTab: 'Interface',
      apiTab: 'Configuração da API',
      translationTab: 'Tradução',
      outputTab: 'Saída',
      interfaceLanguage: 'Idioma da interface',
      provider: 'Provedor',
      apiEndpoint: 'Endpoint da API',
      apiKey: 'Chave da API',
      model: 'Modelo',
      selectModel: 'Selecione um modelo',
      providerPresets: 'Predefinições de provedor',
      providerPresetsDescription: 'Clique para configurar rapidamente provedores populares:',
      sourceLanguage: 'Idioma de origem',
      autoDetect: 'Detectar automaticamente',
      targetLanguage: 'Idioma de destino',
      selectTargetLanguage: 'Selecione o idioma de destino',
      translationStyle: 'Estilo de tradução',
      systemPromptPreview: 'Prévia do prompt do sistema',
      outputDirectory: 'Diretório de saída',
      sameAsInput: 'Mesmo da entrada',
      outputFormat: 'Formato de saída',
      ffmpegPath: 'Caminho do FFmpeg',
      ffmpegPathPlaceholder: 'ffmpeg (usa o PATH)',
      backupSettings: 'Configurações de backup',
      autoBackup: 'Fazer backup automático das legendas antes da tradução',
      keepOriginalTrack: 'Manter a faixa de legenda original no vídeo',
      providerLocal: '{provider} (Local)',
      minimaxTokenPlan: 'MiniMax (plano de tokens)',
      customOpenAICompatible: 'Personalizado compatível com OpenAI',
      reset: 'Redefinir',
      saveSettings: 'Salvar Configurações',
      configuredFor: 'Configurado para {provider}',
      enterApiEndpointFirst: 'Informe o endpoint da API primeiro',
      enterApiKeyFirst: 'Informe a chave da API primeiro',
      loadedModels: '{count} modelos carregados',
      failedToFetchModels: 'Falha ao buscar modelos: {error}',
      selectOutputDirectory: 'Selecionar diretório de saída',
      selectFfmpegExecutable: 'Selecionar executável do FFmpeg',
      settingsSaved: 'Configurações salvas',
      settingsReset: 'Configurações redefinidas para o padrão',
      bearerToken: 'Token Bearer do plano de tokens MiniMax',
      optional: '(opcional)',
      apiKeyPlaceholder: 'Chave da API'
    },
    styles: {
      natural: 'Natural e fluente',
      literal: 'Tradução literal',
      localized: 'Localizada (adaptação cultural)',
      formal: 'Formal',
      casual: 'Casual',
      honorifics: 'Honoríficos preservados'
    },
    formats: {
      auto: 'Detectar automaticamente (igualar faixa de origem)',
      srt: 'SRT (.srt)',
      ass: 'ASS/SSA (.ass)',
      vtt: 'WebVTT (.vtt)'
    },
    languages: {
      auto: 'Detectar automaticamente',
      japanese: 'Japonês',
      english: 'Inglês',
      chineseSimplified: 'Chinês (simplificado)',
      chineseTraditional: 'Chinês (tradicional)',
      korean: 'Coreano',
      spanish: 'Espanhol',
      french: 'Francês',
      german: 'Alemão',
      persian: 'Persa',
      portuguese: 'Português',
      portugueseBrazil: 'Português (Brasil)',
      russian: 'Russo',
      italian: 'Italiano',
      arabic: 'Árabe',
      thai: 'Tailandês',
      vietnamese: 'Vietnamita',
      indonesian: 'Indonésio',
      polish: 'Polonês',
      turkish: 'Turco'
    },
    track: {
      title: 'Faixa {index}'
    },
    dialogs: {
      videoFiles: 'Arquivos de vídeo'
    },
    status: {
      invalidApiKey: 'Chave da API inválida. Verifique suas credenciais em Configurações.',
      accessDenied: 'Acesso negado. Verifique as permissões da chave da API.',
      rateLimited: 'Limite de requisições atingido. Aguarde e tente novamente.',
      apiError: 'Erro da API ({status}): {details}',
      connectionTimeout: 'Tempo de conexão esgotado. Verifique o endpoint e a rede.',
      cannotConnect: 'Não foi possível conectar à API. Verifique o endpoint.',
      connectionFailed: 'Conexão falhou: {message}',
      unknownConnectionError: 'Erro de conexão desconhecido',
      validatingApi: 'Validando conexão com a API...',
      apiValidationFailed: 'Validação da API falhou',
      translatingLines: 'Traduzindo {translated}/{total} linhas...',
      translationComplete: 'Tradução concluída!',
      translationFailed: 'Tradução falhou: {failure}',
      translationFinishedWithErrors: 'Tradução concluída com erros ({completed}/{total}): {failure}',
      error: 'Erro: {error}',
      processingFile: 'Processando {file} ({current}/{total})',
      extractingSubtitlesFrom: 'Extraindo legendas de {file}...',
      parsingSubtitlesFrom: 'Analisando legendas de {file}...',
      translatingFileLines: 'Traduzindo {file} ({lines} linhas)...',
      savingTranslatedSubtitlesFor: 'Salvando legendas traduzidas para {file}...',
      embeddingTranslatedSubtitlesIn: 'Incorporando legendas traduzidas em {file}...',
      finishedFile: '{file} concluído',
      errorInFile: 'Erro em {file}: {reason}',
      translatingAllLines: 'Traduzindo todas as {total} linhas...',
      translatingChunk: 'Traduzindo bloco {current}/{total} ({lines} linhas)...',
      noVideoFilesSelected: 'Nenhum arquivo de vídeo selecionado',
      trackNotFound: 'Faixa {track} não encontrada',
      failedToExtractSubtitleTrack: 'Falha ao extrair faixa de legenda',
      subtitleExtractionNoOutput: 'A extração de legenda não retornou caminho de saída',
      noDialogLinesExtracted: 'Nenhuma linha de diálogo encontrada na legenda extraída',
      noDialogLinesToTranslate: 'Nenhuma linha de diálogo para traduzir',
      cannotReconstructAss: 'Não é possível reconstruir ASS sem o arquivo original ou o cabeçalho',
      unsupportedFormat: 'Formato não suportado: {format}',
      unsupportedSubtitleFormat: 'Formato de legenda não suportado: {format}',
      outputPathRequired: 'output_path é obrigatório quando o salvamento temporário está desativado',
      savedTranslatedSubtitles: 'Legendas traduzidas salvas em {path}',
      ffmpegNotFound: 'FFmpeg não encontrado. Instale o FFmpeg ou especifique o caminho.',
      fileAlreadyRemoved: 'Arquivo já removido',
      fileDeleted: 'Arquivo excluído com sucesso',
      apiKeyLoaded: 'Chave da API carregada',
      apiKeySaved: 'Chave da API salva',
      subtitleEmbedded: 'Legenda incorporada com sucesso',
      subtitleEmbeddedMkvmerge: 'Legenda incorporada com sucesso (mkvmerge)',
      subtitleTrackRemoved: 'Faixa de legenda removida com sucesso',
      subtitleRestored: 'Legenda restaurada com sucesso',
      backupDeleted: 'Backup excluído com sucesso',
      backupFileNotFound: 'Arquivo de backup não encontrado',
      subtitleTrackNotFound: 'Faixa de legenda não encontrada',
      unknownError: 'Erro desconhecido'
    },
    prompts: {
      sourceLanguage: 'Idioma de origem: {language}',
      detectSourceLanguage: 'Detecte o idioma de origem automaticamente.',
      fallbackSystemPrompt: 'Você é um tradutor profissional de legendas. Traduza as seguintes linhas de legenda para {targetLanguage}. Mantenha traduções naturais e contextualmente adequadas para diálogos de anime.',
      natural: 'Você é um tradutor especialista em legendas de anime. Traduza as seguintes linhas de legenda para {targetLang}.\n\nDiretrizes:\n- Forneça traduções naturais e fluentes que soem como fala nativa\n- Preserve o tom emocional e a intenção do diálogo original\n- Adapte expressões idiomáticas para o equivalente natural mais próximo\n- Mantenha nomes de personagens na forma original, a menos que exista uma versão localizada conhecida\n- Preserve um ritmo adequado para leitura em legenda\n- NÃO adicione explicações ou notas, forneça apenas a tradução\n\n{context}',
      literal: 'Você é um tradutor preciso de legendas. Traduza as seguintes linhas de legenda para {targetLang}.\n\nDiretrizes:\n- Traduza da forma mais literal possível, mantendo correção gramatical\n- Preserve a estrutura da frase original quando viável\n- Mantenha todos os nomes e termos na forma original\n- Não adicione nem remova informações do original\n- NÃO adicione explicações ou notas, forneça apenas a tradução\n\n{context}',
      localized: 'Você é um especialista em localização para legendas de anime. Traduza e adapte as seguintes linhas para {targetLang}.\n\nDiretrizes:\n- Adapte referências culturais para equivalentes compreensíveis pelo público-alvo\n- Converta medidas, moedas e conceitos culturais apropriadamente\n- Reescreva piadas e jogos de palavras para funcionarem no idioma de destino\n- Faça o diálogo soar natural para a cultura de destino\n- Preserve o significado geral da história e as relações entre personagens\n- NÃO adicione explicações ou notas, forneça apenas a tradução\n\n{context}',
      formal: 'Você é um tradutor profissional de legendas. Traduza as seguintes linhas para {targetLang} usando linguagem formal.\n\nDiretrizes:\n- Use registro formal e linguagem polida\n- Evite gírias, contrações e expressões casuais\n- Mantenha um tom profissional e respeitoso\n- Adequado para contextos educacionais ou profissionais\n- NÃO adicione explicações ou notas, forneça apenas a tradução\n\n{context}',
      casual: 'Você é um tradutor de legendas especializado em diálogos casuais. Traduza para {targetLang}.\n\nDiretrizes:\n- Use linguagem casual e conversacional\n- Inclua gírias e coloquialismos apropriados\n- Use expressões informais\n- Acompanhe o tom descontraído de uma conversa casual\n- NÃO adicione explicações ou notas, forneça apenas a tradução\n\n{context}',
      honorifics: 'Você é um tradutor de legendas de anime que preserva honoríficos japoneses. Traduza para {targetLang}.\n\nDiretrizes:\n- Mantenha honoríficos japoneses (-san, -kun, -chan, -sama, -sensei, -senpai etc.)\n- Preserve a ordem dos nomes (sobrenome primeiro, quando apropriado)\n- Mantenha certos termos intraduzíveis (onii-chan, kawaii etc.) com pistas de contexto\n- Preserve as nuances de relação social por meio dos honoríficos\n- Traduza o restante de forma natural e fluente\n- NÃO adicione explicações ou notas, forneça apenas a tradução\n\n{context}'
    }
  }
}

export const i18n = createI18n({
  legacy: false,
  locale: defaultInterfaceLanguage,
  fallbackLocale: defaultInterfaceLanguage,
  messages
})

export const isInterfaceLocale = (value: unknown): value is InterfaceLocale => {
  return typeof value === 'string' && interfaceLocales.includes(value as InterfaceLocale)
}

export const setInterfaceLocale = (locale: string) => {
  if (isInterfaceLocale(locale)) {
    i18n.global.locale.value = locale
  }
}

export const translationLanguageKey = (value: string) => {
  const keys: Record<string, string> = {
    '': 'languages.auto',
    ja: 'languages.japanese',
    en: 'languages.english',
    'zh-CN': 'languages.chineseSimplified',
    'zh-TW': 'languages.chineseTraditional',
    ko: 'languages.korean',
    es: 'languages.spanish',
    fr: 'languages.french',
    de: 'languages.german',
    fa: 'languages.persian',
    pt: 'languages.portuguese',
    ru: 'languages.russian',
    it: 'languages.italian',
    ar: 'languages.arabic',
    th: 'languages.thai',
    vi: 'languages.vietnamese',
    id: 'languages.indonesian',
    pl: 'languages.polish',
    tr: 'languages.turkish'
  }

  return keys[value] || value
}

type TranslateFn = (key: string, named?: Record<string, unknown>) => string

export const localizeBackendMessage = (message: string, t: TranslateFn): string => {
  const exact: Record<string, string> = {
    'Translation complete!': 'status.translationComplete',
    'Failed to extract subtitle track': 'status.failedToExtractSubtitleTrack',
    'Subtitle extraction returned no output path': 'status.subtitleExtractionNoOutput',
    'No dialog lines found in extracted subtitle': 'status.noDialogLinesExtracted',
    'No dialog lines to translate': 'status.noDialogLinesToTranslate',
    'Cannot reconstruct ASS without original file or header': 'status.cannotReconstructAss',
    'output_path is required when temporary save is disabled': 'status.outputPathRequired',
    'No video files selected': 'status.noVideoFilesSelected',
    'FFmpeg not found. Please install FFmpeg or specify its path.': 'status.ffmpegNotFound',
    'File already removed': 'status.fileAlreadyRemoved',
    'File deleted successfully': 'status.fileDeleted',
    'API key loaded': 'status.apiKeyLoaded',
    'API key saved': 'status.apiKeySaved',
    'Subtitle embedded successfully': 'status.subtitleEmbedded',
    'Subtitle embedded successfully (mkvmerge)': 'status.subtitleEmbeddedMkvmerge',
    'Subtitle track removed successfully': 'status.subtitleTrackRemoved',
    'Subtitle restored successfully': 'status.subtitleRestored',
    'Backup deleted successfully': 'status.backupDeleted',
    'Backup file not found': 'status.backupFileNotFound',
    'Subtitle track not found': 'status.subtitleTrackNotFound',
    'Unknown error': 'status.unknownError'
  }

  const exactKey = exact[message]
  if (exactKey) return t(exactKey)

  const patterns: Array<[RegExp, string, (match: RegExpMatchArray) => Record<string, unknown>]> = [
    [/^Processing (.+) \((\d+)\/(\d+)\)$/, 'status.processingFile', m => ({
      file: m[1],
      current: m[2],
      total: m[3]
    })],
    [/^Extracting subtitles from (.+)\.\.\.$/, 'status.extractingSubtitlesFrom', m => ({
      file: m[1]
    })],
    [/^Parsing subtitles from (.+)\.\.\.$/, 'status.parsingSubtitlesFrom', m => ({
      file: m[1]
    })],
    [/^Translating (.+) \((\d+) lines\)\.\.\.$/, 'status.translatingFileLines', m => ({
      file: m[1],
      lines: m[2]
    })],
    [/^Saving translated subtitles for (.+)\.\.\.$/, 'status.savingTranslatedSubtitlesFor', m => ({
      file: m[1]
    })],
    [/^Embedding translated subtitles in (.+)\.\.\.$/, 'status.embeddingTranslatedSubtitlesIn', m => ({
      file: m[1]
    })],
    [/^Finished (.+)$/, 'status.finishedFile', m => ({
      file: m[1]
    })],
    [/^Error in (.+): (.+)$/, 'status.errorInFile', m => ({
      file: m[1],
      reason: localizeBackendMessage(m[2], t)
    })],
    [/^Translating all (\d+) lines\.\.\.$/, 'status.translatingAllLines', m => ({
      total: m[1]
    })],
    [/^Translating chunk (\d+)\/(\d+) \((\d+) lines\)\.\.\.$/, 'status.translatingChunk', m => ({
      current: m[1],
      total: m[2],
      lines: m[3]
    })],
    [/^Track (\d+) not found$/, 'status.trackNotFound', m => ({
      track: m[1]
    })],
    [/^Unsupported format: (.+)$/, 'status.unsupportedFormat', m => ({
      format: m[1]
    })],
    [/^Unsupported subtitle format: (.+)$/, 'status.unsupportedSubtitleFormat', m => ({
      format: m[1]
    })],
    [/^Saved translated subtitles to (.+)$/, 'status.savedTranslatedSubtitles', m => ({
      path: m[1]
    })],
    [/^Translation failed: (.+)$/, 'status.translationFailed', m => ({
      failure: localizeBackendMessage(m[1], t)
    })],
    [/^Translation finished with errors \((\d+)\/(\d+)\): (.+)$/, 'status.translationFinishedWithErrors', m => ({
      completed: m[1],
      total: m[2],
      failure: localizeBackendMessage(m[3], t)
    })]
  ]

  for (const [pattern, key, toNamed] of patterns) {
    const match = message.match(pattern)
    if (match) return t(key, toNamed(match))
  }

  return message
}
