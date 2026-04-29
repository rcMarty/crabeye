-- =============================================================================
-- Analyzované SQL dotazy – aplikace crabeye
-- Databáze: PostgreSQL 14, repozitář: rust-lang/rust
-- Počty řádků: issues ~200 000, issue_event_history ~564 000,
--              issue_labels_history ~355 000, file_activity ~525 000
-- =============================================================================


-- ---------------------------------------------------------------------------
-- Q1: Počet PR ve stavu „S-waiting-on-review" k danému datu
--     (funkce: get_pr_count_in_state – větev WaitingForReview/Bors/Author)
--
--     Princip: DISTINCT ON (issue) zachová pro každý PR pouze nejnovější
--     label-event a nejnovější stavovou změnu k časovému řezu T.
--     LEFT JOIN zajistí zahrnutí PR bez stavové změny (nikdy nezavřené).
-- ---------------------------------------------------------------------------
explain (ANALYZE, BUFFERS)
WITH latest_labels AS (
    -- Pro každý PR zachová nejnovější label-event pro cílový S-* label k T
    SELECT DISTINCT ON (issue) issue, action
    FROM issue_labels_history
    WHERE repository = 'rust-lang/rust'
      AND is_pr = true
      AND label = 'S-waiting-on-review' -- parametr: $3
      AND timestamp <= '2026-01-01'     -- parametr: $2
    ORDER BY issue, timestamp DESC),
     latest_state AS (
         -- Pro každý PR zachová nejnovější stavovou změnu (closed/merged/reopened) k T
         SELECT DISTINCT ON (issue) issue, event
         FROM issue_event_history
         WHERE repository = 'rust-lang/rust' -- parametr: $1
           AND is_pr = true
           AND event IN ('closed', 'merged', 'reopened')
           AND timestamp <= '2026-01-01'     -- parametr: $2
         ORDER BY issue, timestamp DESC)
SELECT COUNT(*)
FROM latest_labels ll
         LEFT JOIN latest_state ls ON ls.issue = ll.issue
WHERE ll.action = 'ADDED'
  -- PR musí být k T otevřen (žádná zavírací událost, nebo poslední byla 'reopened')
  AND (ls.issue IS NULL OR ls.event = 'reopened');


-- ---------------------------------------------------------------------------
-- Q2: Kumulativní počet merged PR k danému datu
--     (funkce: get_pr_count_in_state – větev Merged)
--
--     Princip: stav „merged" je trvalý – stačí COUNT(DISTINCT issue)
--     nad jedinou tabulkou s indexovatelným filtrem.
-- ---------------------------------------------------------------------------
explain (ANALYZE, BUFFERS)
SELECT COUNT(DISTINCT issue)
FROM issue_event_history
WHERE repository = 'rust-lang/rust' -- parametr: $1
  AND is_pr = true
  AND event = 'merged'
  AND timestamp <= '2026-01-01';
-- parametr: $2


-- ---------------------------------------------------------------------------
-- Q3: Soubory modifikované členy daného týmu v časovém okně
--     (funkce: get_files_modified_by_team)
--
--     Princip: Nested Loop Join přes members týmu (contributors_teams)
--     a pokrývající index na file_activity (repository, contributor_id, timestamp).
-- ---------------------------------------------------------------------------

explain (ANALYZE, BUFFERS)
SELECT fa.file_path, count(*) AS editions
FROM file_activity fa
         JOIN contributors_teams ct
              ON ct.contributor_id = fa.contributor_id
                  AND ct.team = 'compiler' -- parametr: $4
WHERE fa.repository = 'rust-lang/rust' -- parametr: $1
  AND fa.timestamp BETWEEN '2025-06-01' -- parametr: $2
    AND '2026-04-04'                   -- parametr: $3
GROUP BY fa.file_path
ORDER BY editions DESC;


-- ---------------------------------------------------------------------------
-- Q4: Denní vývoj počtu PR ve stavu „S-waiting-on-review" v časovém rozsahu
--     (funkce: get_pr_count_in_state_over_time – větev WaitingForReview/Bors/Author)
--
--     Dotaz vrací pro každý den v zadaném rozsahu počet PR, které jsou
--     současně otevřené (event IN created/reopened) a mají aktivní label
--     „S-waiting-on-review". Místo naivního přístupu (samostatný poddotaz
--     pro každý den, složitost O(days × events)) je použit delta-event přístup
--     se složitostí O(events + days):
--
--     Princip (delta-event přístup):
--       1. all_transitions      – stavové události každého PR (created/closed/merged/reopened)
--                                  z issue_event_history
--       2. ordered_transitions  – ke každé události přiřazen timestamp následující události
--                                  téhož PR pomocí LEAD() OVER (PARTITION BY issue)
--       3. open_periods         – tsrange [created|reopened, next_event) – periody otevřenosti PR
--       4. label_transitions    – události přidání/odebrání cílového labelu s timestampem
--                                  následující label-události téhož PR (opět LEAD())
--       5. label_active_periods – tsrange [ADDED, next_label_event) – periody aktivity labelu
--       6. in_state_periods     – průnik open_periods × label_active_periods:
--                                  && ověří neprázdnost průniku, * ho vypočítá
--       7. period_deltas        – UNION ALL: dolní mez každého intervalu → +1,
--                                  horní mez (není-li nekonečná) → −1
--       8. daily_deltas         – SUM(delta) seskupeno po dnech
--       9. base                 – kumulativní součet všech delt před začátkem rozsahu
--                                  (počet PR ve stavu těsně před začátkem sledovaného okna)
--      10. date_series          – generovaná řada dnů [from, to]; LEFT JOIN s daily_deltas
--                                  a okenní SUM OVER ORDER BY day → výsledná časová řada
--
--     Složitost: O(events + days) místo naivního O(days × events).
-- ---------------------------------------------------------------------------
explain (ANALYZE, BUFFERS)
WITH all_transitions AS (SELECT issue, timestamp, event AS event_type
                         FROM issue_event_history
                         WHERE repository = 'rust-lang/rust'         -- parametr: $1
                           AND is_pr = true
                           AND event IN ('created', 'closed', 'merged', 'reopened')),
     ordered_transitions AS (SELECT issue,
                                    timestamp,
                                    event_type,
                                    -- LEAD() vrátí timestamp následující události téhož PR → konec intervalu
                                    LEAD(timestamp) OVER (PARTITION BY issue ORDER BY timestamp) AS next_ts
                             FROM all_transitions),
     open_periods AS (SELECT issue,
                             -- Interval otevřenosti PR: [vznik/znovuotevření, následující událost)
                             tsrange(timestamp, COALESCE(next_ts, 'infinity'::timestamp),
                                     '[)') AS period
                      FROM ordered_transitions
                      WHERE event_type IN ('created', 'reopened')),
     label_transitions AS (SELECT issue,
                                  timestamp,
                                  action,
                                  LEAD(timestamp) OVER (PARTITION BY issue ORDER BY timestamp) AS next_ts
                           FROM issue_labels_history
                           WHERE repository = 'rust-lang/rust'       -- parametr: $1
                             AND is_pr = true
                             AND label = 'S-waiting-on-review'),      -- parametr: $4
-- Interval aktivity labelu: [ADDED, následující label-event)
     label_active_periods AS (SELECT issue,
                                     tsrange(timestamp, COALESCE(next_ts, 'infinity'::timestamp),
                                             '[)') AS period
                              FROM label_transitions
                              WHERE action = 'ADDED'),
     in_state_periods AS (SELECT lap.issue,
                                 -- Průnik intervalu otevřenosti a aktivity labelu = skutečný interval „ve stavu"
                                 lap.period * op.period AS valid_period
                          FROM label_active_periods lap
                                   JOIN open_periods op
                                        ON lap.issue = op.issue
                                            AND lap.period && op.period    -- && = neprázdný průnik
                          WHERE NOT isempty(lap.period * op.period)),
-- Každý interval převeden na delta-events: +1 při vstupu do stavu, −1 při výstupu.
-- Protože LEAD() generuje nepřekrývající se intervaly na PR, běžící SUM
-- je ekvivalentní COUNT(DISTINCT issue) v libovolném časovém bodě.
     period_deltas AS (SELECT lower(valid_period)::date AS event_date, 1 AS delta
                       FROM in_state_periods
                       UNION ALL
                       SELECT upper(valid_period)::date AS event_date, -1 AS delta
                       FROM in_state_periods
                       WHERE NOT upper_inf(valid_period)),
     daily_deltas AS (SELECT event_date, SUM(delta) AS daily_change
                      FROM period_deltas
                      GROUP BY event_date),
     -- Kumulativní hodnota před začátkem sledovaného rozsahu (základ pro běžící součet)
     base AS (SELECT COALESCE(SUM(daily_change), 0) AS cnt
              FROM daily_deltas
              WHERE event_date < '2025-06-01'::date),                -- parametr: $2
     date_series AS (SELECT d::date AS day
                     FROM generate_series('2025-06-01'::timestamp,   -- parametr: $2
                                          '2026-04-04'::timestamp,   -- parametr: $3
                                          '1 day'::interval) d)
SELECT ds.day                                                                  AS date,
       ((SELECT cnt FROM base)
           + COALESCE(SUM(dd.daily_change) OVER (ORDER BY ds.day), 0))::bigint AS count
FROM date_series ds
         LEFT JOIN daily_deltas dd ON dd.event_date = ds.day
ORDER BY ds.day
