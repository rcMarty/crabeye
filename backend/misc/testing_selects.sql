SELECT fa.file_path, count(*) as editions
FROM file_activity fa
         JOIN contributors_teams ct
              ON ct.contributor_id = fa.contributor_id
                  AND ct.team = 'compiler'
WHERE fa.repository = 'rust-lang/rust'
  AND fa.timestamp BETWEEN '2024-01-01' AND '2024-12-31'
GROUP BY fa.file_path
ORDER BY editions DESC;

SELECT subquery.label     as label,
       subquery.timestamp as timestamp,
       subquery.action    as label_event
FROM (SELECT DISTINCT ON (issue, label) *
      FROM issue_labels_history
      WHERE issue = 154545
        and repository = 'rust-lang/rust'
        and timestamp <= '2026-06-01'
        and is_pr = true
        and label like 'S-%'
      ORDER BY issue, label, timestamp DESC) subquery
WHERE action = 'ADDED';


WITH latest_labels AS (
    -- For each PR, keep only the most recent label event for the target S-* label up to T
    SELECT DISTINCT ON (issue) issue, action
    FROM issue_labels_history
    WHERE repository = 'rust-lang/rust'
      AND is_pr = true
      AND label = 'S-waiting-on-author'
      AND timestamp <= '2026-06-01'
    ORDER BY issue, timestamp DESC),
     latest_state AS (
         -- For each PR, keep the most recent state-change event (closed / merged / reopened) up to T
         SELECT DISTINCT ON (issue) issue, event
         FROM issue_event_history
         WHERE repository = 'rust-lang/rust'
           AND is_pr = true
           AND event IN ('closed', 'merged', 'reopened')
           AND timestamp <= '2026-06-01'
         ORDER BY issue, timestamp DESC)
SELECT COUNT(*)
FROM latest_labels ll
         LEFT JOIN latest_state ls ON ls.issue = ll.issue
WHERE ll.action = 'ADDED'
  -- PR must be open at T (no close/merge event, or most recent was 'reopened')
  AND (ls.issue IS NULL OR ls.event = 'reopened');



select issue as pr_id, repository, file_path, github_id, github_name, name
from file_activity
         join contributors c on file_activity.contributor_id = c.github_id
where contributor_id = ANY (SELECT github_id
                            FROM contributors
                            WHERE github_name = 'Kobzol')
  and timestamp between '2025-01-01' and '2026-12-31'
  and repository = 'rust-lang/rust'
order by timestamp DESC
LIMIT 100;


SELECT c.github_id, c.github_name, c.name
FROM contributors c
         JOIN (SELECT DISTINCT contributor_id
               FROM file_activity
               WHERE file_path LIKE 'compiler/%'
                 AND timestamp BETWEEN '2025-01-01' AND '2026-12-31'
                 AND repository = 'rust-lang/rust'
               ORDER BY contributor_id
               OFFSET 0 LIMIT 100) fa ON fa.contributor_id = c.github_id;


WITH current_waiting_labels AS (
    -- Per PR keep only the single most-recently-changed waiting label
    SELECT DISTINCT ON (issue) timestamp,
                               issue,
                               label,
                               action
    FROM issue_labels_history
    WHERE repository = 'rust-lang/rust'
      AND is_pr = true
      AND label IN ('S-waiting-on-review', 'S-waiting-on-bors', 'S-waiting-on-author')
    ORDER BY issue, timestamp DESC)
SELECT c.repository     AS repository,
       l.issue          AS pr,
       l.label          AS state,
       l.timestamp      AS edited_at,
       c.created_at     AS created_at,
       c.merge_sha      AS merge_sha,
       c.contributor_id AS author_id
FROM current_waiting_labels l
         JOIN issues c ON l.issue = c.issue AND c.repository = 'rust-lang/rust'
WHERE l.action = 'ADDED'
  AND c.is_pr = true
  AND c.current_state NOT IN ('closed', 'merged')
;


--aaa
WITH pr_lifecycle AS (SELECT i.repository,
                             i.issue,
                             i.created_at::date                      AS opened_on,
                             MIN(CASE
                                     WHEN e.event IN ('closed', 'merged')
                                         THEN e.timestamp END)::date AS closed_on
                      FROM issues i
                               LEFT JOIN issue_event_history e
                                         ON e.repository = i.repository
                                             AND e.issue = i.issue
                                             AND e.event IN ('closed', 'merged')
                      WHERE i.is_pr = TRUE
                        and i.repository = 'rust-lang/rust'
                      GROUP BY i.repository, i.issue, i.created_at),
     days AS (SELECT d::date AS day
              FROM generate_series(
                           (SELECT MIN(opened_on) FROM pr_lifecycle),
                           CURRENT_DATE,
                           '1 day'::interval
                   ) d)
SELECT d.day,
       COUNT(*) AS open_prs
FROM days d
         JOIN pr_lifecycle p
              ON d.day >= p.opened_on
                  AND (p.closed_on IS NULL OR d.day < p.closed_on)
GROUP BY d.day
ORDER BY d.day
