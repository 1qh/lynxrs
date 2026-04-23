//! Background housekeeping tasks — run on a separate tokio task alongside the server.

use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::time::Duration;

use crate::entity::{email_verification, password_reset};

/// Spawn a task that periodically deletes expired/used tokens.
pub fn spawn(db: DatabaseConnection, interval: Duration) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            if let Err(e) = run_once(&db).await {
                tracing::warn!(error=%e, "housekeeping run failed");
            }
        }
    });
}

async fn run_once(db: &DatabaseConnection) -> anyhow::Result<()> {
    let now = chrono::Utc::now();

    // Delete password reset tokens that are either expired or consumed.
    let pr_deleted = password_reset::Entity::delete_many()
        .filter(
            password_reset::Column::ExpiresAt
                .lt(now)
                .or(password_reset::Column::UsedAt.is_not_null()),
        )
        .exec(db)
        .await?
        .rows_affected;

    let ev_deleted = email_verification::Entity::delete_many()
        .filter(
            email_verification::Column::ExpiresAt
                .lt(now)
                .or(email_verification::Column::UsedAt.is_not_null()),
        )
        .exec(db)
        .await?
        .rows_affected;

    if pr_deleted > 0 || ev_deleted > 0 {
        tracing::info!(
            pr_deleted,
            ev_deleted,
            "housekeeping swept expired auth tokens"
        );
    }
    Ok(())
}
