-- HISTORY TABLES: To quickly find history for a specific issue/PR
-- We include 'timestamp' to make the index "sorted" for free
CREATE INDEX idx_issues_history_lookup
    ON issue_state_history (repository, issue, timestamp DESC);

CREATE INDEX idx_pr_history_lookup
    ON pr_state_history (repository, pr, timestamp DESC);

-- LABELS: To quickly find labels belonging to an issue/PR
CREATE INDEX idx_issue_labels_lookup
    ON issue_labels_history (repository, issue);

CREATE INDEX idx_pr_labels_lookup
    ON pr_labels_history (repository, pr);

-- FILES: To list all files touched in a specific PR
CREATE INDEX idx_file_activity_lookup
    ON file_activity (repository, pr);

-- CONTRIBUTORS: To find all items created by a specific user
CREATE INDEX idx_issues_contributor ON issues (contributor_id);
CREATE INDEX idx_pr_contributor ON pull_requests (contributor_id);
CREATE INDEX idx_file_activity_contributor ON file_activity (contributor_id);


-- DASHBOARD: "Show me Open issues for this repo, sorted by date"
-- The PK handles 'repository', but this composite index handles the rest.
CREATE INDEX idx_issues_state_time
    ON issues (repository, current_state, timestamp DESC);

CREATE INDEX idx_pr_state_time
    ON pull_requests (repository, current_state, timestamp DESC);

-- LABEL SEARCH: "Find all issues with the label 'bug'"
-- Since we are searching across all repos, we index 'label' first.
CREATE INDEX idx_issue_labels_name ON issue_labels_history (label);
CREATE INDEX idx_pr_labels_name ON pr_labels_history (label);

-- USER LOOKUP: Find user by name (e.g., for autocomplete or URL lookup)
CREATE INDEX idx_contributors_name ON contributors (github_name);

-- TEAM MEMBERSHIP: Reverse lookup (Find all members of a team)
-- The PK covers (contributor, team), so we need the reverse:
CREATE INDEX idx_contributors_teams_team ON contributors_teams (team);

-- FILE HISTORY: "Show me history of 'src/main.rs' in this repo"
-- using varchar_pattern_ops optimizes for LIKE 'path/%' queries
CREATE INDEX idx_file_activity_path
    ON file_activity (repository, file_path varchar_pattern_ops, timestamp DESC);

CREATE INDEX idx_issues_state_history_upsert ON issue_labels_history (repository, issue, label, timestamp DESC, action, issue)