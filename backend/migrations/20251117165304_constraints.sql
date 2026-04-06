-- Add migration script here
ALTER TABLE teams
    ADD CONSTRAINT fk_teams_subteam
        FOREIGN KEY (subteam_of) REFERENCES teams (team);

ALTER TABLE teams
    ADD CONSTRAINT check_no_self_parent
        CHECK (team != subteam_of);

ALTER TABLE contributors_teams
    ADD CONSTRAINT fk_contributors_teams_contributor
        FOREIGN KEY (contributor_id) REFERENCES contributors (github_id);
