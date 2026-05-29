# AGENTS.md

Guide for AI coding agents working on psyche-subtitle-toolkit.

## Quick Reference

```sh
cargo check                                      # compilation check
cargo test                                       # run all tests (48 unit + integration)
cargo clippy --all-targets --all-features -- -D warnings  # strict lint
cargo build --release                            # release binary
cargo run -- inspect <mkv>                       # inspect MKV tracks
cargo run -- translate --help                    # show all translate options
```

All commands run from the project root.

## Project Overview

Rust crate + CLI that extracts ASS subtitles from MKV files, translates them via pluggable providers, and muxes the translation back in-place. Designed for Psyche but usable standalone.

**Key constraints:**
- ASS subtitle format only (no SRT, VTT, etc.)
- In-place MKV remuxing (replaces original track, does not append)
- Sequential file processing (one MKV at a time)
- Local-first: no telemetry, no cloud required, every provider is opt-in

## Project Structure

```
src/
  lib.rs              # crate root: module declarations + public re-exports
  main.rs             # CLI binary (clap): inspect + translate subcommands
  error.rs            # thiserror enum: SubtitleToolkitError
  pipeline.rs         # translate_mkv (MKV pipeline) + translate_ass (subtitle-only pipeline)
  media/
    mod.rs
    mkv.rs            # mkvmerge/mkvextract wrappers: inspect, extract, mux
  subtitles/
    mod.rs
    model.rs          # SubtitleDocument, SubtitleCue
    ass.rs            # ASS parser + renderer (AssSubtitle)
    structured.rs     # numbered text format, tag stripping, chunking
  translation/
    mod.rs            # Translator trait + TranslationRequest struct
    ollama.rs         # Ollama /api/generate provider
    openai.rs         # OpenAI /v1/chat/completions provider
    deepl.rs          # DeepL /v2/translate provider
    google.rs         # Google Translate v2 provider
    gemini.rs         # Gemini generateContent provider
    openrouter.rs     # OpenRouter /api/v1/chat/completions provider
```

## Architecture

### Pipeline Flow

```
MKV file → inspect → extract ASS → parse → strip tags → chunk → translate → apply → reinject → render → mux
```

Two entry points:
- `translate_mkv(options, &translator)` — full MKV pipeline (file I/O + external tools)
- `translate_ass(ass, target_language, &translator)` — subtitle-only pipeline (no file I/O)

### Translator Trait

```rust
#[async_trait]
pub trait Translator: Send + Sync {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String>;
}
```

- Input: numbered subtitle text (`<1> hello\n<2> world`) + target language code
- Output: translated text in the same `<N> text` format
- Each provider owns its own prompt construction and HTTP client
- Pipeline is generic over `T: Translator + ?Sized` (supports `dyn Translator`)

### Error Handling

All errors use `SubtitleToolkitError` (thiserror). Provider errors use:
```rust
Translation { provider: &'static str, message: String }
```

## Adding a New Provider

Five steps, no pipeline changes needed:

1. **Create `src/translation/foo.rs`** — struct implementing `Translator`
2. **Add `pub mod foo;`** to `src/translation/mod.rs`
3. **Add doc comment** to the `Translator` trait listing
4. **Add re-export** `pub use translation::foo::FooTranslator;` to `src/lib.rs`
5. **Add CLI support** in `src/main.rs`:
   - Add `use` import
   - Add `"foo"` to the `--provider` help string
   - Add match arm in the provider dispatch
   - Update the unknown provider error message

### Provider Implementation Template

Each provider file follows this structure:

```rust
use serde::{Deserialize, Serialize};
use crate::error::{Result, SubtitleToolkitError};
use super::{TranslationRequest, Translator};

pub struct FooTranslator {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl FooTranslator {
    pub fn new(api_key: impl Into<String>) -> Result<Self> { /* ... */ }
    pub fn with_base_url(base_url: impl Into<String>, api_key: impl Into<String>) -> Result<Self> { /* ... */ }
}

#[async_trait::async_trait]
impl Translator for FooTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        // 1. Send HTTP request
        // 2. Check status → Err(SubtitleToolkitError::Translation { provider: "foo", ... })
        // 3. Parse response
        // 4. Return translated text (trimmed)
    }
}

// Private request/response serde types

#[cfg(test)]
mod tests {
    // wiremock mock tests: happy path, auth header, error on non-200, edge cases
}
```

### Provider Design Rules

- Use `reqwest::Client` with 120s timeout
- Constructor returns `Result<Self>` (reqwest build can fail)
- `with_base_url` for custom endpoints; `new` for defaults
- Trim `.base_url` trailing slash
- Error on non-200 with `SubtitleToolkitError::Translation { provider, message }`
- Trim whitespace from response text
- Each provider owns its own prompt/request construction (no shared prompt module)

## Testing

### Test Categories (48 tests total)

| Category | Count | Location | What they test |
|---|---|---|---|
| Subtitle parsing | 2 | `subtitles/ass.rs` | ASS parse + render |
| Structured text | 15 | `subtitles/structured.rs` | Numbered format, tag strip/reinject, chunking |
| Pipeline integration | 7 | `pipeline.rs` | Full subtitle processing with FakeTranslator |
| Provider mocks | 5-6 each | `translation/*.rs` | HTTP contracts per provider |
| Doc-tests | 6 | `lib.rs` + provider files | Compile-check public examples |

### Writing Provider Tests

Use `wiremock` (dev-dependency) to mock HTTP:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn translates_numbered_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/expected/path"))
            .respond_with(ResponseTemplate::new(200).set_body_json(/* ... */))
            .mount(&server)
            .await;

        let translator = FooTranslator::with_base_url(server.uri(), "test-key").unwrap();
        let result = translator.translate(TranslationRequest {
            source_text: "<1> hello",
            target_language: "pt-BR",
        }).await.unwrap();

        assert_eq!(result, "<1> expected");
    }
}
```

### Writing Pipeline Tests

Pipeline tests use `FakeTranslator` (defined in `pipeline.rs::tests`) that records calls and returns configured responses. No external tools needed — tests the subtitle processing logic only.

### What Is NOT Tested

- MKV file I/O (requires mkvmerge/mkvextract + test media)
- CLI argument parsing (trivial clap derive)
- Real API calls (all providers use wiremock)

## Conventions

- **Edition:** Rust 2024
- **Safety:** `#![forbid(unsafe_code)]`
- **Errors:** `thiserror` only, no `anyhow`
- **Async:** `tokio` with `features = ["fs", "macros", "process", "rt-multi-thread"]`
- **HTTP:** `reqwest` with `json` + `rustls-tls` features, no default features
- **Lint:** `cargo clippy --all-targets --all-features -- -D warnings` must pass
- **Format:** `cargo fmt` (default rustfmt)
- **No unnecessary abstractions:** each provider is a standalone module, no factory/registry
- **No `Box<dyn>` in library API:** the pipeline uses generics (`T: Translator + ?Sized`); `Box<dyn>` is only in the CLI for provider dispatch

## External Dependencies

Runtime:
- `mkvmerge`, `mkvextract` (MKVToolNix) must be in PATH for MKV operations

Dev:
- `wiremock` for HTTP mock tests
