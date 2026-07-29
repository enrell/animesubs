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
      eyebrow: 'AI subtitle studio',
      title: 'AnimeSubs',
      provider: 'Provider: {provider}',
      model: 'Model: {model}',
      ffmpeg: 'FFmpeg: {status}',
      engine: 'Engine',
      unconfigured: 'Not configured',
      noModel: 'No model',
      checking: 'checking',
      online: 'available',
      missing: 'missing',
      theme: 'Theme',
      settings: 'Settings',
      language: 'Language',
      translation: 'Translation',
      targetLanguage: 'Translate into',
      subtitleTrack: 'Subtitle track',
      autoDetectFirstAvailable: 'Auto-detect (first available)',
      embedTranslatedSubtitles: 'Embed subtitles into the video',
      useMkvmerge: 'Use mkvmerge to embed',
      seriesContext: 'Series context',
      seriesContextHint: 'Consistent names and terms across episodes',
      advancedOptions: 'Advanced options',
      customPrompt: 'Custom prompt',
      customPromptPlaceholder: 'Extra instructions for this translation run...',
      ffmpegMissingTitle: 'FFmpeg not found',
      ffmpegMissingDescription: 'FFmpeg is required to extract and embed subtitles. Install it or set its path in Settings.',
      dropHint: 'Drop video files anywhere to add them'
    },
    empty: {
      title: 'Translate your anime subtitles',
      description: 'Drop a season of episodes and get translated subtitles, embedded into the video. Your original files are backed up automatically.',
      selectFiles: 'Choose episodes',
      scanFolder: 'Add a folder',
      checklistTitle: 'One-time setup',
      providerStep: 'Translation provider',
      providerDone: 'Connected to {provider}',
      providerTodo: 'Choose a provider and model',
      ffmpegStep: 'FFmpeg',
      ffmpegDone: 'Ready',
      ffmpegTodo: 'Not found on this system',
      fixInSettings: 'Set up'
    },
    queue: {
      title: 'Episodes',
      addFiles: 'Add files',
      addFolder: 'Add folder',
      clearAll: 'Clear all',
      originalsSafe: 'Originals are backed up automatically',
      reading: 'Reading file…',
      probeFailed: "Couldn't read this file",
      noSubs: 'No embedded subtitles',
      trackSummary: '{language} subtitle',
      trackMeta: 'track {index} · {codec}',
      inQueue: 'In queue — position {n}',
      phaseExtract: 'Extracting subtitle…',
      phaseTranslate: 'Translating — {done}/{total} lines',
      phaseWrite: 'Writing subtitles…',
      phaseOther: 'Working…',
      doneEmbedded: 'Translated and embedded into the video',
      doneSaved: 'Translated subtitle saved',
      retry: 'Try again',
      retranslate: 'Translate again',
      remove: 'Remove from queue',
      expand: 'Details',
      collapse: 'Hide details',
      subs: '{count} subs',
      forced: 'forced',
      default: 'default',
      extractSubtitle: 'Extract subtitle',
      backupSubtitle: 'Back up subtitle',
      backupsTitle: 'Backups',
      backupMeta: 'Track {track} · {format} · {date}',
      restoreBackupConfirm: 'Restore this backup? It will replace the current subtitle track.',
      deleteBackupConfirm: 'Delete this backup?'
    },
    footer: {
      ready: '{n} ready',
      translating: '{n} translating',
      failed: '{n} failed',
      done: '{n} done',
      translate: 'Translate {n} episodes',
      retry: 'Retry {n} failed',
      busy: 'Translating {file} — {current}/{total}',
      disabledAdd: 'Add episodes to start',
      disabledProvider: 'Set up a provider to start',
      disabledFfmpeg: 'Set up FFmpeg to start',
      openSettings: 'Open settings'
    },
    log: {
      title: 'Activity',
      clear: 'Clear',
      empty: 'Nothing yet — activity will show up here.',
      jobStarted: 'Translation started for {count} file(s)',
      fileDone: '{file} — done',
      fileFailed: '{file} — {reason}'
    },
    themes: {
      kitsune: 'Kitsune',
      matsuri: 'Matsuri',
      hanami: 'Hanami',
      matcha: 'Matcha'
    },
    setup: {
      title: 'Choose interface language',
      eyebrow: 'initial setup',
      description: 'Select the language AnimeSubs should use for menus, messages, and controls.',
      continue: 'Continue'
    },
    settings: {
      title: 'Settings',
      interfaceTab: 'Interface',
      apiTab: 'API Configuration',
      translationTab: 'Translation',
      outputTab: 'Output',
      interfaceLanguage: 'Interface Language',
      provider: 'Provider',
      apiEndpoint: 'API Endpoint',
      apiKey: 'API Key',
      tmdbApiKey: 'TMDB API Key',
      model: 'Model',
      selectModel: 'Select a model',
      providerPresets: 'Provider Presets',
      providerPresetsDescription: 'Click to quickly configure popular providers:',
      sourceLanguage: 'Source Language',
      autoDetect: 'Auto-detect',
      targetLanguage: 'Target Language',
      selectTargetLanguage: 'Select target language',
      systemPromptPreview: 'System Prompt',
      customSystemPromptHint: 'Leave empty to use the default prompt. Enter your own text to override it completely.',
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
    context: {
      eyebrow: 'story context',
      title: 'Context Extraction',
      enable: 'Use during translation',
      noProfile: 'No context profile extracted yet. Analyze a subtitle file to extract characters, terminology, and story context.',
      extractFromMedia: 'Extract from media',
      loadFromFile: 'Load from file',
      saveToFile: 'Save to file',
      edit: 'Edit',
      hideEditor: 'Hide editor',
      reextract: 'Re-extract',
      clearConfirm: 'Clear context profile?',
      extracting: 'Extracting context...',
      addCharacter: 'Add character',
      addTerm: 'Add term',
      fields: {
        title: 'Title',
        characters: 'Characters',
        locations: 'Locations',
        terminology: 'Terminology',
        tone: 'Tone notes',
        plot: 'Plot summary',
        customInstructions: 'Custom instructions',
        verifiedMatch: 'Verified match (AniList)'
      },
      placeholders: {
        title: 'Series or episode title',
        name: 'Character name',
        role: 'Role (e.g. protagonist, mentor)',
        description: 'Description / personality notes',
        tone: 'Overall tone and speech style notes',
        plot: 'Brief plot summary',
        sourceTerm: 'Source term',
        translatedTerm: 'Translated term',
        customInstructions: 'Special instructions for extraction and translation'
      },
      extractPromptTitle: 'Extract Context',
      extractPromptDescription: 'Analyze the first selected file to extract characters, terminology, and story context for consistent translations.',
      extractPromptPlaceholder: 'Additional instructions for context extraction (optional)...',
      extractPromptConfirm: 'Extract',
      savedToFile: 'Context profile saved to {path}',
      loadedFromFile: 'Context profile loaded',
      profileCleared: 'Context profile cleared',
      extractionFailed: 'Context extraction failed: {error}'
    }
  },
  'pt-BR': {
    app: {
      eyebrow: 'Estúdio de legendas com IA',
      title: 'AnimeSubs',
      provider: 'Provedor: {provider}',
      model: 'Modelo: {model}',
      ffmpeg: 'FFmpeg: {status}',
      engine: 'Motor',
      unconfigured: 'Não configurado',
      noModel: 'Sem modelo',
      checking: 'verificando',
      online: 'disponível',
      missing: 'ausente',
      theme: 'Tema',
      settings: 'Configurações',
      language: 'Idioma',
      translation: 'Tradução',
      targetLanguage: 'Traduzir para',
      subtitleTrack: 'Faixa de legenda',
      autoDetectFirstAvailable: 'Detectar automaticamente (primeira disponível)',
      embedTranslatedSubtitles: 'Embutir legendas no vídeo',
      useMkvmerge: 'Usar mkvmerge para embutir',
      seriesContext: 'Contexto da série',
      seriesContextHint: 'Nomes e termos consistentes entre episódios',
      advancedOptions: 'Opções avançadas',
      customPrompt: 'Prompt personalizado',
      customPromptPlaceholder: 'Instruções extras para esta tradução...',
      ffmpegMissingTitle: 'FFmpeg não encontrado',
      ffmpegMissingDescription: 'O FFmpeg é necessário para extrair e embutir legendas. Instale-o ou configure o caminho em Configurações.',
      dropHint: 'Solte arquivos de vídeo em qualquer lugar para adicioná-los'
    },
    empty: {
      title: 'Traduza as legendas dos seus animes',
      description: 'Solte uma temporada de episódios e receba legendas traduzidas, embutidas no vídeo. Seus arquivos originais são protegidos com backup automático.',
      selectFiles: 'Escolher episódios',
      scanFolder: 'Adicionar uma pasta',
      checklistTitle: 'Configuração única',
      providerStep: 'Provedor de tradução',
      providerDone: 'Conectado a {provider}',
      providerTodo: 'Escolha um provedor e um modelo',
      ffmpegStep: 'FFmpeg',
      ffmpegDone: 'Pronto',
      ffmpegTodo: 'Não encontrado neste sistema',
      fixInSettings: 'Configurar'
    },
    queue: {
      title: 'Episódios',
      addFiles: 'Adicionar arquivos',
      addFolder: 'Adicionar pasta',
      clearAll: 'Limpar tudo',
      originalsSafe: 'Os originais são protegidos com backup automático',
      reading: 'Lendo arquivo…',
      probeFailed: 'Não foi possível ler este arquivo',
      noSubs: 'Nenhuma legenda embutida',
      trackSummary: 'Legenda em {language}',
      trackMeta: 'faixa {index} · {codec}',
      inQueue: 'Na fila — posição {n}',
      phaseExtract: 'Extraindo legenda…',
      phaseTranslate: 'Traduzindo — {done}/{total} linhas',
      phaseWrite: 'Gravando legendas…',
      phaseOther: 'Trabalhando…',
      doneEmbedded: 'Traduzido e embutido no vídeo',
      doneSaved: 'Legenda traduzida salva',
      retry: 'Tentar de novo',
      retranslate: 'Traduzir novamente',
      remove: 'Remover da fila',
      expand: 'Detalhes',
      collapse: 'Ocultar detalhes',
      subs: '{count} legendas',
      forced: 'forçada',
      default: 'padrão',
      extractSubtitle: 'Extrair legenda',
      backupSubtitle: 'Fazer backup da legenda',
      backupsTitle: 'Backups',
      backupMeta: 'Faixa {track} · {format} · {date}',
      restoreBackupConfirm: 'Restaurar este backup? Isso substituirá a faixa de legenda atual.',
      deleteBackupConfirm: 'Excluir este backup?'
    },
    footer: {
      ready: '{n} prontos',
      translating: '{n} traduzindo',
      failed: '{n} falharam',
      done: '{n} concluídos',
      translate: 'Traduzir {n} episódios',
      retry: 'Retentar {n} falhas',
      busy: 'Traduzindo {file} — {current}/{total}',
      disabledAdd: 'Adicione episódios para começar',
      disabledProvider: 'Configure um provedor para começar',
      disabledFfmpeg: 'Configure o FFmpeg para começar',
      openSettings: 'Abrir configurações'
    },
    log: {
      title: 'Atividade',
      clear: 'Limpar',
      empty: 'Nada por aqui — a atividade aparecerá neste painel.',
      jobStarted: 'Tradução iniciada para {count} arquivo(s)',
      fileDone: '{file} — concluído',
      fileFailed: '{file} — {reason}'
    },
    themes: {
      kitsune: 'Kitsune',
      matsuri: 'Matsuri',
      hanami: 'Hanami',
      matcha: 'Matcha'
    },
    setup: {
      title: 'Escolha o idioma da interface',
      eyebrow: 'setup inicial',
      description: 'Selecione o idioma que o AnimeSubs deve usar em menus, mensagens e controles.',
      continue: 'Continuar'
    },
    settings: {
      title: 'Configurações',
      interfaceTab: 'Interface',
      apiTab: 'Configuração da API',
      translationTab: 'Tradução',
      outputTab: 'Saída',
      interfaceLanguage: 'Idioma da interface',
      provider: 'Provedor',
      apiEndpoint: 'Endpoint da API',
      apiKey: 'Chave da API',
      tmdbApiKey: 'Chave da API do TMDB',
      model: 'Modelo',
      selectModel: 'Selecione um modelo',
      providerPresets: 'Predefinições de provedor',
      providerPresetsDescription: 'Clique para configurar rapidamente provedores populares:',
      sourceLanguage: 'Idioma de origem',
      autoDetect: 'Detectar automaticamente',
      targetLanguage: 'Idioma de destino',
      selectTargetLanguage: 'Selecione o idioma de destino',
      systemPromptPreview: 'Prompt do Sistema',
      customSystemPromptHint: 'Deixe vazio para usar o prompt padrão. Digite seu próprio texto para substituí-lo completamente.',
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
    context: {
      eyebrow: 'contexto da história',
      title: 'Extração de Contexto',
      enable: 'Usar na tradução',
      noProfile: 'Nenhum perfil de contexto extraído ainda. Analise um arquivo de legenda para extrair personagens, terminologia e contexto da história.',
      extractFromMedia: 'Extrair da mídia',
      loadFromFile: 'Carregar de arquivo',
      saveToFile: 'Salvar em arquivo',
      edit: 'Editar',
      hideEditor: 'Ocultar editor',
      reextract: 'Reextrair',
      clearConfirm: 'Limpar perfil de contexto?',
      extracting: 'Extraindo contexto...',
      addCharacter: 'Adicionar personagem',
      addTerm: 'Adicionar termo',
      fields: {
        title: 'Título',
        characters: 'Personagens',
        locations: 'Locais',
        terminology: 'Terminologia',
        tone: 'Notas de tom',
        plot: 'Resumo da trama',
        customInstructions: 'Instruções personalizadas',
        verifiedMatch: 'Correspondência verificada (AniList)'
      },
      placeholders: {
        title: 'Título da série ou episódio',
        name: 'Nome do personagem',
        role: 'Papel (ex.: protagonista, mentor)',
        description: 'Descrição / notas de personalidade',
        tone: 'Notas gerais de tom e estilo de fala',
        plot: 'Resumo breve da trama',
        sourceTerm: 'Termo de origem',
        translatedTerm: 'Termo traduzido',
        customInstructions: 'Instruções especiais para extração e tradução'
      },
      extractPromptTitle: 'Extrair Contexto',
      extractPromptDescription: 'Analise o primeiro arquivo selecionado para extrair personagens, terminologia e contexto da história para traduções consistentes.',
      extractPromptPlaceholder: 'Instruções adicionais para a extração de contexto (opcional)...',
      extractPromptConfirm: 'Extrair',
      savedToFile: 'Perfil de contexto salvo em {path}',
      loadedFromFile: 'Perfil de contexto carregado',
      profileCleared: 'Perfil de contexto limpo',
      extractionFailed: 'Falha na extração de contexto: {error}'
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
