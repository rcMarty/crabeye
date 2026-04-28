use std::fmt;
use std::time::Duration;

/// Error returned when a GitHub API rate limit is hit and retries are exhausted.
///
/// Carries the original octocrab error and, when available, the UTC timestamp
/// at which the rate limit window resets (from the `x-ratelimit-reset` header
/// or error body).
#[derive(Debug)]
pub struct RateLimitExhausted {
    pub inner: octocrab::Error,
    /// UTC epoch seconds at which the limit resets, if GitHub told us.
    pub retry_after_epoch: Option<u64>,
}

impl fmt::Display for RateLimitExhausted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(reset) = self.retry_after_epoch {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let wait_secs = reset.saturating_sub(now);
            write!(
                f,
                "GitHub rate limit exceeded; retry possible in ~{} min {} s (resets at epoch {}): {}",
                wait_secs / 60,
                wait_secs % 60,
                reset,
                self.inner
            )
        } else {
            write!(
                f,
                "GitHub rate limit exceeded (reset time unknown): {}",
                self.inner
            )
        }
    }
}

impl std::error::Error for RateLimitExhausted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.inner)
    }
}

/// Check if an octocrab error is a GitHub rate limit error (HTTP 429, 403 rate limit, secondary rate limit).
pub(crate) fn is_rate_limit_error(err: &octocrab::Error) -> bool {
    let s = format!("{:?}", err).to_lowercase();
    s.contains("rate limit")
        || s.contains("too many requests")
        || s.contains("secondary rate")
        || s.contains("abuse detection")
        || s.contains("429")
}

/// Try to extract the `x-ratelimit-reset` epoch value from the error debug output.
/// GitHub embeds it in the response headers / error body.
fn parse_reset_epoch(err: &octocrab::Error) -> Option<u64> {
    let s = format!("{:?}", err);
    // Look for patterns like `x-ratelimit-reset: 1234567890` or `"reset": 1234567890`
    for marker in &["ratelimit-reset", "\"reset\""] {
        if let Some(pos) = s.to_lowercase().find(marker) {
            let after = &s[pos + marker.len()..];
            // skip non-digit chars (`:`, ` `, `"`, etc.)
            let digits: String = after
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(epoch) = digits.parse::<u64>() {
                // sanity: must be a realistic epoch (after 2020)
                if epoch > 1_577_836_800 {
                    return Some(epoch);
                }
            }
        }
    }
    None
}

/// Retry an async operation with exponential backoff when a GitHub rate limit is hit.
///
/// - Initial wait: **5 s**
/// - Max wait between retries: **30 s**
/// - Total max time: **2 min**
///
/// Non-rate-limit errors are returned immediately without retry.
///
/// When retries are exhausted due to a persistent rate limit, the function
/// returns a [`RateLimitExhausted`] error (wrapped in `anyhow`) that
/// includes the estimated reset time so the caller can decide what to do.
pub(crate) async fn retry_on_rate_limit<F, Fut, T>(
    operation: &str,
    f: F,
) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, octocrab::Error>>,
{
    let backoff = backoff::ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_secs(5))
        .with_max_interval(Duration::from_secs(60))
        .with_max_elapsed_time(Some(Duration::from_secs(5 * 60)))
        .build();

    // We need to capture the last rate-limit error to extract the reset epoch
    // after backoff gives up.
    let last_reset_epoch = std::sync::Mutex::new(None::<u64>);

    let result = backoff::future::retry(backoff, || {
        let fut = f();
        async {
            match fut.await {
                Ok(val) => Ok(val),
                Err(err) if is_rate_limit_error(&err) => {
                    let reset = parse_reset_epoch(&err);
                    *last_reset_epoch.lock().unwrap() = reset;
                    log::warn!(
                        "GitHub rate limit hit during '{}', retrying with backoff (reset epoch: {:?}): {}",
                        operation,
                        reset,
                        err
                    );
                    Err(backoff::Error::transient(err))
                }
                Err(err) => Err(backoff::Error::permanent(err)),
            }
        }
    })
        .await;

    match result {
        Ok(val) => Ok(val),
        Err(err) if is_rate_limit_error(&err) => {
            let reset = *last_reset_epoch.lock().unwrap();
            let rate_err = RateLimitExhausted {
                inner: err,
                retry_after_epoch: reset,
            };
            log::error!(
                "Rate limit retries exhausted for '{}': {}",
                operation,
                rate_err
            );
            Err(anyhow::Error::new(rate_err))
        }
        Err(err) => Err(anyhow::Error::new(err)),
    }
}
