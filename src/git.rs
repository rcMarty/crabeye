use git2::{Oid, Repository};

pub struct Repo {
    repository: Repository,
}

impl Repo {
    pub fn init(path: &str) -> Self {
        let repository = Repository::open(path).expect("failed to open repository");
        Self { repository }
    }

    pub fn get_diff_files(&self, commit_id: Oid) -> anyhow::Result<Vec<String>> {
        let commit = self.repository.find_commit(commit_id).expect("failed to find commit");
        let parent = commit.parent(0).expect("failed to find parent");
        let diff = self.repository
            .diff_tree_to_tree(
                Some(&parent.tree().expect("failed to find parent tree")),
                Some(&commit.tree().expect("failed to find commit tree")),
                None)
            .expect("failed to get diff");
        for delta in diff.deltas() {
            println!("delta: {:?}", delta.new_file().path().expect("failed to get path"));
        }
        let files: Vec<String> = diff.deltas()
            .map(|delta| delta.new_file()
                .path()
                .expect("failed to get path")
                .to_str().unwrap()
                .to_owned()
                .to_string())
            .collect();
        Ok(files)
    }
}