-- Optimized indexes for analytical SELECT queries

-- PR state-change events (closed/merged/reopened)
-- Supports DISTINCT ON (issue) ORDER BY issue, timestamp DESC
-- and LEAD() OVER (PARTITION BY issue ORDER BY timestamp)
-- INCLUDE (event) enables Index-Only Scans.
CREATE INDEX idx_pr_event_hist_state_changes
    ON issue_event_history (repository, issue, timestamp DESC)
    INCLUDE (event)
    WHERE is_pr = true AND event IN ('closed', 'merged', 'reopened');

-- Merged-only events for cumulative merge counts & MIN(timestamp)
CREATE INDEX idx_pr_event_hist_merged
    ON issue_event_history (repository, issue, timestamp)
    WHERE is_pr = true AND event = 'merged';

-- MAX(timestamp) per repository — Index Scan Backward + LIMIT 1
CREATE INDEX idx_event_hist_repo_ts
    ON issue_event_history (repository, timestamp DESC);

-- PR label state queries (S-waiting-on-review, etc.)
-- Supports DISTINCT ON (issue), LEAD window, label IN filters
-- INCLUDE (action) enables Index-Only Scans.
CREATE INDEX idx_pr_labels_repo_label_issue_ts
    ON issue_labels_history (repository, label, issue, timestamp DESC)
    INCLUDE (action)
    WHERE is_pr = true;

-- Contributor-based file activity queries
-- INCLUDE (file_path, issue) enables Index-Only Scans.
CREATE INDEX idx_file_activity_repo_contrib_ts
    ON file_activity (repository, contributor_id, timestamp DESC)
    INCLUDE (file_path, issue);
