use crate::db::Database;
// use octocrab::models::webhook_events::WebhookEventType;

// api/webhooks.rs
pub async fn handle_webhook(_event: octocrab::models::webhook_events::WebhookEvent, _db: Database) {
    todo!();

    // match event {
    //     WebhookEventType::PullRequest() => {
    //         let event = PrEvent::from_webhook(pr_event);
    //         db.insert_event(&event).await?;
    //     } // Ostatní eventy...
    // }
    // Ok(StatusCode::OK)
}
