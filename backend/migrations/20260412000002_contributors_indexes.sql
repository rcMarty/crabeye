-- USER LOOKUP: Users exist independently of issues/PRs
CREATE INDEX idx_contributors_name ON contributors (github_name);

