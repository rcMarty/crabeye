-- ============================================================
-- All tables – final schema
-- ============================================================

CREATE TABLE teams
(
    team       TEXT PRIMARY KEY,
    subteam_of TEXT,
    kind       TEXT,

    CONSTRAINT fk_teams_subteam     FOREIGN KEY (subteam_of) REFERENCES teams (team),
    CONSTRAINT check_no_self_parent CHECK (team != subteam_of)
);

CREATE TABLE contributors
(
    github_id   BIGINT PRIMARY KEY,
    github_name TEXT NOT NULL,
    name        TEXT
);

CREATE TABLE contributors_teams
(
    team           TEXT   NOT NULL REFERENCES teams (team),
    contributor_id BIGINT NOT NULL,

    PRIMARY KEY (team, contributor_id),
    CONSTRAINT fk_contributors_teams_contributor FOREIGN KEY (contributor_id) REFERENCES contributors (github_id)
);

CREATE TABLE issues
(
    repository     TEXT      NOT NULL,
    issue          BIGINT,
    is_pr          BOOLEAN   NOT NULL,

    contributor_id BIGINT    NOT NULL REFERENCES contributors (github_id),
    current_state  TEXT      NOT NULL,
    edited_at      TIMESTAMP NOT NULL,
    created_at     TIMESTAMP NOT NULL,
    merge_sha      TEXT,

    PRIMARY KEY (repository, issue)
);

CREATE TABLE issue_event_history
(
    id         SERIAL PRIMARY KEY,
    repository TEXT      NOT NULL,
    issue      BIGINT    NOT NULL,

    is_pr      BOOLEAN   NOT NULL,

    event      TEXT      NOT NULL,
    timestamp  TIMESTAMP NOT NULL,

    FOREIGN KEY (repository, issue) REFERENCES issues (repository, issue),
    CONSTRAINT uq_issue_event_history_conflict UNIQUE (repository, issue, timestamp, event)
);

CREATE TABLE issue_labels_history
(
    id         SERIAL PRIMARY KEY,
    repository TEXT      NOT NULL,
    issue      BIGINT    NOT NULL,

    is_pr      BOOLEAN   NOT NULL,

    label      TEXT      NOT NULL,
    timestamp  TIMESTAMP NOT NULL,
    action     TEXT      NOT NULL CHECK (action IN ('ADDED', 'REMOVED')),

    FOREIGN KEY (repository, issue) REFERENCES issues (repository, issue),
    CONSTRAINT uq_issue_labels_history_conflict UNIQUE (repository, issue, timestamp, label)
);

CREATE TABLE file_activity
(
    id             SERIAL PRIMARY KEY,
    repository     TEXT      NOT NULL,
    issue          BIGINT    NOT NULL,

    file_path      TEXT      NOT NULL,
    contributor_id BIGINT    NOT NULL,
    activity_type  TEXT,
    timestamp      TIMESTAMP NOT NULL,

    FOREIGN KEY (repository, issue) REFERENCES issues (repository, issue),
    CONSTRAINT uq_file_activity_conflict UNIQUE (repository, issue, timestamp, file_path)
);

