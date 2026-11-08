//! `animesubs-cli`: the animesubs engine from the command line.
//!
//! `tracks` and `translate` run the engine in this process by default, so no
//! server is needed for scripts. With `--server` they go through the local
//! server instead (the one the desktop app uses), which is how jobs show up
//! in the app and survive the terminal closing. `serve` runs that server.

#![deny(unsafe_code)]

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use animesubs_api::{
    DEFAULT_PORT, JobSpec, JobState, JobStatus, ProviderConfig, ProviderKind, TOKEN_ENV,
    TracksResponse, TranslateSubtitleJob, paths,
};
use animesubs_server::progress::JobProgress;
use animesubs_server::{ServerConfig, ops, providers};
use clap::{Args, Parser, Subcommand};

type CliResult<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Debug, Parser)]
#[command(
    name = "animesubs-cli",
    version,
    about = "Extract, inspect and translate subtitles"
)]
struct Cli {
    /// Server home directory (token, discovery file, logs). Defaults to
    /// $ANIMESUBS_HOME, else the desktop app's config directory.
    #[arg(long, global = true, value_name = "DIR")]
    home: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the local server in the foreground (loopback only).
    Serve {
        /// Port to listen on; 0 picks a free one.
        #[arg(long, default_value_t = DEFAULT_PORT)]
        port: u16,
        /// Stop when stdin closes (used by the desktop app, so the server
        /// does not outlive it).
        #[arg(long)]
        exit_on_stdin_eof: bool,
    },
    /// Print the running server's health as JSON.
    Health,
    /// List the subtitle tracks of a video file (needs mkvmerge).
    Tracks {
        file: PathBuf,
        /// Ask the running server instead of inspecting in this process.
        #[arg(long)]
        server: bool,
        /// Print JSON.
        #[arg(long)]
        json: bool,
    },
    /// Translate a .ass/.ssa/.srt/.vtt file into a new file.
    Translate(TranslateArgs),
    /// Print one server job as JSON.
    Job { id: String },
    /// Print all server jobs as JSON.
    Jobs,
    /// Stop the running server.
    Stop,
}

#[derive(Debug, Args)]
struct TranslateArgs {
    input: PathBuf,
    /// Target language tag, e.g. pt-BR.
    #[arg(long)]
    to: String,
    /// Output file. Defaults to <input stem>.<lang>.<ext> next to the input.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// ollama, openai, opencode-zen, opencode-go, openrouter, anthropic, deepl, google, gemini.
    #[arg(long, default_value = "ollama")]
    provider: ProviderKind,
    #[arg(long)]
    model: Option<String>,
    /// Provider API origin, e.g. http://localhost:1234 for LM Studio.
    #[arg(long)]
    base_url: Option<String>,
    /// API key. Prefer ANIMESUBS_API_KEY or the provider's variable: flags show up
    /// in shell history and process listings.
    #[arg(long)]
    api_key: Option<String>,
    /// Concurrent chunk requests.
    #[arg(long)]
    parallel: Option<usize>,
    /// Replace the output file if it exists.
    #[arg(long)]
    overwrite: bool,
    /// Submit a job to the running server instead of translating in this process.
    #[arg(long)]
    server: bool,
    /// With --server: print the queued job and return without waiting.
    #[arg(long, requires = "server")]
    no_wait: bool,
    /// Print the final job status as JSON.
    #[arg(long)]
    json: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> CliResult {
    let home = cli
        .home
        .or_else(paths::default_home)
        .ok_or("could not determine the animesubs home directory; pass --home")?;

    match cli.command {
        Command::Serve {
            port,
            exit_on_stdin_eof,
        } => {
            let mut config = ServerConfig::local(home, port);
            config.token = std::env::var(TOKEN_ENV)
                .ok()
                .filter(|t| !t.trim().is_empty());
            config.stop_on_stdin_eof = exit_on_stdin_eof;
            animesubs_server::run(config).await?;
        }
        Command::Health => {
            let client = animesubs_client::connect(&home).await?;
            print_json(&client.health().await?)?;
        }
        Command::Tracks { file, server, json } => {
            let file = std::path::absolute(&file)?;
            let tracks = if server {
                animesubs_client::connect(&home).await?.tracks(file).await?
            } else {
                ops::list_tracks(&file).await?
            };
            if json {
                print_json(&tracks)?;
            } else {
                print_tracks(&tracks);
            }
        }
        Command::Translate(args) => translate(&home, args).await?,
        Command::Job { id } => {
            print_json(&animesubs_client::connect(&home).await?.job(&id).await?)?;
        }
        Command::Jobs => {
            print_json(&animesubs_client::connect(&home).await?.jobs().await?)?;
        }
        Command::Stop => {
            animesubs_client::connect(&home).await?.shutdown().await?;
            eprintln!("server stopped");
        }
    }
    Ok(())
}

async fn translate(home: &Path, args: TranslateArgs) -> CliResult {
    let mut provider = ProviderConfig {
        kind: args.provider,
        model: args.model,
        base_url: args.base_url,
        api_key: args.api_key,
    };
    // Resolve here so a server started elsewhere does not need our environment.
    provider.api_key = providers::resolve_api_key(&provider);
    let job = TranslateSubtitleJob {
        input: std::path::absolute(&args.input)?,
        output: args.output.map(std::path::absolute).transpose()?,
        target_language: args.to,
        provider,
        parallel: args.parallel,
        overwrite: args.overwrite,
    };

    if !args.server {
        let plan = ops::plan_translate(job)?;
        let progress = Arc::new(JobProgress::new("starting"));
        let output = ops::run_translate(&plan, progress.clone()).await?;
        if args.json {
            print_json(&serde_json::json!({ "output": output, "progress": progress.snapshot() }))?;
        } else {
            println!("{}", output.display());
        }
        return Ok(());
    }

    let client = animesubs_client::connect(home).await?;
    let queued = client.submit(&JobSpec::TranslateSubtitle(job)).await?;
    if args.no_wait {
        return print_json(&queued);
    }
    let mut last = String::new();
    let status = client
        .wait_for_job(
            &queued.id,
            Duration::from_millis(250),
            Duration::from_secs(24 * 60 * 60),
            |status| {
                let line = progress_line(status);
                if line != last {
                    eprintln!("{line}");
                    last = line;
                }
            },
        )
        .await?;
    if args.json {
        print_json(&status)?;
    }
    match (status.state, status.error) {
        (JobState::Succeeded, _) => {
            if !args.json
                && let Some(output) = &status.output
            {
                println!("{}", output.display());
            }
            Ok(())
        }
        (_, Some(error)) => Err(format!(
            "job {} failed: {} ({})",
            status.id, error.message, error.code
        )
        .into()),
        (state, None) => Err(format!("job {} ended as {state:?}", status.id).into()),
    }
}

fn progress_line(status: &JobStatus) -> String {
    match status.progress {
        Some(progress) => format!(
            "[{}] {} {}/{}",
            status.id, status.phase, progress.done, progress.total
        ),
        None => format!("[{}] {}", status.id, status.phase),
    }
}

fn print_tracks(tracks: &TracksResponse) {
    if tracks.tracks.is_empty() {
        println!("no subtitle tracks in {}", tracks.path.display());
        return;
    }
    for track in &tracks.tracks {
        let marker = if tracks.selected == Some(track.id) {
            '*'
        } else {
            ' '
        };
        let format = track
            .format
            .map(|format| format!("{format:?}").to_lowercase())
            .unwrap_or_else(|| "unsupported".into());
        let mut flags = Vec::new();
        if track.default {
            flags.push("default");
        }
        if track.forced {
            flags.push("forced");
        }
        println!(
            "{marker} track {}: {format} codec={} language={} name={}{}",
            track.id,
            track.codec.as_deref().unwrap_or("unknown"),
            track.language.as_deref().unwrap_or("und"),
            track.name.as_deref().unwrap_or(""),
            if flags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", flags.join(", "))
            },
        );
    }
}

fn print_json(value: &impl serde::Serialize) -> CliResult {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
