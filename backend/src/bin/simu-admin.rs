//! simu-admin CLI — seed/promote admin users without opening a DB shell.
//!
//! Usage:
//!   cargo run --bin simu-admin -- promote --email you@example.com
//!   cargo run --bin simu-admin -- create  --email admin@example.com --password s3cret-pw

use std::env;

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, SaltString, rand_core::OsRng},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, Database, EntityTrait, QueryFilter, Set,
};
use sea_orm_migration::MigratorTrait;
use simu_backend::{entity::user, migration::Migrator};

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = env::args().collect();
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        if a == name {
            return iter.next().cloned();
        }
    }
    None
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).cloned().unwrap_or_default();
    let email = arg("--email").unwrap_or_else(|| {
        eprintln!("--email required");
        std::process::exit(2);
    });

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL");
    let db = Database::connect(ConnectOptions::new(database_url)).await?;
    Migrator::up(&db, None).await?;

    let email_norm = email.trim().to_lowercase();

    match cmd.as_str() {
        "promote" => {
            let u = user::Entity::find()
                .filter(user::Column::Email.eq(&email_norm))
                .one(&db)
                .await?
                .ok_or_else(|| anyhow::anyhow!("no such user: {email_norm}"))?;
            let mut am: user::ActiveModel = u.into();
            am.role = Set("admin".to_string());
            am.updated_at = Set(chrono::Utc::now());
            am.update(&db).await?;
            println!("promoted to admin: {email_norm}");
        }
        "create" => {
            let password = arg("--password").expect("--password required");
            if password.len() < 8 {
                eprintln!("--password must be ≥ 8 chars");
                std::process::exit(2);
            }
            let existing = user::Entity::find()
                .filter(user::Column::Email.eq(&email_norm))
                .one(&db)
                .await?;
            if existing.is_some() {
                eprintln!("user already exists: {email_norm}");
                std::process::exit(1);
            }

            let salt = SaltString::generate(&mut OsRng);
            let phc = Argon2::default()
                .hash_password(password.as_bytes(), &salt)
                .map_err(|e| anyhow::anyhow!("hash: {e}"))?
                .to_string();

            let now = chrono::Utc::now();
            user::ActiveModel {
                id: Set(uuid::Uuid::now_v7()),
                email: Set(email_norm.clone()),
                password_hash: Set(phc),
                role: Set("admin".to_string()),
                email_verified_at: Set(Some(now)),
                session_version: Set(0),
                created_at: Set(now),
                updated_at: Set(now),
                totp_secret: Set(None),
                totp_enabled: Set(false),
            }
            .insert(&db)
            .await?;
            println!("created admin: {email_norm}");
        }
        _ => {
            eprintln!("usage: simu-admin <promote|create> --email <e> [--password <p>]");
            std::process::exit(2);
        }
    }
    Ok(())
}
