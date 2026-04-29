-- FILES: "Show changed files for this PR"
CREATE INDEX idx_pr_file_activity_lookup
    ON file_activity (repository, issue);

-- FILE HISTORY: "Show history of 'src/main.rs'"
CREATE INDEX idx_pr_file_activity_path
    ON file_activity (repository, file_path varchar_pattern_ops, timestamp DESC);

-- CONTRIBUTOR: "Show files touched by this user"
CREATE INDEX idx_pr_file_activity_contributor
    ON file_activity (contributor_id);

-- Contributor-based file activity queries
-- INCLUDE (file_path, issue) enables Index-Only Scans.
CREATE INDEX idx_file_activity_repo_contrib_ts
    ON file_activity (repository, contributor_id, timestamp DESC)
    INCLUDE (file_path, issue);

