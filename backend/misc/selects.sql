select count(event)
from issue_event_history
where event = 'merged'
  and timestamp > '2023-01-01';

-- 2) Jaký byl stav konkrétního issue v daný timestamp?
-- sissueavit že ta změna nemusí být v tom between může být před
SELECT distinct event, timestamp
FROM issue_event_history
WHERE issue = 138694
  and timestamp between '2025-03-21' and '2025-03-22'
ORDER BY timestamp DESC;

-- 1) Jaký byl počet issue v daném stavu (waiting for review, waiting for author, waiting for bors, merged) v daný timestamp/den.
SELECT count(*) as count
FROM issue_event_history
WHERE timestamp BETWEEN '2025-03-21' AND '2025-03-22'
  AND event = 'open';

-- 3) issueo daného uživatele/tým (z https://github.com/rust-lang/team), jakých je top N souborů, které byly buď uissueaveny nebo reviewovány za posledních N časových jednotek?
-- TODO netuším jk získat zda byl soubor uissueaven nebo reviewován. odkud to zjistím?
-- řidat sloupeček reviewreea

select issue, file_path, timestamp
from file_activity
where contributor_id = 4539057
  and timestamp between '2025-03-21' and '2025-03-22'
order by timestamp desc
limit 10;

-- issueo daného uživatele v časovém období kolik souborů změnil v kterých issue
select issue, count(file_path) as count
from file_activity
where contributor_id = 476013
  and timestamp between '2025-03-21' and '2025-03-22'
group by issue;


-- 4) issueo daný soubor/složku, kteří uživatelé/týmy jej v posledních N časových jednotkách uissueavovali nebo reviewovali?
-- TODO jak poznám že reviewovali
select distinct contributor_id, issue
from file_activity
where file_path like 'compiler/rustc_hir_issueetty/src/lib.rs%'
  and timestamp between '2022-03-21' and '2026-03-22';

-- kolikrát se uissueavil jaký soubor
select distinct file_path, count(file_path) as count
from file_activity
group by file_path
order by count desc;

-- 5) dotaz: issue, které čekají nejdelší dobu na review (jednodušší verze: jsou nejdelší čas ve stavu "waiting-on-review",
select issue, timestamp
from issue_event_history as p
where NOT EXISTS (SELECT id
                  FROM issue_event_history AS p2
                  WHERE p.id = p2.id
                    AND p2.timestamp > p.timestamp)
  AND (p.event = 'S-waiting-on-review' OR p.event = 'S-waiting-on-bors' OR
       p.event = 'S-waiting-on-author')
order by timestamp;


-- 6) u těch změněných souborů by to mělo fungovat hierarchicky
-- např. když issue 1 změní a/b/c a issue 2 změní a/b/d, tak když se zeptám co změnilo naposledy složku a/b, tak by to mělo vrátit 1 i 2
-- tohle git umí automaticky, přes SQL by se to asi dělalo naivně skenem přes všechny issue v daném časovém horizontu
select *
from file_activity
where file_path like 'src/libsyntax/parse/%';

select *
from issue_event_history
where issue = 129342;



select distinct github_id, github_name
from contributors
where github_id in (select distinct contributor_id
                    from file_activity
                    where file_path like 'src/%'
                      and timestamp between '2024-03-21' and '2025-03-22'
                    order by contributor_id
                    limit 100 offset 0);

select distinct contributor_id
from file_activity
where file_path like $1
  and timestamp between $2 and $3


--graf počtu issue vytvořených lidmi z teamu vs lidmi mimo Rust týmy


--počet zavřených/otevřených issue za den, ve stacked bar chartu, by byl zajímavý. a to stejné s issues


select * from issue_event_history
where issue = 8412;

SELECT *
FROM (
         SELECT DISTINCT ON (issue, label) *
         FROM issue_labels_history
         WHERE issue = 8412 and repository = 'rust-lang/rust'
         ORDER BY issue, label, timestamp DESC
         -- dej mi jen poslední záznamy podle mimo jiné timestamp issueo každou kombinaci issue a label_event
         -- (zaručeno že ta akce bude latest)
     ) subquery
WHERE action = 'ADDED';
-- jen ty co byly přidány



select issue as issue_id, file_path, github_id, github_name, name
from file_activity
         join contributors c on file_activity.contributor_id = c.github_id
where contributor_id = ANY(123457)
  and timestamp between '2024-01-01' and '2026-01-01'
order by timestamp DESC
LIMIT 100 ;


SELECT MAX(timestamp) as timestamp
FROM issue_event_history
WHERE is_pr = true;

-- všechny isues které nemají záznam v issue_event_history -> nedotahovalo se přes timeline api
select repository, issue
from issues
where (repository, issue) NOT IN (SELECT repository, issue FROM issue_event_history)
    and is_pr = true;