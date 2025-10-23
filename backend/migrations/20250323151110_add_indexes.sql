-- Add migration script here
-- migrations/20240301000001_add_indexes.sql
-- For state history table (previously pr_event_log)
CREATE INDEX IF NOT EXISTS idx_pr_state_history_pr ON pr_state_history (pr);
CREATE INDEX IF NOT EXISTS idx_pr_state_history_timestamp ON pr_state_history (timestamp);

-- For file_activity (similar to original but with PostgreSQL optimizations)
CREATE INDEX IF NOT EXISTS idx_file_activity_pr ON file_activity (pr);
CREATE INDEX IF NOT EXISTS idx_file_activity_file_path ON file_activity (file_path varchar_pattern_ops);
CREATE INDEX IF NOT EXISTS idx_file_activity_user_login ON file_activity (user_login);

CREATE INDEX IF NOT EXISTS idx_pull_requests_author_id ON pull_requests (author_id);
