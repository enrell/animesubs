use std::path::PathBuf;

use clap::{Parser, Subcommand};
use psyche_subtitle_toolkit::media::mkv::{inspect_mkv, select_ass_track};
use psyche_subtitle_toolkit::AssSubtitle;
use psyche_subtitle_toolkit::pipeline::{TranslateMkvOptions, translate_ass, translate_mkv};
use psyche_subtitle_toolkit::translation::Translator;
use psyche_subtitle_toolkit::translation::deepl::DeepLTranslator;
use psyche_subtitle_toolkit::translation::gemini::GeminiTranslator;
use psyche_subtitle_toolkit::translation::google::GoogleTranslator;
use psyche_subtitle_toolkit::translation::ollama::OllamaTranslator;
use psyche_subtitle_toolkit::translation::openai::OpenAiTranslator;
use psyche_subtitle_toolkit::translation::openrouter::OpenRouterTranslator;

#[derive(Debug, Parser)]
#[command(name = "psyche-subtitle-toolkit")]
#[command(about = "Translate and mux local MKV subtitles for Psyche")]
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

        #[arg(long, default_value = "ollama", help = "Translation backend: ollama, openai, openrouter, deepl, google, gemini")]
        provider: String,

        #[arg(long)]
        track: Option<u64>,

        #[arg(long, default_value = "llama3.1")]
        model: String,

        #[arg(long, default_value = "http://localhost:11434")]
        ollama_url: String,

        #[arg(long, help = "API key (required for openai, openrouter, deepl, google, and gemini providers)")]
        api_key: Option<String>,

        #[arg(long, default_value = "https://api-free.deepl.com", help = "DeepL API base URL (free or pro tier)")]
        deepl_url: String,

        #[arg(long)]
        keep_temp: bool,

        #[arg(long, help = "Show what would be translated without modifying files")]
        dry_run: bool,

        #[arg(long, help = "Source language code (e.g. en, ja)")]
        source_lang: Option<String>,

        #[arg(long, help = "Save progress and skip already-translated files on restart")]
        resume: bool,

        #[arg(long, default_value = "1", help = "Max concurrent chunk translations (ollama: 3, deepl: 5, google: 10, openai/openrouter/gemini: 2)")]
        parallel: usize,
    },
    TranslateAss {
        #[arg(short, long)]
        input: PathBuf,

        #[arg(short, long)]
        output: PathBuf,

        #[arg(long)]
        to: String,

        #[arg(long, default_value = "ollama", help = "Translation backend: ollama, openai, openrouter, deepl, google, gemini")]
        provider: String,

        #[arg(long, default_value = "llama3.1")]
        model: String,

        #[arg(long, default_value = "http://localhost:11434")]
        ollama_url: String,

        #[arg(long, help = "API key (required for openai, openrouter, deepl, google, and gemini providers)")]
        api_key: Option<String>,

        #[arg(long, default_value = "https://api-free.deepl.com", help = "DeepL API base URL (free or pro tier)")]
        deepl_url: String,

        #[arg(long, help = "Source language code (e.g. en, ja)")]
        source_lang: Option<String>,
    },
}

#[tokio::main]
async fn main() -> psyche_subtitle_toolkit::Result<()> {
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
            source_lang,
        } => {
            let translator = build_translator(&provider, &model, &ollama_url, &deepl_url, api_key)?;
            let translator: std::sync::Arc<dyn psyche_subtitle_toolkit::Translator> =
                std::sync::Arc::from(translator);
            let source = tokio::fs::read_to_string(&input).await?;
            let ass = AssSubtitle::parse(&source)?;
            let translated = translate_ass(
                ass,
                &to,
                source_lang.as_deref(),
                1,
                translator,
            )
            .await?;
            tokio::fs::write(&output, translated.render()).await?;
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
            keep_temp,
            dry_run,
            source_lang,
            resume,
            parallel,
        } => {
            let translator = build_translator(&provider, &model, &ollama_url, &deepl_url, api_key)?;
            let translator: std::sync::Arc<dyn psyche_subtitle_toolkit::Translator> =
                std::sync::Arc::from(translator);
            translate_mkv(
                TranslateMkvOptions {
                    input,
                    target_language: to,
                    track_id: track,
                    keep_temp,
                    dry_run,
                    source_language: source_lang,
                    resume,
                    max_concurrent: parallel,
                },
                translator,
            )
            .await
        }
    }
}

fn build_translator(
    provider: &str,
    model: &str,
    ollama_url: &str,
    deepl_url: &str,
    api_key: Option<String>,
) -> psyche_subtitle_toolkit::Result<Box<dyn Translator>> {
    match provider {
        "ollama" => Ok(Box::new(OllamaTranslator::with_base_url(
            ollama_url, model,
        )?)),
        "openai" => {
            let key = api_key.ok_or_else(|| {
                psyche_subtitle_toolkit::SubtitleToolkitError::Translation {
                    provider: "openai",
                    message: "--api-key is required for the openai provider".into(),
                }
            })?;
            Ok(Box::new(OpenAiTranslator::new(key, model)?))
        }
        "deepl" => {
            let key = api_key.ok_or_else(|| {
                psyche_subtitle_toolkit::SubtitleToolkitError::Translation {
                    provider: "deepl",
                    message: "--api-key is required for the deepl provider".into(),
                }
            })?;
            Ok(Box::new(DeepLTranslator::with_base_url(deepl_url, key)?))
        }
        "google" => {
            let key = api_key.ok_or_else(|| {
                psyche_subtitle_toolkit::SubtitleToolkitError::Translation {
                    provider: "google",
                    message: "--api-key is required for the google provider".into(),
                }
            })?;
            Ok(Box::new(GoogleTranslator::new(key)?))
        }
        "gemini" => {
            let key = api_key.ok_or_else(|| {
                psyche_subtitle_toolkit::SubtitleToolkitError::Translation {
                    provider: "gemini",
                    message: "--api-key is required for the gemini provider".into(),
                }
            })?;
            Ok(Box::new(GeminiTranslator::new(key, model)?))
        }
        "openrouter" => {
            let key = api_key.ok_or_else(|| {
                psyche_subtitle_toolkit::SubtitleToolkitError::Translation {
                    provider: "openrouter",
                    message: "--api-key is required for the openrouter provider".into(),
                }
            })?;
            Ok(Box::new(OpenRouterTranslator::new(key, model)?))
        }
        unknown => Err(psyche_subtitle_toolkit::SubtitleToolkitError::Translation {
            provider: "cli",
            message: format!(
                "unknown provider `{unknown}`, supported: ollama, openai, openrouter, deepl, google, gemini"
            ),
        }),
    }
}

async fn inspect(input: PathBuf) -> psyche_subtitle_toolkit::Result<()> {
    let info = inspect_mkv(&input).await?;
    let selected = select_ass_track(&info, None).map(|track| track.id);

    for track in info.tracks {
        let marker = if Some(track.id) == selected { "*" } else { " " };
        println!(
            "{marker} track {}: type={} codec={} language={} name={}",
            track.id,
            track.track_type,
            track.codec.as_deref().unwrap_or("unknown"),
            track.properties.language.as_deref().unwrap_or("und"),
            track.properties.track_name.as_deref().unwrap_or("")
        );
    }

    Ok(())
}
