use sea_orm_migration::prelude::*;

mod m20260424_000001_create_users;
mod m20260424_000002_create_file_objects;
mod m20260424_000003_password_resets;
mod m20260424_000004_email_ci_index;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260424_000001_create_users::Migration),
            Box::new(m20260424_000002_create_file_objects::Migration),
            Box::new(m20260424_000003_password_resets::Migration),
            Box::new(m20260424_000004_email_ci_index::Migration),
        ]
    }
}
