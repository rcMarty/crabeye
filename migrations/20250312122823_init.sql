-- Add migration script here

CREATE TABLE IF NOT EXISTS pr_event_log
(
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    pr        INTEGER  NOT NULL,
    state     TEXT     NOT NULL,
    timestamp DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS file_activity
(
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    pr            INTEGER  NOT NULL,
    file_path     TEXT     NOT NULL,
    user_login    TEXT     NOT NULL,
    activity_type TEXT CHECK (activity_type IN ('edit', 'review')),
    timestamp     DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS team_members
(
    team_slug  TEXT NOT NULL,
    user_login TEXT NOT NULL,
    PRIMARY KEY (team_slug, user_login)
);