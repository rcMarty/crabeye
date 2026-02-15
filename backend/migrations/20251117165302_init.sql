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
    team           TEXT   NOT NULL REFERENCES teams (team),
    contributor_id BIGINT NOT NULL,

    PRIMARY KEY (team, contributor_id)
);

CREATE TABLE contributors
(
    github_id   BIGINT PRIMARY KEY,
    github_name TEXT NOT NULL,
    name        TEXT
);


CREATE TABLE issues
(
    repository     TEXT      NOT NULL,
    issue          BIGINT,

    contributor_id BIGINT    NOT NULL REFERENCES contributors (github_id),
    current_state  TEXT      NOT NULL,
    timestamp      TIMESTAMP NOT NULL,

    PRIMARY KEY (repository, issue)
);

CREATE TABLE issue_state_history
(
    id         SERIAL PRIMARY KEY,
    repository TEXT      NOT NULL,
    issue      BIGINT    NOT NULL,

    state      TEXT      NOT NULL,
    timestamp  TIMESTAMP NOT NULL,

    FOREIGN KEY (repository, issue) REFERENCES issues (repository, issue)
);

CREATE TABLE issue_labels_history
(
    id         SERIAL PRIMARY KEY,
    repository TEXT      NOT NULL,
    issue      BIGINT    NOT NULL,

    label      TEXT      NOT NULL,
    timestamp  TIMESTAMP NOT NULL,
    action     TEXT CHECK ( action IN ('ADDED', 'REMOVED') ),

    FOREIGN KEY (repository, issue) REFERENCES issues (repository, issue)
);


CREATE TABLE pull_requests
(
    repository     TEXT   NOT NULL,
    pr             BIGINT NOT NULL,

    contributor_id BIGINT NOT NULL REFERENCES contributors (github_id),
    current_state  TEXT   NOT NULL,
    merge_sha      TEXT,
    timestamp      TIMESTAMP,

    PRIMARY KEY (repository, pr)
);

CREATE TABLE pr_state_history
(
    id         SERIAL PRIMARY KEY,
    repository TEXT      NOT NULL,
    pr         BIGINT    NOT NULL,

    state      TEXT      NOT NULL,
    timestamp  TIMESTAMP NOT NULL,
    merge_sha  TEXT,

    FOREIGN KEY (repository, pr) REFERENCES pull_requests (repository, pr)

);

CREATE TABLE pr_labels_history
(
    id         SERIAL PRIMARY KEY,
    repository TEXT      NOT NULL,
    pr         BIGINT    NOT NULL,

    label      TEXT      NOT NULL,
    timestamp  TIMESTAMP NOT NULL,
    action     TEXT CHECK ( action IN ('ADDED', 'REMOVED') ),

    FOREIGN KEY (repository, pr) REFERENCES pull_requests (repository, pr)
);

CREATE TABLE file_activity
(
    id             SERIAL PRIMARY KEY,
    repository     TEXT      NOT NULL,
    pr             BIGINT    NOT NULL,

    file_path      TEXT      NOT NULL,
    contributor_id BIGINT    NOT NULL,
    activity_type  TEXT,
    timestamp      TIMESTAMP NOT NULL,

    FOREIGN KEY (repository, pr) REFERENCES pull_requests (repository, pr)
);