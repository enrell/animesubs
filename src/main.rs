use std::path::PathBuf;

use clap::{Parser, Subcommand};
use psyche_subtitle_toolkit::media::mkv::{inspect_mkv, select_ass_track};
use psyche_subtitle_toolkit::pipeline::{TranslateMkvOptions, translate_mkv};
use psyche_subtitle_toolkit::translation::ollama::OllamaTranslator;

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

        #[arg(long)]
        track: Option<u64>,

        #[arg(long, default_value = "llama3.1")]
        model: String,

        #[arg(long, default_value = "http://localhost:11434")]
        ollama_url: String,

        #[arg(long)]
        keep_temp: bool,
    },
}

#[tokio::main]
async fn main() -> psyche_subtitle_toolkit::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Inspect { input } => inspect(input).await,
        Command::Translate {
            input,
            to,
            track,
            model,
            ollama_url,
            keep_temp,
        } => {
            let translator = OllamaTranslator::with_base_url(ollama_url, model);
            translate_mkv(
                TranslateMkvOptions {
                    input,
                    target_language: to,
                    track_id: track,
                    keep_temp,
                },
                &translator,
            )
            .await
        }
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
