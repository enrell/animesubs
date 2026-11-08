//! Job progress without changing the engine: a [`Translator`] decorator counts
//! the cues each successful provider response returns.
//!
//! The count can run ahead when the engine rejects a response and retries the
//! chunk, so readers clamp it to the total. Native progress events from the
//! engine will replace this (see `docs/ARCHITECTURE.md`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use animesubs_api::Progress;
use async_trait::async_trait;
use psyche_subtitle_toolkit::{Result, TranslationLimits, TranslationRequest, Translator};

/// Live state of a running job, shared between the worker and status readers.
#[derive(Debug)]
pub struct JobProgress {
    phase: Mutex<String>,
    done: AtomicU64,
    total: AtomicU64,
}

impl JobProgress {
    pub fn new(phase: &str) -> Self {
        Self {
            phase: Mutex::new(phase.to_string()),
            done: AtomicU64::new(0),
            total: AtomicU64::new(0),
        }
    }

    pub fn set_phase(&self, phase: &str) {
        if let Ok(mut current) = self.phase.lock() {
            phase.clone_into(&mut current);
        }
    }

    pub fn phase(&self) -> String {
        self.phase.lock().map(|p| p.clone()).unwrap_or_default()
    }

    pub fn set_total(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
    }

    pub fn add_done(&self, cues: u64) {
        self.done.fetch_add(cues, Ordering::Relaxed);
    }

    pub fn complete(&self) {
        self.done
            .store(self.total.load(Ordering::Relaxed), Ordering::Relaxed);
    }

    /// `None` until the total is known.
    pub fn snapshot(&self) -> Option<Progress> {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return None;
        }
        let done = self.done.load(Ordering::Relaxed).min(total);
        Some(Progress { done, total })
    }
}

pub struct CountingTranslator {
    inner: Arc<dyn Translator>,
    progress: Arc<JobProgress>,
}

impl CountingTranslator {
    pub fn wrap(inner: Arc<dyn Translator>, progress: Arc<JobProgress>) -> Arc<dyn Translator> {
        Arc::new(Self { inner, progress })
    }
}

#[async_trait]
impl Translator for CountingTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        let output = self.inner.translate(request).await?;
        self.progress.add_done(numbered_lines(&output));
        Ok(output)
    }

    // Delegated so resume manifests and chunk sizes match the wrapped provider.
    fn identifier(&self) -> String {
        self.inner.identifier()
    }

    fn limits(&self) -> TranslationLimits {
        self.inner.limits()
    }
}

/// Lines in the engine's `<N> text` exchange format.
fn numbered_lines(text: &str) -> u64 {
    text.lines()
        .filter(|line| {
            let line = line.trim_start();
            line.strip_prefix('<')
                .and_then(|rest| rest.split_once('>'))
                .is_some_and(|(id, _)| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        })
        .count() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Echo;

    #[async_trait]
    impl Translator for Echo {
        async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
            Ok(request.source_text.to_string())
        }

        fn identifier(&self) -> String {
            "echo".into()
        }
    }

    #[test]
    fn counts_numbered_lines_only() {
        assert_eq!(numbered_lines("<1> a\n<2> b\ncontinued\n<x> no\n<> no"), 2);
    }

    #[tokio::test]
    async fn decorator_counts_and_delegates() {
        let progress = Arc::new(JobProgress::new("translating"));
        progress.set_total(3);
        let translator = CountingTranslator::wrap(Arc::new(Echo), progress.clone());
        assert_eq!(translator.identifier(), "echo");
        translator
            .translate(TranslationRequest {
                source_text: "<1> a\n<2> b",
                target_language: "pt-BR",
            })
            .await
            .unwrap();
        assert_eq!(progress.snapshot(), Some(Progress { done: 2, total: 3 }));
        progress.add_done(5);
        assert_eq!(progress.snapshot(), Some(Progress { done: 3, total: 3 }));
    }

    #[test]
    fn no_progress_until_total_is_known() {
        let progress = JobProgress::new("parsing");
        progress.add_done(1);
        assert_eq!(progress.snapshot(), None);
        assert_eq!(progress.phase(), "parsing");
    }
}
