-- HISTORY: "Show timeline for a specific ISSUE"
CREATE INDEX idx_issues_history_lookup
    ON issue_event_history (repository, issue, timestamp DESC)
    WHERE is_pr = false;

-- HISTORY: "Show timeline for a specific PR"
CREATE INDEX idx_prs_history_lookup
    ON issue_event_history (repository, issue, timestamp DESC)
    WHERE is_pr = true;

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

