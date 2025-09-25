select count(state)
from pr_state_history
where state = 'merged'
  and timestamp > '2023-01-01';

-- 2) Jaký byl stav konkrétního PR v daný timestamp?
-- spravit že ta změna nemusí být v tom between může být před
SELECT distinct state
FROM pr_state_history
WHERE pr = 138694
  and timestamp between '2025-03-21' and '2025-03-22'
ORDER BY timestamp DESC;

-- 1) Jaký byl počet PR v daném stavu (waiting for review, waiting for author, waiting for bors, merged) v daný timestamp/den.
SELECT count(*) as count
FROM pr_state_history
WHERE timestamp BETWEEN '2025-03-21' AND '2025-03-22'
  AND state = 'open';

-- 3) Pro daného uživatele/tým (z https://github.com/rust-lang/team), jakých je top N souborů, které byly buď upraveny nebo reviewovány za posledních N časových jednotek?
-- TODO netuším jk získat zda byl soubor upraven nebo reviewován. odkud to zjistím?
-- řidat sloupeček reviewreea

select pr, file_path, timestamp
from file_activity
where user_login = 4539057
  and timestamp between '2025-03-21' and '2025-03-22'
order by timestamp desc
limit 10;

-- pro daného uživatele v časovém období kolik souborů změnil v kterých PR
select pr, count(file_path) as count
from file_activity
where user_login = 476013
  and timestamp between '2025-03-21' and '2025-03-22'
group by pr;


-- 4) Pro daný soubor/složku, kteří uživatelé/týmy jej v posledních N časových jednotkách upravovali nebo reviewovali?
-- TODO jak poznám že reviewovali
select distinct user_login, pr
from file_activity
where file_path like 'compiler/rustc_hir_pretty/src/lib.rs%'
  and timestamp between '2022-03-21' and '2026-03-22';

-- kolikrát se upravil jaký soubor
select distinct file_path, count(file_path) as count
from file_activity
group by file_path
order by count desc;

-- 5) dotaz: PR, které čekají nejdelší dobu na review (jednodušší verze: jsou nejdelší čas ve stavu "waiting-on-review",
select pr, timestamp
from pr_state_history as p
where NOT EXISTS (SELECT id FROM pr_state_history AS p2 WHERE p.id = p2.id AND p2.timestamp > p.timestamp)
  AND (p.state = 'S-waiting-on-review' OR p.state = 'S-waiting-on-bors' OR p.state = 'S-waiting-on-author')
order by timestamp;


-- 6) u těch změněných souborů by to mělo fungovat hierarchicky
-- např. když PR 1 změní a/b/c a PR 2 změní a/b/d, tak když se zeptám co změnilo naposledy složku a/b, tak by to mělo vrátit 1 i 2
-- tohle git umí automaticky, přes SQL by se to asi dělalo naivně skenem přes všechny PR v daném časovém horizontu
select *
from file_activity
where file_path like 'src/libsyntax/parse/%';

select *
from pr_state_history
where pr = 129342;



select distinct github_id, github_name
from team_members
where github_id in (select distinct user_login
             from file_activity
             where file_path like 'src/%'
               and timestamp between '2024-03-21' and '2025-03-22'
             order by user_login
             limit 100 offset 0
             );

select distinct user_login
from file_activity
where file_path like $1
  and timestamp between $2 and $3