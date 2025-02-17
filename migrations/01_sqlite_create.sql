CREATE TABLE users
(
    id    INTEGER PRIMARY KEY,
    login TEXT
);

CREATE TABLE pull_requests
(
    pr_number   REAL PRIMARY KEY,
    author_id   INTEGER,
    state       TEXT,
    title       TEXT,
    description TEXT,
    created_at  DATETIME,
    updated_at  DATETIME,
    files_state TEXT,
    FOREIGN KEY (author_id) REFERENCES users (id)
);