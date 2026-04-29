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

-- SEARCH: "Find all PRS with label 'bug'"
CREATE INDEX idx_pr_labels_name
    ON issue_labels_history (label)
    WHERE is_pr = true;

-- UPSERT OPTIMIZATION (Finding the latest label state) — split for faster writes/reads
CREATE INDEX idx_issue_labels_upsert_latest
    ON issue_labels_history (repository, issue, label, timestamp DESC, action)
    WHERE is_pr = false;

CREATE INDEX idx_pr_labels_upsert_latest
    ON issue_labels_history (repository, issue, label, timestamp DESC, action)
    WHERE is_pr = true;

-- PR label state queries (S-waiting-on-review, etc.)
-- Supports DISTINCT ON (issue), LEAD window, label IN filters
-- INCLUDE (action) enables Index-Only Scans.
CREATE INDEX idx_pr_labels_repo_label_issue_ts
    ON issue_labels_history (repository, label, issue, timestamp DESC)
    INCLUDE (action)
    WHERE is_pr = true;

