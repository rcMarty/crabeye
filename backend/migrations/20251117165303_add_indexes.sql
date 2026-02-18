-- DASHBOARD: "Show me Open ISSUES for this repo, sorted by date"
CREATE INDEX idx_issues_dashboard_state
    ON issues (repository, current_state, timestamp DESC)
    WHERE is_pr = false;

-- DASHBOARD: "Show me Open PRS for this repo, sorted by date"
CREATE INDEX idx_prs_dashboard_state
    ON issues (repository, current_state, timestamp DESC)
    WHERE is_pr = true;

-- LOOKUP: "Find specific ISSUE by ID" (Optimized for filtering)
CREATE INDEX idx_issues_lookup
    ON issues (repository, issue)
    WHERE is_pr = false;

-- LOOKUP: "Find specific PR by ID"
CREATE INDEX idx_prs_lookup
    ON issues (repository, issue)
    WHERE is_pr = true;


-- "Show me all ISSUES created by this user"
CREATE INDEX idx_issues_contributor_only
    ON issues (contributor_id)
    WHERE is_pr = false;

-- "Show me all PRS created by this user"
CREATE INDEX idx_prs_contributor_only
    ON issues (contributor_id)
    WHERE is_pr = true;


-- HISTORY: "Show timeline for a specific ISSUE"
CREATE INDEX idx_issues_history_lookup
    ON issue_event_history (repository, issue, timestamp DESC)
    WHERE is_pr = false;

-- HISTORY: "Show timeline for a specific PR"
CREATE INDEX idx_prs_history_lookup
    ON issue_event_history (repository, issue, timestamp DESC)
    WHERE is_pr = true;


-- LOOKUP: "Show labels for a specific ISSUE"
CREATE INDEX idx_issue_labels_lookup
    ON issue_labels_history (repository, issue)
    WHERE is_pr = false;

-- LOOKUP: "Show labels for a specific PR"
CREATE INDEX idx_pr_labels_lookup
    ON issue_labels_history (repository, issue)
    WHERE is_pr = true;

-- SEARCH: "Find all ISSUES with label 'bug'"
CREATE INDEX idx_issue_labels_name
    ON issue_labels_history (label)
    WHERE is_pr = false;

-- SEARCH: "Find all PRS with label 'bug'" (Rare, but useful for 'do not merge' etc.)
CREATE INDEX idx_pr_labels_name
    ON issue_labels_history (label)
    WHERE is_pr = true;

-- UPSERT OPTIMIZATION (Finding the latest label state)
-- Split into two smaller indexes for faster writes/reads
CREATE INDEX idx_issue_labels_upsert_latest
    ON issue_labels_history (repository, issue, label, timestamp DESC, action)
    WHERE is_pr = false;

CREATE INDEX idx_pr_labels_upsert_latest
    ON issue_labels_history (repository, issue, label, timestamp DESC, action)
    WHERE is_pr = true;


-- FILES: "Show changed files for this PR"
CREATE INDEX idx_pr_file_activity_lookup
    ON file_activity (repository, issue);

-- FILE HISTORY: "Show history of 'src/main.rs'" (Likely only relevant for PRs)
CREATE INDEX idx_pr_file_activity_path
    ON file_activity (repository, file_path varchar_pattern_ops, timestamp DESC);

-- CONTRIBUTOR: "Show files touched by this user"
CREATE INDEX idx_pr_file_activity_contributor
    ON file_activity (contributor_id);


-- USER LOOKUP: Users exist independently of issues/PRs
CREATE INDEX idx_contributors_name ON contributors (github_name);

-- TEAM MEMBERSHIP: Teams exist independently
CREATE INDEX idx_contributors_teams_team ON contributors_teams (team);


ALTER TABLE issue_event_history
    ADD CONSTRAINT uq_issue_event_history_conflict
        UNIQUE (repository, issue, timestamp);

ALTER TABLE issue_labels_history
    ADD CONSTRAINT uq_issue_labels_history_conflict
        UNIQUE (repository, issue, label, timestamp);

ALTER TABLE issues
    ADD CONSTRAINT uq_issues_conflict
        UNIQUE (repository, issue);
