use std::path::PathBuf;

use clap::{Parser, Subcommand};
use animesubs_engine::AssSubtitle;
use animesubs_engine::media::mkv::{inspect_mkv, select_subtitle_track};
use animesubs_engine::pipeline::{TranslateMkvOptions, translate_ass, translate_mkv};
use animesubs_engine::translation::Translator;
use animesubs_engine::translation::anthropic::AnthropicTranslator;
use animesubs_engine::translation::deepl::DeepLTranslator;
use animesubs_engine::translation::gemini::GeminiTranslator;
use animesubs_engine::translation::google::GoogleTranslator;
use animesubs_engine::translation::ollama::OllamaTranslator;
use animesubs_engine::translation::openai::OpenAiTranslator;
use animesubs_engine::translation::opencode::{OpenCodeGoTranslator, OpenCodeZenTranslator};
use animesubs_engine::translation::openrouter::OpenRouterTranslator;

#[derive(Debug, Parser)]
#[command(name = "animesubs-engine")]
#[command(about = "Translate and mux local MKV subtitles")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Inspect {
        input: PathBuf,
    },
    Translate {
        #[arg(short, long)]
        input: PathBuf,

        #[arg(long)]
        to: String,

        #[arg(
            long,
            default_value = "ollama",
            help = "Translation backend: ollama, openai, \
                opencode-zen, opencode-go, openrouter, anthropic, deepl, google, gemini"
        )]
        provider: String,

        #[arg(long)]
        track: Option<u64>,

        #[arg(long, default_value = "llama3.1")]
        model: String,

        #[arg(long, default_value = "http://localhost:11434")]
        ollama_url: String,

        #[arg(
            long,
            help = "API key; prefer PSYCHE_SUBTITLE_API_KEY or the provider-specific environment variable"
        )]
        api_key: Option<String>,

        #[arg(
            long,
            default_value = "https://api-free.deepl.com",
            help = "DeepL API base URL (free or pro tier)"
        )]
        deepl_url: String,

        #[arg(
            long,
            default_value = "https://api.openai.com",
            help = "OpenAI API base URL (any OpenAI-compatible endpoint)"
        )]
        openai_url: String,

        #[arg(
            long,
            default_value = "https://opencode.ai/zen",
            help = "OpenCode Zen base URL"
        )]
        opencode_zen_url: String,

        #[arg(
            long,
            default_value = "https://opencode.ai/zen/go",
            help = "OpenCode Go base URL"
        )]
        opencode_go_url: String,

        #[arg(
            long,
            default_value = "https://api.anthropic.com",
            help = "Anthropic API base URL"
        )]
        anthropic_url: String,

        #[arg(long)]
        keep_temp: bool,

        #[arg(long, help = "Show what would be translated without modifying files")]
        dry_run: bool,

        #[arg(
            long,
            help = "Save progress and skip already-translated files on restart"
        )]
        resume: bool,

        #[arg(
            long,
            default_value = "1",
            help = "Max concurrent chunk translations (ollama: 3, deepl: 5, google: 10, \
                openai/opencode-zen/opencode-go/openrouter/gemini: 2)"
        )]
        parallel: usize,
    },
    #[command(name = "translate-subtitle", alias = "translate-ass")]
    TranslateAss {
        #[arg(short, long)]
        input: PathBuf,

        #[arg(short, long)]
        output: PathBuf,

        #[arg(long)]
        to: String,

        #[arg(
            long,
            default_value = "ollama",
            help = "Translation backend: ollama, openai, \
                opencode-zen, opencode-go, openrouter, anthropic, deepl, google, gemini"
        )]
        provider: String,

        #[arg(long, default_value = "llama3.1")]
        model: String,

        #[arg(long, default_value = "http://localhost:11434")]
        ollama_url: String,

        #[arg(
            long,
            help = "API key; prefer PSYCHE_SUBTITLE_API_KEY or the provider-specific environment variable"
        )]
        api_key: Option<String>,

        #[arg(
            long,
            default_value = "https://api-free.deepl.com",
            help = "DeepL API base URL (free or pro tier)"
        )]
        deepl_url: String,

        #[arg(
            long,
            default_value = "https://api.openai.com",
            help = "OpenAI API base URL (any OpenAI-compatible endpoint)"
        )]
        openai_url: String,

        #[arg(
            long,
            default_value = "https://opencode.ai/zen",
            help = "OpenCode Zen base URL"
        )]
        opencode_zen_url: String,

        #[arg(
            long,
            default_value = "https://opencode.ai/zen/go",
            help = "OpenCode Go base URL"
        )]
        opencode_go_url: String,

        #[arg(
            long,
            default_value = "https://api.anthropic.com",
            help = "Anthropic API base URL"
        )]
        anthropic_url: String,
    },
}

#[tokio::main]
async fn main() -> animesubs_engine::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Inspect { input } => inspect(input).await,
        Command::TranslateAss {
            input,
            output,
            to,
            provider,
            model,
            ollama_url,
            api_key,
            deepl_url,
            openai_url,
            opencode_zen_url,
            opencode_go_url,
            anthropic_url,
        } => {
            let urls = ProviderUrls {
                ollama: &ollama_url,
                deepl: &deepl_url,
                openai: &openai_url,
                opencode_zen: &opencode_zen_url,
                opencode_go: &opencode_go_url,
                anthropic: &anthropic_url,
            };
            let translator = build_translator(&provider, &model, &urls, api_key)?;
            let translator: std::sync::Arc<dyn animesubs_engine::Translator> =
                std::sync::Arc::from(translator);
            let source = tokio::fs::read_to_string(&input).await?;

            // Detect format: extension first, then content
            let ext = input.extension().and_then(|e| e.to_str()).unwrap_or("");
            let is_vtt =
                ext.eq_ignore_ascii_case("vtt") || source.trim_start().starts_with("WEBVTT");
            let is_srt = !is_vtt
                && (ext.eq_ignore_ascii_case("srt")
                    || (!source.trim_start().starts_with('[') && source.contains("-->")));

            if is_vtt {
                let vtt = animesubs_engine::VttSubtitle::parse(&source)?;
                let translated =
                    animesubs_engine::translate_vtt(vtt, &to, 1, translator).await?;
                tokio::fs::write(&output, translated.render()).await?;
            } else if is_srt {
                let srt = animesubs_engine::SrtSubtitle::parse(&source)?;
                let translated =
                    animesubs_engine::translate_srt(srt, &to, 1, translator).await?;
                tokio::fs::write(&output, translated.render()).await?;
            } else {
                let ass = AssSubtitle::parse(&source)?;
                let translated = translate_ass(ass, &to, 1, translator).await?;
                tokio::fs::write(&output, translated.render()).await?;
            }
            eprintln!("[translate-ass] written to {}", output.display());
            Ok(())
        }
        Command::Translate {
            input,
            to,
            provider,
            track,
            model,
            ollama_url,
            api_key,
            deepl_url,
            openai_url,
            opencode_zen_url,
            opencode_go_url,
            anthropic_url,
            keep_temp,
            dry_run,
            resume,
            parallel,
        } => {
            let urls = ProviderUrls {
                ollama: &ollama_url,
                deepl: &deepl_url,
                openai: &openai_url,
                opencode_zen: &opencode_zen_url,
                opencode_go: &opencode_go_url,
                anthropic: &anthropic_url,
            };
            let translator = build_translator(&provider, &model, &urls, api_key)?;
            let translator: std::sync::Arc<dyn animesubs_engine::Translator> =
                std::sync::Arc::from(translator);
            translate_mkv(
                TranslateMkvOptions {
                    input,
                    target_language: to,
                    track_id: track,
                    keep_temp,
                    dry_run,
                    resume,
                    max_concurrent: parallel,
                },
                translator,
            )
            .await
        }
    }
}

struct ProviderUrls<'a> {
    ollama: &'a str,
    deepl: &'a str,
    openai: &'a str,
    opencode_zen: &'a str,
    opencode_go: &'a str,
    anthropic: &'a str,
}

fn build_translator(
    provider: &str,
    model: &str,
    urls: &ProviderUrls<'_>,
    api_key: Option<String>,
) -> animesubs_engine::Result<Box<dyn Translator>> {
    match provider {
        "ollama" => Ok(Box::new(OllamaTranslator::with_base_url(
            urls.ollama,
            model,
        )?)),
        "openai" => {
            let key = require_api_key("openai", api_key)?;
            Ok(Box::new(OpenAiTranslator::with_base_url(
                urls.openai,
                key,
                model,
            )?))
        }
        "opencode-zen" => {
            let key = require_api_key("opencode-zen", api_key)?;
            Ok(Box::new(OpenCodeZenTranslator::with_base_url(
                urls.opencode_zen,
                key,
                model,
            )?))
        }
        "opencode-go" => {
            let key = require_api_key("opencode-go", api_key)?;
            Ok(Box::new(OpenCodeGoTranslator::with_base_url(
                urls.opencode_go,
                key,
                model,
            )?))
        }
        "openrouter" => {
            let key = require_api_key("openrouter", api_key)?;
            Ok(Box::new(OpenRouterTranslator::new(key, model)?))
        }
        "anthropic" => {
            let key = require_api_key("anthropic", api_key)?;
            Ok(Box::new(AnthropicTranslator::with_base_url(
                urls.anthropic,
                key,
                model,
            )?))
        }
        "deepl" => {
            let key = require_api_key("deepl", api_key)?;
            Ok(Box::new(DeepLTranslator::with_base_url(urls.deepl, key)?))
        }
        "google" => {
            let key = require_api_key("google", api_key)?;
            Ok(Box::new(GoogleTranslator::new(key)?))
        }
        "gemini" => {
            let key = require_api_key("gemini", api_key)?;
            Ok(Box::new(GeminiTranslator::new(key, model)?))
        }
        unknown => Err(animesubs_engine::SubtitleToolkitError::Translation {
            provider: "cli",
            message: format!(
                "unknown provider `{unknown}`, supported: \
                    ollama, openai, opencode-zen, opencode-go, openrouter, anthropic, deepl, google, gemini"
            ),
        }),
    }
}

/// Returns the API key or a `SubtitleToolkitError::Translation` reporting which
/// provider required it. Used by all non-Ollama providers in [`build_translator`].
fn require_api_key(
    provider: &'static str,
    api_key: Option<String>,
) -> animesubs_engine::Result<String> {
    let provider_env = match provider {
        "openai" => "OPENAI_API_KEY",
        "opencode-zen" => "OPENCODE_ZEN_API_KEY",
        "opencode-go" => "OPENCODE_GO_API_KEY",
        "openrouter" => "OPENROUTER_API_KEY",
        "anthropic" => "ANTHROPIC_API_KEY",
        "deepl" => "DEEPL_API_KEY",
        "google" => "GOOGLE_API_KEY",
        "gemini" => "GEMINI_API_KEY",
        _ => "PSYCHE_SUBTITLE_API_KEY",
    };
    api_key
        .or_else(|| nonempty_env("PSYCHE_SUBTITLE_API_KEY"))
        .or_else(|| nonempty_env(provider_env))
        .ok_or_else(|| animesubs_engine::SubtitleToolkitError::Translation {
        provider,
        message: format!(
            "an API key is required for {provider}; set PSYCHE_SUBTITLE_API_KEY or {provider_env}"
        ),
    })
}

fn nonempty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

async fn inspect(input: PathBuf) -> animesubs_engine::Result<()> {
    let info = inspect_mkv(&input).await?;
    let selected_id = select_subtitle_track(&info, None).map(|(track, _)| track.id);

    for track in &info.tracks {
        let marker = if selected_id == Some(track.id) {
            "*"
        } else {
            " "
        };
        let name = track.properties.track_name.as_deref().unwrap_or_default();
        println!(
            "{} track {}: type={} codec={} language={} name={}",
            marker,
            track.id,
            track.track_type,
            track.codec.as_deref().unwrap_or("unknown"),
            track.properties.language.as_deref().unwrap_or("unknown"),
            name,
        );
    }

    if let Some((track, format)) = select_subtitle_track(&info, None) {
        println!(
            "\nAuto-selected track {} ({format:?}) for translation",
            track.id
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_translate_ass_alias_still_parses() {
        let cli = Cli::try_parse_from([
            "animesubs-engine",
            "translate-ass",
            "--input",
            "source.srt",
            "--output",
            "translated.srt",
            "--to",
            "pt-BR",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::TranslateAss { .. }));
    }

    #[test]
    fn preferred_translate_subtitle_name_parses() {
        let cli = Cli::try_parse_from([
            "animesubs-engine",
            "translate-subtitle",
            "--input",
            "source.vtt",
            "--output",
            "translated.vtt",
            "--to",
            "pt-BR",
        ])
        .unwrap();
        assert!(matches!(cli.command, Command::TranslateAss { .. }));
    }

    #[test]
    fn opencode_providers_use_first_class_dispatch() {
        let urls = ProviderUrls {
            ollama: "http://ollama",
            deepl: "http://deepl",
            openai: "http://openai",
            opencode_zen: "http://zen",
            opencode_go: "http://go",
            anthropic: "http://anthropic",
        };
        let zen = build_translator(
            "opencode-zen",
            "x-preview-f-free",
            &urls,
            Some("test-key".into()),
        )
        .unwrap();
        let go = build_translator(
            "opencode-go",
            "kimi-k2.7-code",
            &urls,
            Some("test-key".into()),
        )
        .unwrap();

        assert!(zen.identifier().starts_with("opencode-zen:http://zen"));
        assert!(go.identifier().starts_with("opencode-go:http://go"));
    }
}
