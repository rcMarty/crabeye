use indicatif::MultiProgress;

lazy_static::lazy_static! {
    static ref MULTI_PROGRESS_BAR: MultiProgress = MultiProgress::new();
}

pub(crate) fn multi_progress_bar() -> &'static MultiProgress {
    &MULTI_PROGRESS_BAR
}

pub(crate) async fn with_progress_bar_async<F, T>(
    total: usize,
    message: Option<String>,
    process: F,
) -> anyhow::Result<T>
where
    F: AsyncFnOnce(Option<&indicatif::ProgressBar>, &indicatif::MultiProgress) -> anyhow::Result<T>,
{
    let multi = multi_progress_bar().clone();

    match message {
        None => match process(None, &multi).await {
            Ok(result) => anyhow::Ok(result),
            Err(e) => Err(e),
        },
        Some(mess) => {
            let bar = multi.add(indicatif::ProgressBar::new(total as u64));
            bar.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")?
                    .progress_chars("##-"),
            );
            bar.set_message(mess);
            match process(Some(&bar), &multi).await {
                Ok(result) => {
                    bar.finish_with_message("Done");
                    multi.remove(&bar);
                    anyhow::Ok(result)
                }
                Err(e) => {
                    bar.finish_with_message(format!("Error: {e}"));
                    multi.remove(&bar);
                    Err(e)
                }
            }
        }
    }
}


