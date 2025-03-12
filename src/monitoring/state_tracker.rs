// monitoring/state_tracker.rs
// pub struct StateMonitor {
//     github: GitHubClient,
//     db: Database,
//     interval: Duration,
// }
//
// impl StateMonitor {
//     pub async fn run(&self) {
//         let mut interval = time::interval(self.interval);
//         loop {
//             interval.tick().await;
//             self.check_prs().await;
//         }
//     }
//
//     async fn check_prs(&self) {
//         // 1. Získat všechny PRs z GitHubu
//         // 2. Pro každý PR získat aktuální stav
//         // 3. Porovnat s posledním stavem v DB
//         // 4. Pokud se změnil, uložit nový event
//     }
// }
