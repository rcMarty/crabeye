-- Add migration script here
-- migrations/20240301000001_add_indexes.sql
CREATE INDEX IF NOT EXISTS idx_pr_event_log_pr ON pr_event_log(pr);
CREATE INDEX IF NOT EXISTS idx_pr_event_log_timestamp ON pr_event_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_file_activity_pr ON file_activity(pr);
CREATE INDEX IF NOT EXISTS idx_file_activity_file_path ON file_activity(file_path);