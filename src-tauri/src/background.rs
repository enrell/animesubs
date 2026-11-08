//! Keep a server the app started alive while its jobs run.
//!
//! Closing the window normally quits the app, and the app stops the server it
//! started. When that server still has queued or running jobs, the window is
//! hidden instead and a tray icon shows their progress. The app (and the
//! server) quit once the jobs finish. From the tray the user can reopen the
//! window or quit: right away when idle, otherwise "quit when finished" or
//! "stop jobs and quit" after a confirmation.
//!
//! If the app is killed instead, the server notices its stdin closing and
//! finishes the jobs on its own (`animesubs-cli serve --exit-on-stdin-eof`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use animesubs_api::{JobState, JobStatus};
use animesubs_client::Client;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Window, WindowEvent, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogResult};

use crate::commands::local_server::LocalServerState;

const TRAY_ID: &str = "background-jobs";
const STATUS_ITEM: &str = "background-status";
const SHOW_ITEM: &str = "background-show";
const QUIT_ITEM: &str = "background-quit";
const POLL: Duration = Duration::from_secs(1);

/// Background-mode flags, managed as Tauri state.
#[derive(Default)]
pub struct BackgroundState {
    /// The window is hidden and the app quits when the jobs finish.
    active: AtomicBool,
    monitor_running: AtomicBool,
    status_item: std::sync::Mutex<Option<MenuItem<Wry>>>,
}

/// What closing the window should do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDecision {
    /// As before: the window closes, the app quits, an owned server stops.
    Close,
    /// Hide to the tray until the jobs finish.
    KeepRunning,
}

pub fn close_decision(owns_server: bool, active_jobs: Option<usize>) -> CloseDecision {
    match (owns_server, active_jobs) {
        (true, Some(active)) if active > 0 => CloseDecision::KeepRunning,
        _ => CloseDecision::Close,
    }
}

/// Answer to "jobs are still running, quit?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitChoice {
    WhenFinished,
    Now,
    Cancel,
}

pub fn quit_choice(result: &MessageDialogResult, texts: &Texts) -> QuitChoice {
    match result {
        MessageDialogResult::Yes => QuitChoice::WhenFinished,
        MessageDialogResult::No => QuitChoice::Now,
        MessageDialogResult::Custom(label) if label == texts.quit_when_finished => {
            QuitChoice::WhenFinished
        }
        MessageDialogResult::Custom(label) if label == texts.stop_and_quit => QuitChoice::Now,
        _ => QuitChoice::Cancel,
    }
}

pub fn active_jobs(jobs: &[JobStatus]) -> usize {
    jobs.iter().filter(|job| !job.state.is_terminal()).count()
}

/// Tray texts in the interface language (`pt*` → Portuguese, else English).
pub struct Texts {
    pub show: &'static str,
    pub quit: &'static str,
    pub quit_when_finished: &'static str,
    pub stop_and_quit: &'static str,
    pub cancel: &'static str,
    portuguese: bool,
}

impl Texts {
    pub fn for_locale(locale: &str) -> Self {
        if locale.to_ascii_lowercase().starts_with("pt") {
            Self {
                show: "Abrir AnimeSubs",
                quit: "Sair",
                quit_when_finished: "Sair ao terminar",
                stop_and_quit: "Parar e sair",
                cancel: "Cancelar",
                portuguese: true,
            }
        } else {
            Self {
                show: "Show AnimeSubs",
                quit: "Quit",
                quit_when_finished: "Quit when finished",
                stop_and_quit: "Stop jobs and quit",
                cancel: "Cancel",
                portuguese: false,
            }
        }
    }

    /// e.g. "2 jobs running · 40/120 lines" (only running jobs count lines).
    pub fn status_line(&self, jobs: &[JobStatus]) -> String {
        let active = active_jobs(jobs);
        let (done, total) = jobs
            .iter()
            .filter(|job| job.state == JobState::Running)
            .filter_map(|job| job.progress)
            .fold((0, 0), |(d, t), p| (d + p.done, t + p.total));
        let head = match (self.portuguese, active) {
            (false, 0) => "No jobs running".to_string(),
            (false, 1) => "1 job running".to_string(),
            (false, n) => format!("{n} jobs running"),
            (true, 0) => "Nenhum trabalho em andamento".to_string(),
            (true, 1) => "1 trabalho em andamento".to_string(),
            (true, n) => format!("{n} trabalhos em andamento"),
        };
        if total == 0 {
            return head;
        }
        let unit = if self.portuguese { "linhas" } else { "lines" };
        format!("{head} · {done}/{total} {unit}")
    }

    pub fn confirm_quit(&self, active: usize) -> String {
        if self.portuguese {
            format!(
                "{active} trabalho(s) de tradução ainda em andamento. Sair quando \
                 terminarem, ou parar agora?"
            )
        } else {
            format!("{active} translation job(s) still running. Quit when they finish, or stop them now?")
        }
    }
}

fn texts(app: &AppHandle) -> Texts {
    Texts::for_locale(&app.state::<LocalServerState>().locale())
}

/// Jobs of the owned server; `None` if we own none or it does not answer.
fn owned_jobs_blocking(app: &AppHandle) -> Option<(Client, Vec<JobStatus>)> {
    let client = app.state::<LocalServerState>().owned_client()?;
    let jobs = tauri::async_runtime::block_on(async {
        tokio::time::timeout(Duration::from_secs(1), client.jobs()).await
    });
    match jobs {
        Ok(Ok(jobs)) => Some((client, jobs)),
        _ => None,
    }
}

/// Window handler: hide instead of closing while owned jobs are active.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let app = window.app_handle();
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            let jobs = owned_jobs_blocking(app);
            let owns = jobs.is_some();
            let active = jobs.as_ref().map(|(_, jobs)| active_jobs(jobs));
            if close_decision(owns, active) == CloseDecision::KeepRunning {
                api.prevent_close();
                put_away(window);
                enter(app);
            }
        }
        // Restored from the taskbar (no-tray fallback): the user is back, so
        // the deferred quit is off until the next close.
        WindowEvent::Focused(true)
            if app.state::<BackgroundState>().active.load(Ordering::SeqCst) =>
        {
            leave(app);
        }
        _ => {}
    }
}

/// Hide to the tray, or minimize when this desktop cannot show one.
fn put_away(window: &Window) {
    if tray_supported() {
        let _ = window.hide();
    } else {
        let _ = window.minimize();
    }
}

/// Whether a tray icon can be shown. On Linux the tray loads
/// libayatana-appindicator3 (or libappindicator3) at runtime and panics when
/// neither is installed, so look for the library before creating it.
pub fn tray_supported() -> bool {
    #[cfg(target_os = "linux")]
    {
        const NAMES: [&str; 2] = ["libayatana-appindicator3.so.1", "libappindicator3.so.1"];
        let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("LD_LIBRARY_PATH")
            .map(|value| std::env::split_paths(&value).collect())
            .unwrap_or_default();
        if let Some(appdir) = std::env::var_os("APPDIR") {
            let appdir = std::path::PathBuf::from(appdir);
            dirs.push(appdir.join("usr/lib"));
            dirs.push(appdir.join("usr/lib64"));
        }
        for dir in [
            "/usr/lib",
            "/usr/lib64",
            "/lib",
            "/lib64",
            "/usr/local/lib",
            "/usr/lib/x86_64-linux-gnu",
            "/usr/lib/aarch64-linux-gnu",
            "/lib/x86_64-linux-gnu",
            "/lib/aarch64-linux-gnu",
        ] {
            dirs.push(dir.into());
        }
        dirs.iter()
            .any(|dir| NAMES.iter().any(|name| dir.join(name).exists()))
    }
    #[cfg(not(target_os = "linux"))]
    {
        true
    }
}

/// Keep running in the background; quit when the owned server's jobs finish.
fn enter(app: &AppHandle) {
    let state = app.state::<BackgroundState>();
    state.active.store(true, Ordering::SeqCst);
    if tray_supported() {
        if let Err(error) = show_tray(app) {
            eprintln!("[background] tray unavailable: {error}");
        }
    }
    if !state.monitor_running.swap(true, Ordering::SeqCst) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { monitor(app).await });
    }
}

fn leave(app: &AppHandle) {
    app.state::<BackgroundState>()
        .active
        .store(false, Ordering::SeqCst);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_visible(false);
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn show_tray(app: &AppHandle) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        return tray.set_visible(true);
    }
    let texts = texts(app);
    let status = MenuItem::with_id(
        app,
        STATUS_ITEM,
        texts.status_line(&[]),
        false,
        None::<&str>,
    )?;
    let show = MenuItem::with_id(app, SHOW_ITEM, texts.show, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ITEM, texts.quit, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&status, &separator, &show, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip("AnimeSubs")
        .show_menu_on_left_click(true)
        .on_menu_event(on_menu_event);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    if let Ok(mut slot) = app.state::<BackgroundState>().status_item.lock() {
        *slot = Some(status);
    }
    Ok(())
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        SHOW_ITEM => leave(app),
        QUIT_ITEM => request_quit(app),
        _ => {}
    }
}

fn request_quit(app: &AppHandle) {
    let active = owned_jobs_blocking(app).map_or(0, |(_, jobs)| active_jobs(&jobs));
    if active == 0 {
        app.exit(0);
        return;
    }
    let texts = texts(app);
    let handle = app.clone();
    app.dialog()
        .message(texts.confirm_quit(active))
        .title("AnimeSubs")
        .buttons(MessageDialogButtons::YesNoCancelCustom(
            texts.quit_when_finished.to_string(),
            texts.stop_and_quit.to_string(),
            texts.cancel.to_string(),
        ))
        .show_with_result(move |result| match quit_choice(&result, &texts) {
            QuitChoice::WhenFinished => {
                if let Some(window) = handle.get_webview_window("main") {
                    let _ = window.hide();
                }
                enter(&handle);
            }
            // Exit stops the owned server (RunEvent::Exit), which drops its jobs.
            QuitChoice::Now => handle.exit(0),
            QuitChoice::Cancel => {}
        });
}

/// While in background mode: update the tray; quit once no job is active.
async fn monitor(app: AppHandle) {
    let state = app.state::<BackgroundState>();
    loop {
        if !state.active.load(Ordering::SeqCst) {
            break;
        }
        let client = app.state::<LocalServerState>().owned_client();
        let jobs = match client {
            Some(client) => tokio::time::timeout(Duration::from_secs(2), client.jobs())
                .await
                .ok()
                .and_then(Result::ok),
            None => None,
        };
        match jobs {
            Some(jobs) if active_jobs(&jobs) > 0 => {
                let line = texts(&app).status_line(&jobs);
                if let Ok(slot) = state.status_item.lock() {
                    if let Some(item) = slot.as_ref() {
                        let _ = item.set_text(&line);
                    }
                }
                if let Some(tray) = app.tray_by_id(TRAY_ID) {
                    let _ = tray.set_tooltip(Some(format!("AnimeSubs — {line}")));
                }
            }
            // Finished, or the server is gone: the deferred close happens now.
            _ => {
                app.exit(0);
                break;
            }
        }
        tokio::time::sleep(POLL).await;
    }
    state.monitor_running.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use animesubs_api::Progress;

    fn job(state: JobState, progress: Option<(u64, u64)>) -> JobStatus {
        JobStatus {
            id: "job-1".into(),
            kind: "translate_subtitle".into(),
            state,
            phase: "translating".into(),
            progress: progress.map(|(done, total)| Progress { done, total }),
            input: "/m/ep01.ass".into(),
            output: None,
            target_language: "pt-BR".into(),
            created_at_ms: 0,
            started_at_ms: None,
            finished_at_ms: None,
            error: None,
        }
    }

    #[test]
    fn closing_keeps_running_only_for_an_owned_server_with_active_jobs() {
        assert_eq!(close_decision(true, Some(2)), CloseDecision::KeepRunning);
        assert_eq!(close_decision(true, Some(0)), CloseDecision::Close);
        assert_eq!(close_decision(true, None), CloseDecision::Close);
        assert_eq!(close_decision(false, Some(3)), CloseDecision::Close);
    }

    #[test]
    fn counts_queued_and_running_jobs() {
        let jobs = [
            job(JobState::Queued, None),
            job(JobState::Running, Some((1, 2))),
            job(JobState::Succeeded, Some((2, 2))),
            job(JobState::Failed, None),
        ];
        assert_eq!(active_jobs(&jobs), 2);
    }

    #[test]
    fn status_line_sums_running_progress_in_both_languages() {
        let jobs = [
            job(JobState::Running, Some((40, 100))),
            job(JobState::Queued, None),
            job(JobState::Succeeded, Some((20, 20))),
        ];
        assert_eq!(
            Texts::for_locale("en").status_line(&jobs),
            "2 jobs running · 40/100 lines"
        );
        assert_eq!(
            Texts::for_locale("pt-BR").status_line(&jobs),
            "2 trabalhos em andamento · 40/100 linhas"
        );
        assert_eq!(
            Texts::for_locale("en").status_line(&[job(JobState::Queued, None)]),
            "1 job running"
        );
    }

    #[test]
    fn dialog_answers_map_to_quit_choices() {
        let texts = Texts::for_locale("en");
        let custom = |label: &str| MessageDialogResult::Custom(label.to_string());
        assert_eq!(
            quit_choice(&MessageDialogResult::Yes, &texts),
            QuitChoice::WhenFinished
        );
        assert_eq!(
            quit_choice(&MessageDialogResult::No, &texts),
            QuitChoice::Now
        );
        assert_eq!(
            quit_choice(&custom("Quit when finished"), &texts),
            QuitChoice::WhenFinished
        );
        assert_eq!(
            quit_choice(&custom("Stop jobs and quit"), &texts),
            QuitChoice::Now
        );
        assert_eq!(quit_choice(&custom("Cancel"), &texts), QuitChoice::Cancel);
        assert_eq!(
            quit_choice(&MessageDialogResult::Cancel, &texts),
            QuitChoice::Cancel
        );
    }
}
