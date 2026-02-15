#![allow(unused)]
use anyhow::Context;
use git2::{FetchOptions, Oid, Repository};
use std::collections::HashSet;
use std::fmt::format;
use std::path::Path;

pub struct Repo {
    pub repository_identifier: String,
    repository: Repository,
}

impl Repo {
    pub fn init(repository_identifier: String, repository_url: &str, path: &Path) -> anyhow::Result<Self> {
        if path.exists() {
            match Repository::open(path)
                .map(|repository| Self { repository_identifier: repository_identifier.clone(), repository })
                .with_context(|| format!("Failed to open repository {:?}", path))
            {
                Ok(mut rep) => {
                    log::info!("Repository opened: {:?}", path);
                    rep.update()?;
                    log::info!("Repository updated: {:?}", path);
                    Ok(rep)
                }
                Err(e) => {
                    log::warn!("Failed to open repository: {:?}", e);
                    log::info!("Trying to clone repository from {}", repository_url);
                    let result = Self::clone_repository(repository_identifier, repository_url, path);
                    log::info!("Repository cloned. Ok?:{:?}", result.is_ok());
                    result
                }
            }
        } else {
            log::info!(
                "Trying to clone repository from {repository_url} to {:?}",
                path
            );
            Repo::clone_repository(repository_identifier, repository_url, path)
        }
    }

    fn clone_repository(repository_identifier: String, url: &str, path: &Path) -> anyhow::Result<Self> {
        let mut fetch_options = FetchOptions::new();
        fetch_options.download_tags(git2::AutotagOption::All);

        let mut builder = git2::build::RepoBuilder::new();
        let mut checkout_builder = git2::build::CheckoutBuilder::new();
        builder
            .fetch_options(fetch_options)
            .with_checkout(checkout_builder);

        let repository = builder
            .clone(url, path)
            .with_context(|| format!("Failed to clone repository from {} to {:?}", url, path))?;

        Ok(Self { repository_identifier, repository })
    }

    pub fn update(&self) -> anyhow::Result<()> {
        let mut remote = self
            .repository
            .find_remote("origin")
            .context("failed to find remote 'origin'")?;
        log::info!("fetching remote 'origin'");
        remote.fetch(&["master"], None, None)?;
        log::debug!("Repository updated");
        Ok(())
    }

    pub fn modified_files(&self, commit_id: Oid) -> anyhow::Result<Option<HashSet<String>>> {
        let Ok(merge_commit) = self.repository.find_commit(commit_id) else {
            log::warn!("Cannot find commit {}", commit_id);
            return Ok(None);
        };

        let Some(first_parent) = merge_commit.parents().next() else {
            log::warn!("Cannot find parent commit for {}", commit_id);
            return Ok(None);
        };

        let diff = self
            .repository
            .diff_tree_to_tree(
                merge_commit.tree().ok().as_ref(),
                first_parent.tree().ok().as_ref(),
                None,
            )
            .context(format!("Cannot diff tree {}", commit_id))?;

        let mut files = HashSet::with_capacity(diff.deltas().len());
        for delta in diff.deltas() {
            if let Some(old_file) = delta.old_file().path().and_then(|s| s.to_str()) {
                files.insert(old_file.to_string());
            }
            if let Some(new_file) = delta.new_file().path().and_then(|s| s.to_str()) {
                files.insert(new_file.to_string());
            }
        }
        Ok(Some(files))
    }
}
