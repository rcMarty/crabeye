CREATE INDEX idx_prs_transitions ON issues (repository, issue, timestamp)
    WHERE is_pr = true;

CREATE INDEX idx_issue_event_history_repo_evt_pr_only
    ON issue_event_history (repository, event)
    WHERE is_pr = true;