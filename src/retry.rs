use std::time::Duration;

use crate::error::{Result, SubtitleToolkitError};

/// Retry an async operation with exponential backoff.
///
/// Retries up to `max_retries` times on HTTP and translation errors.
/// Uses exponential backoff: 1s, 2s, 4s, etc.
/// Does not retry on I/O, JSON, ASS parse, or other non-transient errors.
pub async fn retry_async<F, Fut, T>(max_retries: u32, mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut last_err = None;

    for attempt in 0..=max_retries {
        match op().await {
            Ok(val) => return Ok(val),
            Err(e) if is_retryable(&e) && attempt < max_retries => {
                let delay = Duration::from_secs(1 << attempt); // 1s, 2s, 4s
                eprintln!(
                    "[retry] attempt {}/{} failed: {}. Retrying in {}s...",
                    attempt + 1,
                    max_retries + 1,
                    e,
                    delay.as_secs(),
                );
                tokio::time::sleep(delay).await;
                last_err = Some(e);
            }
            Err(e) => return Err(e),
        }
    }

    // Unreachable in practice, but satisfies the compiler.
    Err(last_err.unwrap_or_else(|| SubtitleToolkitError::Translation {
        provider: "retry",
        message: "all retry attempts failed with no error".into(),
    }))
}

/// Returns true for errors that are likely transient and worth retrying.
fn is_retryable(err: &SubtitleToolkitError) -> bool {
    matches!(
        err,
        SubtitleToolkitError::Http(_)
            | SubtitleToolkitError::Translation { .. }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let result = retry_async(3, || async { Ok::<_, SubtitleToolkitError>(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn retries_on_http_error() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_async(2, || {
            let attempts = attempts_clone.clone();
            async move {
                let count = attempts.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(SubtitleToolkitError::Translation {
                        provider: "test",
                        message: "transient error".into(),
                    })
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn does_not_retry_on_io_error() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result: std::result::Result<(), _> = retry_async(3, || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err(SubtitleToolkitError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "file not found",
                )))
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1); // no retry
    }

    #[tokio::test]
    async fn gives_up_after_max_retries() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result: std::result::Result<(), _> = retry_async(2, || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err(SubtitleToolkitError::Translation {
                    provider: "test",
                    message: "always fails".into(),
                })
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 3); // 1 initial + 2 retries
    }
}
