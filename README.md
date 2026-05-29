# psyche-subtitle-toolkit

Extract, translate, and mux ASS and SRT subtitles in MKV files. Built for Psyche but usable as a standalone CLI or Rust library.

No cloud required. No telemetry. Every translation provider is opt-in.

## Features

- Extract ASS and SRT subtitle tracks from MKV files via mkvmerge/mkvextract
- Translate subtitle dialogue through 7 pluggable providers
- Protect ASS override tags (`{\pos(...)}`, `{\an7}`, etc.) during translation
- Automatic chunking (200 lines per request) for LLM context limits
- Concurrent chunk translation with configurable parallelism
- Retry with exponential backoff on HTTP, provider, and malformed output errors
- Mux translated subtitles back into the MKV, replacing the original track
- Process single files or entire directories
- Translate standalone `.ass` and `.srt` files without MKV (via `translate-ass` subcommand)
- Resume interrupted translations with `--resume`

## Supported Providers

| Provider | Flag | Auth | `--parallel` | Notes |
|----------|------|------|-------------|-------|
| [Ollama](https://ollama.com) | `--provider ollama` | None | 3 | Default. Any Ollama model. |
| [Anthropic](https://docs.anthropic.com) | `--provider anthropic` | `--api-key` | 2 | Messages API. Custom endpoint via `--anthropic-url`. |
| [OpenAI](https://platform.openai.com) | `--provider openai` | `--api-key` | 2 | Compatible with any OpenAI-compatible API. |
| [OpenRouter](https://openrouter.ai) | `--provider openrouter` | `--api-key` | 2 | 400+ models, including free models. |
| [DeepL](https://www.deepl.com) | `--provider deepl` | `--api-key` | 5 | Free tier (500K chars/month) or pro tier. |
| [Google Translate](https://cloud.google.com/translate) | `--provider google` | `--api-key` | 10 | v2 API. First 500K chars/month free. |
| [Gemini](https://ai.google.dev/gemini-api/docs) | `--provider gemini` | `--api-key` | 2 | LLM-based. 1,500 req/day free on Flash models. |

The `--parallel` column shows recommended concurrency for each provider.

## Installation

```sh
cargo install --path .
```

Or build from source:

```sh
cargo build --release
```

### Requirements

- `mkvmerge` and `mkvextract` from [MKVToolNix](https://mkvtoolnix.download/) must be in your `PATH`.

## CLI Usage

### Inspect MKV tracks

```sh
psyche-subtitle-toolkit inspect episode.mkv
```

Output shows all tracks with a `*` marking the selected ASS subtitle track:

```
* track 2: type=subtitles codec=SubStationAlpha language=eng name=HIDIVE_English
  track 3: type=subtitles codec=SubStationAlpha language=jpn name=
```

### Translate subtitles

```sh
# Ollama (default, local)
psyche-subtitle-toolkit translate --input episode.mkv --to pt-BR --model gemma4:31b-cloud

# OpenAI
psyche-subtitle-toolkit translate --provider openai --api-key sk-... --model gpt-4o-mini --input episode.mkv --to pt-BR

# DeepL (free tier)
psyche-subtitle-toolkit translate --provider deepl --api-key YOUR_KEY --input episode.mkv --to pt-BR

# Google Translate
psyche-subtitle-toolkit translate --provider google --api-key YOUR_KEY --input episode.mkv --to pt

# Gemini
psyche-subtitle-toolkit translate --provider gemini --api-key YOUR_KEY --model gemini-2.5-flash-lite --input episode.mkv --to pt-BR

# OpenRouter (free model)
psyche-subtitle-toolkit translate --provider openrouter --api-key YOUR_KEY --model meta-llama/llama-3.3-70b-instruct:free --input episode.mkv --to pt-BR
```

### Translate standalone subtitle files

```sh
# ASS file
psyche-subtitle-toolkit translate-ass --input source.ass --output translated.ass --to pt-BR --provider deepl --api-key YOUR_KEY

# SRT file (auto-detected by extension or content)
psyche-subtitle-toolkit translate-ass --input source.srt --output translated.srt --to pt-BR --provider deepl --api-key YOUR_KEY
```

### Resume interrupted translations

If a batch run is interrupted (crash, network failure), restart with `--resume` to skip already-translated files:

```sh
# First run — interrupted at file 15/20
psyche-subtitle-toolkit translate --resume --provider ollama --input /media/anime/ --to pt-BR

# Restart — skips files 1-14, continues from 15
psyche-subtitle-toolkit translate --resume --provider ollama --input /media/anime/ --to pt-BR
```

Progress is saved to `.psyche-subtitle-toolkit-progress.json` in the input directory and auto-deleted when all files complete.

### Full options

```
-i, --input <INPUT>          MKV file or directory containing MKV files
    --to <TO>                Target language code (e.g. pt-BR, en, ja)
    --provider <PROVIDER>    Translation backend [default: ollama]
    --track <TRACK>          Specific subtitle track ID to translate
    --model <MODEL>          Model name [default: llama3.1]
    --ollama-url <URL>       Ollama base URL [default: http://localhost:11434]
    --openai-url <URL>       OpenAI base URL [default: https://api.openai.com]
    --anthropic-url <URL>    Anthropic base URL [default: https://api.anthropic.com]
    --api-key <KEY>          API key (required for openai, openrouter, anthropic, deepl, google, gemini)
    --deepl-url <URL>        DeepL base URL [default: https://api-free.deepl.com]
    --keep-temp              Preserve extracted/translated ASS files
    --dry-run                Show what would be translated without modifying files
    --source-lang <LANG>     Source language code (e.g. en, ja)
    --resume                 Save progress and skip already-translated files on restart
    --parallel <N>           Max concurrent chunk translations [default: 1]
```

## Library Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
psyche-subtitle-toolkit = { path = "../psyche-subtitle-toolkit" }
```

### Translate an MKV file

```rust
use std::sync::Arc;
use psyche_subtitle_toolkit::{translate_mkv, TranslateMkvOptions, OllamaTranslator, Translator};

# async fn example() -> psyche_subtitle_toolkit::Result<()> {
let translator: Arc<dyn Translator> = Arc::new(OllamaTranslator::new("gemma4:31b-cloud")?);
translate_mkv(
    TranslateMkvOptions {
        input: "/media/anime/episode.mkv".into(),
        target_language: "pt-BR".into(),
        track_id: None,
        keep_temp: false,
        dry_run: false,
        source_language: Some("en".into()),
        resume: false,
        max_concurrent: 3,
    },
    translator,
).await?;
# Ok(())
# }
```

### Translate ASS content directly

```rust
use std::sync::Arc;
use psyche_subtitle_toolkit::{translate_ass, AssSubtitle, OllamaTranslator, Translator};

# async fn example() -> psyche_subtitle_toolkit::Result<()> {
let ass = AssSubtitle::parse(&std::fs::read_to_string("source.ass")?)?;
let translator: Arc<dyn Translator> = Arc::new(OllamaTranslator::new("llama3.1")?);
let translated = translate_ass(ass, "pt-BR", Some("en"), 1, translator).await?;
std::fs::write("translated.ass", translated.render())?;
# Ok(())
# }
```

### Implement a custom provider

```rust
use async_trait::async_trait;
use psyche_subtitle_toolkit::{Translator, TranslationRequest, Result};

struct MyTranslator { /* ... */ }

#[async_trait]
impl Translator for MyTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        // Your translation logic here.
        // request.source_text is numbered: "<1> hello\n<2> world"
        // Return translated text in the same format.
        todo!()
    }
}
```

## How It Works

1. **Inspect** -- `mkvmerge -J` identifies tracks and selects the ASS or SRT subtitle
2. **Extract** -- `mkvextract tracks` pulls the ASS file to a temp directory
3. **Parse** -- The ASS parser reads dialogue lines, preserving headers and styles
4. **Strip tags** -- ASS override tags (`{\pos(...)}`, `{\an7}`) are removed and stored
5. **Chunk** -- Cues are split into 200-line batches
6. **Translate** -- Each chunk is sent to the provider as `<N> text` numbered lines (concurrent if `--parallel > 1`)
7. **Retry** -- Failed chunks (HTTP errors, malformed output) are retried up to 3 times with exponential backoff
8. **Apply** -- Translated text is mapped back to cues by ID
9. **Reinject tags** -- Original override tags are prepended back
10. **Mux** -- `mkvmerge` replaces the original subtitle track in-place

## Testing

```sh
cargo test           # 96 unit + 11 doc-tests
cargo clippy -- -D warnings
```

Provider tests use `wiremock` to mock HTTP endpoints -- no real API calls.

## License

[MIT](LICENSE)
