//! End-to-end tests of the `animesubs-cli` binary: standalone commands, the
//! server it runs, and the app's start-the-server flow through
//! `animesubs_client::ensure_server`.

#[path = "../../server/tests/mock_llm/mod.rs"]
mod mock_llm;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use animesubs_api::paths;
use animesubs_client::{EnsureOptions, ensure_server};

const BIN: &str = env!("CARGO_BIN_EXE_animesubs-cli");

fn cli(home: &Path) -> Command {
    let mut command = Command::new(BIN);
    command
        .arg("--home")
        .arg(home)
        // Keep the developer's real keys and server out of the tests.
        .env_remove("ANIMESUBS_TOKEN")
        .env_remove("ANIMESUBS_API_KEY")
        .env_remove("OPENAI_API_KEY");
    command
}

fn run(command: &mut Command) -> Output {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "command failed: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn write_srt(dir: &Path) -> PathBuf {
    let input = dir.join("ep01.srt");
    std::fs::write(&input, mock_llm::SRT).unwrap();
    input
}

/// `mkvmerge` stand-in that prints a fixed `mkvmerge -J` report.
#[cfg(unix)]
fn fake_mkvmerge_path(dir: &Path) -> String {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("fake-bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = bin.join("mkvmerge");
    let report = serde_json::json!({
        "tracks": [
            { "id": 0, "type": "video", "codec": "AVC/H.264/MPEG-4p10", "properties": {} },
            { "id": 2, "type": "subtitles", "codec": "SubStationAlpha",
              "properties": { "language": "jpn", "track_name": "Full", "default_track": true } },
            { "id": 3, "type": "subtitles", "codec": "HDMV PGS",
              "properties": { "language": "eng", "forced_track": true } }
        ]
    });
    std::fs::write(&script, format!("#!/bin/sh\ncat <<'EOF'\n{report}\nEOF\n")).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

#[cfg(unix)]
#[test]
fn tracks_runs_standalone_and_marks_the_selected_track() {
    let dir = tempfile::tempdir().unwrap();
    let video = dir.path().join("episode.mkv");
    std::fs::write(&video, b"fake").unwrap();
    let path = fake_mkvmerge_path(dir.path());

    let human = stdout(&run(cli(dir.path())
        .env("PATH", &path)
        .arg("tracks")
        .arg(&video)));
    assert!(
        human.contains("* track 2: ass codec=SubStationAlpha language=jpn name=Full [default]"),
        "{human}"
    );
    assert!(human.contains("  track 3: pgs"), "{human}");
    assert!(
        !human.contains("track 0"),
        "video tracks are not listed: {human}"
    );

    let json: serde_json::Value = serde_json::from_slice(
        &run(cli(dir.path())
            .env("PATH", &path)
            .args(["tracks", "--json"])
            .arg(&video))
        .stdout,
    )
    .unwrap();
    assert_eq!(json["selected"], 2);
    assert_eq!(json["tracks"][1]["format"], "pgs");
    assert_eq!(json["tracks"][1]["forced"], true);
}

#[test]
fn tracks_reports_missing_mkvmerge() {
    let dir = tempfile::tempdir().unwrap();
    let video = dir.path().join("episode.mkv");
    std::fs::write(&video, b"fake").unwrap();
    let output = cli(dir.path())
        .env("PATH", dir.path())
        .arg("tracks")
        .arg(&video)
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("mkvmerge"), "{stderr}");
}

#[tokio::test(flavor = "multi_thread")]
async fn translate_runs_standalone() {
    let llm = mock_llm::start().await;
    let dir = tempfile::tempdir().unwrap();
    let input = write_srt(dir.path());

    let output = run(cli(dir.path())
        .arg("translate")
        .arg(&input)
        .args(["--to", "pt-BR", "--provider", "openai", "--model", "m"])
        .args(["--base-url", &llm.uri()]));
    let written = PathBuf::from(stdout(&output).trim());
    assert_eq!(written, dir.path().join("ep01.pt-BR.srt"));
    let translated = std::fs::read_to_string(&written).unwrap();
    assert!(translated.contains("PT: Hello there"), "{translated}");

    // A second run refuses to clobber the result unless asked to.
    let again = cli(dir.path())
        .arg("translate")
        .arg(&input)
        .args(["--to", "pt-BR", "--provider", "openai", "--model", "m"])
        .args(["--base-url", &llm.uri()])
        .output()
        .unwrap();
    assert!(!again.status.success());
    assert!(String::from_utf8_lossy(&again.stderr).contains("already exists"));
}

struct ServeProcess(Child);

impl Drop for ServeProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn serve_then_drive_it_with_the_cli() {
    let llm = mock_llm::start().await;
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let input = write_srt(dir.path());

    let mut server = ServeProcess(
        cli(&home)
            .args(["serve", "--port", "0"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_until("server health", || {
        cli(&home)
            .arg("health")
            .output()
            .is_ok_and(|o| o.status.success())
    });
    let health: serde_json::Value =
        serde_json::from_slice(&run(cli(&home).arg("health")).stdout).unwrap();
    assert_eq!(health["service"], "animesubs");

    let output = run(cli(&home)
        .arg("translate")
        .arg(&input)
        .args([
            "--to",
            "es",
            "--provider",
            "openai",
            "--model",
            "m",
            "--server",
            "--json",
        ])
        .args(["--base-url", &llm.uri()]));
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["state"], "succeeded", "{status}");
    assert_eq!(status["progress"]["done"], 2);
    let translated = std::fs::read_to_string(dir.path().join("ep01.es.srt")).unwrap();
    assert!(translated.contains("PT: Hello there"));

    let id = status["id"].as_str().unwrap();
    let job: serde_json::Value =
        serde_json::from_slice(&run(cli(&home).args(["job", id])).stdout).unwrap();
    assert_eq!(job["id"], id);
    let jobs: serde_json::Value =
        serde_json::from_slice(&run(cli(&home).arg("jobs")).stdout).unwrap();
    assert_eq!(jobs.as_array().unwrap().len(), 1);

    run(cli(&home).arg("stop"));
    wait_until("server exit", || server.0.try_wait().unwrap().is_some());
    assert!(!paths::server_info_file(&home).exists());

    let after = cli(&home).arg("health").output().unwrap();
    assert!(!after.status.success());
}

/// The desktop app's flow: no server running, so start the bundled binary,
/// use it, and stop it on exit.
#[tokio::test(flavor = "multi_thread")]
async fn ensure_server_starts_owns_and_stops_a_server() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");

    let connection = ensure_server(EnsureOptions {
        server_binary: Some(BIN.into()),
        ..EnsureOptions::new(&home)
    })
    .await
    .unwrap();
    assert!(connection.is_owned());
    let pid = connection.owned.as_ref().and_then(|p| p.id()).unwrap();
    let health = connection.client.health().await.unwrap();
    assert_eq!(health.service, "animesubs");
    assert!(paths::log_file(&home).exists());

    // A second caller (e.g. the CLI) attaches to the same server.
    let second = ensure_server(EnsureOptions {
        server_binary: Some(BIN.into()),
        ..EnsureOptions::new(&home)
    })
    .await
    .unwrap();
    assert!(!second.is_owned());
    assert_eq!(second.client.base_url(), connection.client.base_url());
    second.close().await;
    assert!(
        connection.client.health().await.is_ok(),
        "borrowed close must not stop it"
    );

    connection.close().await;
    assert!(
        !paths::server_info_file(&home).exists(),
        "server {pid} left server.json"
    );
    assert!(animesubs_client::connect(&home).await.is_err());
}

/// What happens when the app is killed: its end of the server's stdin closes
/// and the server stops on its own instead of lingering.
#[test]
fn server_stops_when_its_parent_goes_away() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let mut server = ServeProcess(
        cli(&home)
            .args(["serve", "--port", "0", "--exit-on-stdin-eof"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_until("server health", || {
        cli(&home)
            .arg("health")
            .output()
            .is_ok_and(|o| o.status.success())
    });

    drop(server.0.stdin.take());
    wait_until("server exit", || server.0.try_wait().unwrap().is_some());
    assert!(server.0.wait().unwrap().success());
    assert!(!paths::server_info_file(&home).exists());
}

/// The app was killed while a job ran: the server finishes the job, stays up
/// while a client keeps calling, then stops by itself.
#[tokio::test(flavor = "multi_thread")]
async fn orphaned_server_finishes_its_jobs_then_stops() {
    let llm = mock_llm::start_slow(Duration::from_millis(1500)).await;
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    let input = write_srt(dir.path());
    let mut server = ServeProcess(
        cli(&home)
            .args([
                "serve",
                "--port",
                "0",
                "--exit-on-stdin-eof",
                "--orphan-grace",
                "1",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_until("server health", || {
        cli(&home)
            .arg("health")
            .output()
            .is_ok_and(|o| o.status.success())
    });

    let queued: serde_json::Value = serde_json::from_slice(
        &run(cli(&home)
            .arg("translate")
            .arg(&input)
            .args(["--to", "pt-BR", "--provider", "openai", "--model", "m"])
            .args(["--base-url", &llm.uri(), "--server", "--no-wait"]))
        .stdout,
    )
    .unwrap();
    let id = queued["id"].as_str().unwrap().to_string();

    // Parent dies mid-job.
    drop(server.0.stdin.take());
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        server.0.try_wait().unwrap().is_none(),
        "stopped with a job running"
    );

    // A client polling the job keeps the server attached past the grace period.
    let job_state = || -> String {
        let job: serde_json::Value =
            serde_json::from_slice(&run(cli(&home).args(["job", &id])).stdout).unwrap();
        job["state"].as_str().unwrap().to_string()
    };
    wait_until("job success", || job_state() == "succeeded");
    let keep_attached_until = Instant::now() + Duration::from_millis(2500);
    while Instant::now() < keep_attached_until {
        assert_eq!(job_state(), "succeeded");
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(
        server.0.try_wait().unwrap().is_none(),
        "stopped while a client was attached"
    );

    // Nobody calls any more: it stops on its own.
    wait_until("server exit", || server.0.try_wait().unwrap().is_some());
    assert!(server.0.wait().unwrap().success());
    assert!(!paths::server_info_file(&home).exists());
    let translated = std::fs::read_to_string(dir.path().join("ep01.pt-BR.srt")).unwrap();
    assert!(translated.contains("PT: Hello there"));
}
