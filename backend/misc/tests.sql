CREATE INDEX idx_prs_transitions ON issues (repository, issue, timestamp)
    WHERE is_pr = true;
