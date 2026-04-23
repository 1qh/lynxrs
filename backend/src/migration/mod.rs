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
        ]
    }
}
