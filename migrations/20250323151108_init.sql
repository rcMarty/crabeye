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

CREATE TABLE team_members
(
    id          SERIAL PRIMARY KEY,
    github_id   BIGINT NOT NULL,
    github_name TEXT   NOT NULL,
    name        TEXT,
    team        TEXT,
    subteam_of  TEXT,
    kind        TEXT
);

CREATE TABLE pull_requests
(
    pr            BIGINT PRIMARY KEY,
    author_id     BIGINT NOT NULL,
    current_state TEXT   NOT NULL,
    merge_sha     TEXT,
    timestamp     TIMESTAMP
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
    id            SERIAL PRIMARY KEY,
    pr            BIGINT    NOT NULL, --REFERENCES pull_requests (pr),
    file_path     TEXT      NOT NULL,
    user_login    BIGINT    NOT NULL,
    activity_type TEXT,
    timestamp     TIMESTAMP NOT NULL
);