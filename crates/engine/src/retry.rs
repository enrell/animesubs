use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::error::{Result, SubtitleToolkitError};

const LONG_RETRY_BUDGET: Duration = Duration::from_secs(2 * 60 * 60);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

/// Retry an async operation with exponential backoff.
///
/// Transient transport/status failures and malformed model output use the supplied
/// retry count. HTTP 503 is special: it keeps retrying until the two-hour retry
/// budget is exhausted. The delay grows exponentially and then stays capped at
/// one minute. Honors numeric `Retry-After` values, also capped at one minute.
/// Permanent provider, parse, and I/O failures return immediately.
pub async fn retry_async<F, Fut, T>(max_retries: u32, mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    retry_async_with_budget(max_retries, LONG_RETRY_BUDGET, &mut op).await
}

async fn retry_async_with_budget<F, Fut, T>(
    max_retries: u32,
    long_retry_budget: Duration,
    mut op: F,
) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let started_at = Instant::now();
    let mut retry_count = 0u32;

    loop {
        match op().await {
            Ok(val) => return Ok(val),
            Err(error) if is_retryable(&error) => {
                let is_long_503 = is_long_retry_503(&error);
                if !is_long_503 && retry_count >= max_retries {
                    return Err(error);
                }

                let elapsed = started_at.elapsed();
                if is_long_503 && elapsed >= long_retry_budget {
                    return Err(error);
                }

                let delay = retry_delay(&error, retry_count);
                if is_long_503 && delay > long_retry_budget.saturating_sub(elapsed) {
                    return Err(error);
                }

                let retry_number = retry_count.saturating_add(1);
                let retry_limit = if is_long_503 {
                    format!("up to {}s total", long_retry_budget.as_secs())
                } else {
                    format!("{} retries", max_retries)
                };
                eprintln!(
                    "[retry] {} failed (retry {retry_number}, {retry_limit}). Retrying in {:.2}s...",
                    retry_label(&error),
                    delay.as_secs_f64(),
                );
                tokio::time::sleep(delay).await;
                retry_count = retry_count.saturating_add(1);
            }
            Err(error) => return Err(error),
        }
    }
}

/// Returns true for errors that are likely transient and worth retrying.
fn is_retryable(err: &SubtitleToolkitError) -> bool {
    match err {
        SubtitleToolkitError::Http(error) => {
            error.is_timeout() || error.is_connect() || error.is_request()
        }
        SubtitleToolkitError::Provider {
            retryable, status, ..
        } => *retryable || *status == Some(503),
        SubtitleToolkitError::InvalidTranslation { .. } => true,
        _ => false,
    }
}

fn is_long_retry_503(err: &SubtitleToolkitError) -> bool {
    matches!(
        err,
        SubtitleToolkitError::Provider {
            status: Some(503),
            ..
        }
    )
}

fn retry_label(err: &SubtitleToolkitError) -> String {
    match err {
        SubtitleToolkitError::Provider {
            provider,
            status: Some(status),
            ..
        } => format!("{provider} HTTP {status}"),
        SubtitleToolkitError::Provider { provider, .. } => format!("{provider} request"),
        SubtitleToolkitError::InvalidTranslation { .. } => "invalid translation".into(),
        SubtitleToolkitError::Http(error) if error.is_timeout() => "HTTP timeout".into(),
        SubtitleToolkitError::Http(_) => "HTTP transport error".into(),
        _ => "transient error".into(),
    }
}

fn retry_delay(err: &SubtitleToolkitError, attempt: u32) -> Duration {
    if let SubtitleToolkitError::Provider {
        retry_after_seconds: Some(seconds),
        ..
    } = err
    {
        return Duration::from_secs((*seconds).min(MAX_RETRY_DELAY.as_secs()));
    }

    let base_seconds = 1_u64
        .checked_shl(attempt.min(6))
        .unwrap_or(MAX_RETRY_DELAY.as_secs())
        .min(MAX_RETRY_DELAY.as_secs());
    let base = Duration::from_secs(base_seconds);
    let jitter_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| u64::from(duration.subsec_nanos()) % 251);
    (base + Duration::from_millis(jitter_millis)).min(MAX_RETRY_DELAY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

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
                    Err(SubtitleToolkitError::Provider {
                        provider: "test",
                        status: Some(503),
                        message: "transient error".into(),
                        retryable: true,
                        retry_after_seconds: None,
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
                Err(SubtitleToolkitError::Provider {
                    provider: "test",
                    status: Some(502),
                    message: "always fails".into(),
                    retryable: true,
                    retry_after_seconds: None,
                })
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 3); // 1 initial + 2 retries
    }

    #[tokio::test]
    async fn retries_503_beyond_the_normal_retry_count() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_async_with_budget(0, Duration::from_secs(2), || {
            let attempts = attempts_clone.clone();
            async move {
                if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(SubtitleToolkitError::Provider {
                        provider: "test",
                        status: Some(503),
                        message: "temporarily unavailable".into(),
                        retryable: false,
                        retry_after_seconds: Some(0),
                    })
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn stops_503_when_the_long_retry_budget_is_exhausted() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_async_with_budget(100, Duration::ZERO, || {
            let attempts = attempts_clone.clone();
            async move {
                attempts.fetch_add(1, Ordering::SeqCst);
                Err::<(), _>(SubtitleToolkitError::Provider {
                    provider: "test",
                    status: Some(503),
                    message: "still unavailable".into(),
                    retryable: true,
                    retry_after_seconds: None,
                })
            }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retries_on_invalid_translation() {
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();

        let result = retry_async(2, || {
            let attempts = attempts_clone.clone();
            async move {
                let count = attempts.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(SubtitleToolkitError::InvalidTranslation {
                        message: format!("missing id <{count}>"),
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

    #[test]
    fn permanent_provider_errors_are_not_retryable() {
        let error = SubtitleToolkitError::Provider {
            provider: "test",
            status: Some(401),
            message: "unauthorized".into(),
            retryable: false,
            retry_after_seconds: None,
        };
        assert!(!is_retryable(&error));
    }

    #[test]
    fn retry_after_header_overrides_backoff() {
        let error = SubtitleToolkitError::Provider {
            provider: "test",
            status: Some(429),
            message: "rate limited".into(),
            retryable: true,
            retry_after_seconds: Some(17),
        };
        assert_eq!(retry_delay(&error, 0), Duration::from_secs(17));
    }

    #[test]
    fn retry_after_and_backoff_are_capped_at_one_minute() {
        let error = SubtitleToolkitError::Provider {
            provider: "test",
            status: Some(503),
            message: "unavailable".into(),
            retryable: true,
            retry_after_seconds: Some(3_600),
        };
        assert_eq!(retry_delay(&error, 0), Duration::from_secs(60));
        assert!(
            retry_delay(
                &SubtitleToolkitError::InvalidTranslation {
                    message: "malformed".into(),
                },
                20
            ) <= Duration::from_secs(60)
        );
    }
}
