-- Add migration script here

CREATE TABLE IF NOT EXISTS pr_event_log
(
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    pr        INTEGER                                         NOT NULL,
    state     TEXT check ( state IN ('open',
                                     'closed',
                                     'merged',
                                     'S-waiting-on-review',
                                     'S-waiting-on-bors',
                                     'S-waiting-on-author') ) NOT NULL, -- "open", "closed", "merged", "S-waiting-on-review", "S-waiting-on-bors", "S-waiting-on-author"
    timestamp DATETIME                                        NOT NULL, -- Event time (opened/closed/merged)
    merge_sha TEXT,                                                     -- Only for "merged"
    author_id INTEGER                                         NOT NULL  -- GitHub user ID
);

CREATE TABLE IF NOT EXISTS file_activity
(
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    pr            INTEGER  NOT NULL,
    file_path     TEXT     NOT NULL,
    user_login    TEXT     NOT NULL,
    activity_type TEXT,
    timestamp     DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS team_members
(
    github_id   TEXT NOT NULL,
    github_name TEXT NOT NULL,
    name        TEXT NOT NULL,
    team        TEXT,
    subteam     TEXT,
    kind        TEXT CHECK ( kind IN ('team',
                                      'working-group',
                                      'project-group',
                                      'marker-team') ), -- team, working-grouup, project-group, marker-team
    
    PRIMARY KEY (github_id)
);