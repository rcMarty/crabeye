-- DASHBOARD: "Show me Open ISSUES for this repo, sorted by date"
CREATE INDEX idx_issues_dashboard_state
    ON issues (repository, current_state, edited_at DESC)
    WHERE is_pr = false;

-- DASHBOARD: "Show me Open PRS for this repo, sorted by date"
CREATE INDEX idx_prs_dashboard_state
    ON issues (repository, current_state, edited_at DESC)
    WHERE is_pr = true;

-- LOOKUP: "Find specific ISSUE by ID"
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

-- Oldest-PR / date-range queries
CREATE INDEX idx_issues_created_at
    ON issues (repository, created_at)
    WHERE is_pr = true;

