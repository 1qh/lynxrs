use sea_orm_migration::prelude::*;

mod m20260424_000001_create_users;
mod m20260424_000002_create_file_objects;
mod m20260424_000003_password_resets;
mod m20260424_000004_email_ci_index;
mod m20260424_000005_add_user_role;
mod m20260424_000006_email_verification;
mod m20260424_000007_session_version;
mod m20260424_000008_api_tokens;
mod m20260424_000009_file_shares;
mod m20260424_000010_share_downloads;
mod m20260424_000011_audit_log;
mod m20260424_000012_mfa;
mod m20260424_000013_webhooks;
mod m20260424_000014_file_checksum;
mod m20260424_000015_login_lockout;
mod m20260424_000016_mfa_recovery;
mod m20260424_000017_soft_delete;
mod m20260424_000018_file_tags;
mod m20260424_000019_webhook_deliveries;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260424_000001_create_users::Migration),
            Box::new(m20260424_000002_create_file_objects::Migration),
            Box::new(m20260424_000003_password_resets::Migration),
            Box::new(m20260424_000004_email_ci_index::Migration),
            Box::new(m20260424_000005_add_user_role::Migration),
            Box::new(m20260424_000006_email_verification::Migration),
            Box::new(m20260424_000007_session_version::Migration),
            Box::new(m20260424_000008_api_tokens::Migration),
            Box::new(m20260424_000009_file_shares::Migration),
            Box::new(m20260424_000010_share_downloads::Migration),
            Box::new(m20260424_000011_audit_log::Migration),
            Box::new(m20260424_000012_mfa::Migration),
            Box::new(m20260424_000013_webhooks::Migration),
            Box::new(m20260424_000014_file_checksum::Migration),
            Box::new(m20260424_000015_login_lockout::Migration),
            Box::new(m20260424_000016_mfa_recovery::Migration),
            Box::new(m20260424_000017_soft_delete::Migration),
            Box::new(m20260424_000018_file_tags::Migration),
            Box::new(m20260424_000019_webhook_deliveries::Migration),
        ]
    }
}
