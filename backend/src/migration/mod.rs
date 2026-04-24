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
mod m20260424_000020_file_versions;
mod m20260424_000021_share_password;
mod m20260424_000022_user_profile;
mod m20260424_000023_token_scopes;
mod m20260424_000024_organizations;
mod m20260424_000025_file_org_id;
mod m20260424_000026_org_invites;

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
            Box::new(m20260424_000020_file_versions::Migration),
            Box::new(m20260424_000021_share_password::Migration),
            Box::new(m20260424_000022_user_profile::Migration),
            Box::new(m20260424_000023_token_scopes::Migration),
            Box::new(m20260424_000024_organizations::Migration),
            Box::new(m20260424_000025_file_org_id::Migration),
            Box::new(m20260424_000026_org_invites::Migration),
        ]
    }
}
