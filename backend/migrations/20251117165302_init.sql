-- TODO enum or text with check constraint
-- CREATE TYPE pr_state AS ENUM (
--     'open',
--     'closed',
--     'merged',
--     'S-waiting-on-review',
--     'S-waiting-on-bors',
--     'S-waiting-on-author'
--     );

-- TODO enum or text with check constraint
-- CREATE TYPE team_kind AS ENUM (
--     'team',
--     'working-group',
--     'project-group',
--     'marker-team'
--     );

CREATE TABLE teams
(
    team       TEXT PRIMARY KEY,
    subteam_of TEXT,
    kind       TEXT
);

CREATE TABLE contributors_teams
(
    github_id BIGINT NOT NULL,
    team      TEXT   NOT NULL REFERENCES teams (team),
    PRIMARY KEY (github_id, team)
);

CREATE TABLE contributors
(
    github_id   BIGINT PRIMARY KEY,
    github_name TEXT NOT NULL,
    name        TEXT
);

CREATE TABLE pull_requests
(
    pr             BIGINT PRIMARY KEY,
    contributor_id BIGINT NOT NULL REFERENCES contributors (github_id),
    current_state  TEXT   NOT NULL,
    merge_sha      TEXT,
    timestamp      TIMESTAMP
);


CREATE TABLE issues_state_history
(
    id             SERIAL PRIMARY KEY,
    issue          BIGINT,
    contributor_id BIGINT    NOT NULL REFERENCES contributors (github_id),
    timestamp      TIMESTAMP NOT NULL,
    label          TEXT,
    label_event    TEXT CHECK ( label_event IN ('added', 'removed'))
);

CREATE TABLE pr_state_history
(
    id        SERIAL PRIMARY KEY,
    pr        BIGINT    NOT NULL REFERENCES pull_requests (pr),
    state     TEXT      NOT NULL,
    timestamp TIMESTAMP NOT NULL,
    merge_sha TEXT
--     CHECK (
--         (state = 'merged' AND merge_sha IS NOT NULL) OR
--         (state <> 'merged' AND merge_sha IS NULL)
--         )
);


CREATE TABLE file_activity
(
    id             SERIAL PRIMARY KEY,
    pr             BIGINT    NOT NULL, -- REFERENCES pull_requests (pr),
    file_path      TEXT      NOT NULL,
    contributor_id BIGINT    NOT NULL,
    activity_type  TEXT,
    timestamp      TIMESTAMP NOT NULL
);