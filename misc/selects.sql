select count(state)
from pr_event_log
where state = 'merged'
  and timestamp > '2023-01-01';

-- 2) Jaký byl stav konkrétního PR v daný timestamp?
SELECT distinct state
FROM pr_event_log
WHERE pr = 138694
  and timestamp between '2025-03-21' and '2025-03-22'
ORDER BY timestamp DESC;

-- 1) Jaký byl počet PR v daném stavu (waiting for review, waiting for author, waiting for bors, merged) v daný timestamp/den.
SELECT count(*) as count
FROM pr_event_log
WHERE timestamp BETWEEN '2025-03-21' AND '2025-03-22'
  AND state = 'open';

-- 3) Pro daného uživatele/tým (z https://github.com/rust-lang/team), jakých je top N souborů, které byly buď upraveny nebo reviewovány za posledních N časových jednotek?
-- netuším jk získat zda byl soubor upraven nebo reviewován. odkud to zjistím?
select pr, file_path
from file_activity
where user_login = 476013
  and timestamp between '2025-03-21' and '2025-03-22'
  and pr = 138791;


select pr, count(file_path) as count
from file_activity
where user_login = 476013
  and timestamp between '2025-03-21' and '2025-03-22'
group by pr;


-- 4) Pro daný soubor/složku, kteří uživatelé/týmy jej v posledních N časových jednotkách upravovali nebo reviewovali?
select distinct user_login as count
from file_activity
where file_path = 'compiler/rustc_hir_pretty/src/lib.rs'
  and timestamp between '2025-03-21' and '2025-03-22';


select distinct file_path, count(file_path) as count
from file_activity
group by file_path
order by count desc;

-- 5) dotaz: PR, které čekají nejdelší dobu na review (jednodušší verze: jsou nejdelší čas ve stavu "waiting-on-review",
select pr, timestamp
from pr_event_log
where state = 'open'
order by timestamp;
