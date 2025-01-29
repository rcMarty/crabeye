use std::collections::HashSet;
use std::fmt::format;
use anyhow::Context;
use git2::{Oid, Repository};

pub struct Repo {
    repository: Repository,
}

impl Repo {
    pub fn init(path: &str) -> Self {
        let repository = Repository::open(path).context(format!("failed to open repository\nPath: {}", path)).expect("failed to open repository");
        log::debug!("Repository opened: {:?}", repository.path());
        Self { repository }
    }

    pub fn update(&mut self) -> anyhow::Result<()> {
        let mut remote = self.repository.find_remote("origin").context("failed to find remote 'origin'")?;
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

        let diff = self.repository
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