//! Background housekeeping tasks — run on a separate tokio task alongside the server.

use object_store::ObjectStoreExt;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;
use std::time::Duration;

use crate::entity::{
    audit_event, email_verification, file_object, password_reset, webhook_delivery,
};

/// Spawn a task that periodically deletes expired/used tokens.
pub fn spawn(
    db: DatabaseConnection,
    interval: Duration,
    storage: Arc<dyn object_store::ObjectStore>,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            if let Err(e) = run_once(&db, storage.as_ref()).await {
                tracing::warn!(error=%e, "housekeeping run failed");
            }
        }
    });
}

pub async fn run_once(
    db: &DatabaseConnection,
    storage: &dyn object_store::ObjectStore,
) -> anyhow::Result<()> {
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

    // Permanently purge trashed files older than 30 days.
    let cutoff = now - chrono::Duration::days(30);
    let old = file_object::Entity::find()
        .filter(file_object::Column::DeletedAt.lt(cutoff))
        .all(db)
        .await?;
    let mut purged = 0u64;
    for r in old {
        let p = object_store::path::Path::from(r.storage_key.clone());
        let _ = storage.delete(&p).await;
        if file_object::Entity::delete_by_id(r.id)
            .exec(db)
            .await
            .is_ok()
        {
            purged += 1;
        }
    }

    // Audit retention — keep 90 days.
    let ai_cutoff = now - chrono::Duration::days(90);
    let audit_deleted = audit_event::Entity::delete_many()
        .filter(audit_event::Column::CreatedAt.lt(ai_cutoff))
        .exec(db)
        .await?
        .rows_affected;

    // Webhook deliveries retention — keep 30 days.
    let whd_cutoff = now - chrono::Duration::days(30);
    let whd_deleted = webhook_delivery::Entity::delete_many()
        .filter(webhook_delivery::Column::CreatedAt.lt(whd_cutoff))
        .exec(db)
        .await?
        .rows_affected;

    if pr_deleted > 0 || ev_deleted > 0 || purged > 0 || audit_deleted > 0 || whd_deleted > 0 {
        tracing::info!(
            pr_deleted,
            ev_deleted,
            purged,
            audit_deleted,
            whd_deleted,
            "housekeeping sweep done"
        );
    }
    Ok(())
}
