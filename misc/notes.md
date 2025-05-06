## benchmarks

cca 300 requestů na stránky na api po 100 záznamech na stránku za cca 5.5 minut
1150 za 20 minut
hodina 18 pro 4500 requestů,

insert všech záznamů do db trvá 1.947s

takže cca lineárně kolem 60 za minutu

db před 4500 pages requestem
file_activity = 28928 po 28928
pr_event_log = 3623 po 4354

do db cca 1min 1200 pr
2100 pr za 2 minuty
5.40 min pro 4900pr

release
5 min => 5300pr

850MB ramky

PŘECHOD NA POSTGRES kvůli jednoduchosti nasaditelnosti a kvůli migracím že jsou složité





