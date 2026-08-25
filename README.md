# psyche-subtitle-toolkit

Extract, translate, and mux ASS, SRT, WebVTT, and PGS (bitmap) subtitles in MKV files. Usable as a standalone CLI or Rust library.

No cloud required. No telemetry. Every translation provider is opt-in.

## Features

- Extract ASS, SRT, WebVTT, and PGS subtitle tracks from MKV files via mkvmerge/mkvextract
- Translate subtitle dialogue through 7 pluggable providers
- Protect inline ASS, SRT, and WebVTT formatting during translation
- Automatic chunking by each provider's item and UTF-8 byte limits
- Concurrent chunk translation with configurable parallelism
- Retry transient failures with `Retry-After`, exponential backoff, and jitter; HTTP 503 retries for up to 2 hours with a 60-second delay cap
- Validate the candidate MKV and preserve track order, name, and flags before replacement
- Process single files or entire directories
- Translate standalone `.ass`, `.srt`, and `.vtt` files without MKV (via `translate-subtitle`; `translate-ass` remains an alias)
- OCR PGS bitmap subtitles via pinned, SHA-256-verified PaddleOCR PP-OCRv5 models
- Resume interrupted translations with a versioned input/configuration manifest

## Supported Providers

| Provider | Flag | Auth | `--parallel` | Notes |
|----------|------|------|-------------|-------|
| [Ollama](https://ollama.com) | `--provider ollama` | None | 3 | Default. Any Ollama model. |
| [Anthropic](https://docs.anthropic.com) | `--provider anthropic` | environment or `--api-key` | 2 | Messages API. Custom endpoint via `--anthropic-url`. |
| [OpenAI](https://platform.openai.com) | `--provider openai` | environment or `--api-key` | 2 | Compatible with OpenAI-style endpoints. |
| [OpenRouter](https://openrouter.ai) | `--provider openrouter` | environment or `--api-key` | 2 | Unified hosted-model API. |
| [DeepL](https://www.deepl.com) | `--provider deepl` | environment or `--api-key` | 5 | Free or pro endpoint. |
| [Google Translate](https://cloud.google.com/translate) | `--provider google` | environment or `--api-key` | 10 | Translation v2 API. |
| [Gemini](https://ai.google.dev/gemini-api/docs) | `--provider gemini` | environment or `--api-key` | 2 | `generateContent` API. |

The `--parallel` column shows recommended concurrency for each provider.

For credentials, prefer `PSYCHE_SUBTITLE_API_KEY` or the provider-specific variable: `OPENAI_API_KEY`, `OPENROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `DEEPL_API_KEY`, `GOOGLE_API_KEY`, or `GEMINI_API_KEY`. `--api-key` remains available for compatibility but can be visible in shell history and process listings.

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

Output shows all tracks with a `*` marking the track that translation will auto-select:

```
* track 2: type=subtitles codec=SubStationAlpha language=eng name=HIDIVE_English
  track 3: type=subtitles codec=SubStationAlpha language=jpn name=
```

### Translate subtitles

```sh
# Ollama (default, local)
psyche-subtitle-toolkit translate --input episode.mkv --to pt-BR --model gemma4:31b-cloud

# OpenAI (set OPENAI_API_KEY or PSYCHE_SUBTITLE_API_KEY first)
psyche-subtitle-toolkit translate --provider openai --model gpt-4o-mini --input episode.mkv --to pt-BR

# DeepL (free tier)
psyche-subtitle-toolkit translate --provider deepl --input episode.mkv --to pt-BR

# Google Translate
psyche-subtitle-toolkit translate --provider google --input episode.mkv --to pt

# Gemini
psyche-subtitle-toolkit translate --provider gemini --model gemini-2.5-flash-lite --input episode.mkv --to pt-BR

# OpenRouter (free model)
psyche-subtitle-toolkit translate --provider openrouter --model meta-llama/llama-3.3-70b-instruct:free --input episode.mkv --to pt-BR
```

### Translate standalone subtitle files

```sh
# ASS file
psyche-subtitle-toolkit translate-subtitle --input source.ass --output translated.ass --to pt-BR --provider deepl

# SRT file (auto-detected by extension or content)
psyche-subtitle-toolkit translate-subtitle --input source.srt --output translated.srt --to pt-BR --provider deepl

# WebVTT file (auto-detected by extension or WEBVTT header)
psyche-subtitle-toolkit translate-subtitle --input source.vtt --output translated.vtt --to pt-BR --provider deepl
```

### Resume interrupted translations

If a batch run is interrupted (crash, network failure), restart with `--resume` to skip already-translated files:

```sh
# First run — interrupted at file 15/20
psyche-subtitle-toolkit translate --resume --provider ollama --input /media/anime/ --to pt-BR

# Restart — skips files 1-14, continues from 15
psyche-subtitle-toolkit translate --resume --provider ollama --input /media/anime/ --to pt-BR
```

Progress is saved to a versioned JSON manifest and auto-deleted when all files complete. It records the target, provider/model identity, track choice, canonical path, size, and modification time. Each file is marked pending before translation; if the MKV changes before the completion checkpoint is written, resume stops for manual inspection instead of risking a second remux. A stale manifest from a different run is rejected instead of silently mixing outputs.

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
    --api-key <KEY>          API key; environment variables are preferred
    --deepl-url <URL>        DeepL base URL [default: https://api-free.deepl.com]
    --keep-temp              Preserve extracted and translated subtitle files
    --dry-run                Show what would be translated without modifying files
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
let translated = translate_ass(ass, "pt-BR", 1, translator).await?;
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

1. **Inspect** -- `mkvmerge -J` identifies tracks and selects the ASS, SRT, or VTT subtitle
2. **Extract** -- `mkvextract tracks` pulls the ASS file to a temp directory
3. **Parse** -- The format parser preserves timing, styles, identifiers, and VTT metadata blocks
4. **Protect tags** -- Inline formatting is replaced by validated opaque tokens
5. **Chunk** -- Cues are split according to provider item and byte limits
6. **Translate** -- Each chunk is sent to the provider as `<N> text` numbered lines (concurrent if `--parallel > 1`)
7. **Retry** -- Only transient failures and malformed model output are retried; permanent 4xx errors fail immediately, while HTTP 503 keeps retrying for up to 2 hours with at most 60 seconds between attempts
8. **Apply** -- Translated text is mapped back to cues by ID
9. **Restore tags** -- Protected formatting is checked for presence/order and restored
10. **Mux** -- A temporary MKV is inspected before a rollback-capable replacement

## Testing

```sh
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

Provider tests use `wiremock` to mock HTTP endpoints -- no real API calls.

## Release Notes

### v0.3.0

- **PGS OCR** — bitmap subtitle recognition via PaddleOCR PP-OCRv5 (auto-downloads models)
- PGS track auto-detection in MKV files
- New dependencies: `pgs-rs`, `ocr-rs`, `image`, `imageproc`

### v0.2.0

- **SRT support** — parse, translate, and render SubRip subtitles
- **WebVTT support** — parse, translate, and render WebVTT subtitles
- **Anthropic provider** — Messages API (`/v1/messages`) with custom endpoint support
- **Remove `--source-lang`** — all providers auto-detect source language, making the flag redundant
- **Format auto-detection** — `translate-ass` CLI auto-detects ASS/SRT/VTT by extension or content
- **MKV format priority** — ASS > SRT > VTT when multiple subtitle tracks exist
- **Refactored pipeline** — `translate_document()` helper shared by ASS, SRT, and VTT pipelines

### v0.1.0

Initial release:

- 7 translation providers (Ollama, OpenAI, OpenRouter, DeepL, Google, Gemini)
- `--parallel N` for concurrent chunk translation
- `--resume` for interrupted batch recovery
- `--dry-run` to preview without modifying files
- Retry with exponential backoff on HTTP and malformed output errors
- DeepL/Google batch mode (per-line array elements)
- 200 lines per chunk
- Progress output to stderr
- `translate-ass` subcommand for standalone subtitle files

## License

[MIT](LICENSE)
