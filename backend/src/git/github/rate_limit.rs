use std::time::Duration;

/// Check if an octocrab error is a GitHub rate limit error (HTTP 429, 403 rate limit, secondary rate limit).
pub(crate) fn is_rate_limit_error(err: &octocrab::Error) -> bool {
    let s = format!("{:?}", err).to_lowercase();
    s.contains("rate limit")
        || s.contains("too many requests")
        || s.contains("secondary rate")
        || s.contains("abuse detection")
        || s.contains("429")
}

/// Retry an async operation with exponential backoff when a GitHub rate limit is hit.
/// - Initial wait: 30s
/// - Max wait between retries: 5 min
/// - Total max time: 10 min
/// Non-rate-limit errors are returned immediately without retry.
pub(crate) async fn retry_on_rate_limit<F, Fut, T>(
    operation: &str,
    f: F,
) -> Result<T, octocrab::Error>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, octocrab::Error>>,
{
    let backoff = backoff::ExponentialBackoffBuilder::new()
        .with_initial_interval(Duration::from_secs(30))
        .with_max_interval(Duration::from_secs(300))
        .with_max_elapsed_time(Some(Duration::from_secs(600)))
        .build();

    backoff::future::retry(backoff, || {
        let fut = f();
        async move {
            match fut.await {
                Ok(val) => Ok(val),
                Err(err) if is_rate_limit_error(&err) => {
                    log::warn!(
                        "GitHub rate limit hit during '{}', retrying with backoff: {}",
                        operation,
                        err
                    );
                    Err(backoff::Error::transient(err))
                }
                Err(err) => Err(backoff::Error::permanent(err)),
            }
        }
    })
    .await
}

