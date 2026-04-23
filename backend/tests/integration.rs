#![allow(dead_code)]
// Integration tests consume the library crate directly (no #[path] hack).
// Spin up real Postgres + MinIO via testcontainers; spawn the app in-process.

use std::{net::SocketAddr, sync::Arc};

use axum_extra::extract::cookie::Key;
use object_store::aws::AmazonS3Builder;
use reqwest::{Client, StatusCode};
use sea_orm::{ConnectOptions, Database};
use sea_orm_migration::MigratorTrait;
use testcontainers::ContainerAsync;
use testcontainers::runners::AsyncRunner;
use testcontainers_modules::{minio::MinIO, postgres::Postgres};
use tokio::net::TcpListener;

use simu_backend::{
    api::{self, BuildOpts},
    events, mailer,
    migration::Migrator,
    state::AppState,
};

struct App {
    base: String,
    client: Client,
    _pg: ContainerAsync<Postgres>,
    _s3: ContainerAsync<MinIO>,
}

async fn spawn_app() -> App {
    let pg = Postgres::default()
        .start()
        .await
        .expect("start postgres");
    let pg_host = pg.get_host().await.expect("pg host");
    let pg_port = pg.get_host_port_ipv4(5432).await.expect("pg port");
    let database_url = format!("postgres://postgres:postgres@{pg_host}:{pg_port}/postgres");

    let s3 = MinIO::default().start().await.expect("start minio");
    let s3_host = s3.get_host().await.expect("s3 host");
    let s3_port = s3.get_host_port_ipv4(9000).await.expect("s3 port");
    let s3_endpoint = format!("http://{s3_host}:{s3_port}");

    let mut opts = ConnectOptions::new(&database_url);
    opts.max_connections(5)
        .connect_timeout(std::time::Duration::from_secs(30))
        .acquire_timeout(std::time::Duration::from_secs(30));
    let db = loop {
        match Database::connect(opts.clone()).await {
            Ok(conn) => break conn,
            Err(_) => tokio::time::sleep(std::time::Duration::from_millis(300)).await,
        }
    };
    Migrator::up(&db, None).await.expect("migrate");

    let bucket = "test-bucket";
    let storage: Arc<dyn object_store::ObjectStore> = Arc::new(
        AmazonS3Builder::new()
            .with_endpoint(&s3_endpoint)
            .with_access_key_id("minioadmin")
            .with_secret_access_key("minioadmin")
            .with_bucket_name(bucket)
            .with_region("us-east-1")
            .with_allow_http(true)
            .build()
            .expect("s3 build"),
    );

    let state = AppState {
        db,
        storage,
        bucket: bucket.to_string(),
        cookie_key: Key::generate(),
        bus: events::new_bus(16),
        // Mailpit not present in testcontainers run; send_password_reset logs but doesn't fail the test.
        mailer: mailer::Mailer::from_env().expect("mailer"),
        public_base_url: "http://localhost".to_string(),
    };

    let app = api::build(
        state,
        BuildOpts {
            production_layers: false,
            ..Default::default()
        },
    );

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .expect("client");
    App {
        base: format!("http://{addr}"),
        client,
        _pg: pg,
        _s3: s3,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn duplicate_signup_returns_409() {
    let app = spawn_app().await;
    let email = format!(
        "dup-{}@example.com",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    for (i, expected) in [StatusCode::CREATED, StatusCode::CONFLICT]
        .iter()
        .enumerate()
    {
        let res = app
            .client
            .post(format!("{}/api/auth/signup", app.base))
            .json(&serde_json::json!({ "email": email, "password": "hunter2hunter2" }))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), *expected, "iteration {i}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_password_rejected() {
    let app = spawn_app().await;
    let email = format!(
        "bp-{}@example.com",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    app.client
        .post(format!("{}/api/auth/signup", app.base))
        .json(&serde_json::json!({ "email": email, "password": "hunter2hunter2" }))
        .send()
        .await
        .unwrap();

    let fresh = reqwest::Client::builder()
        .cookie_store(true)
        .build()
        .unwrap();
    let res = fresh
        .post(format!("{}/api/auth/login", app.base))
        .json(&serde_json::json!({ "email": email, "password": "wrongwrongwrong" }))
        .send()
        .await
        .unwrap();
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    assert_eq!(status, StatusCode::UNAUTHORIZED, "body: {body}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn email_is_case_insensitive() {
    let app = spawn_app().await;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let upper = format!("Case{nonce}@Example.COM");
    let lower = upper.to_lowercase();

    let r1 = app
        .client
        .post(format!("{}/api/auth/signup", app.base))
        .json(&serde_json::json!({ "email": upper, "password": "hunter2hunter2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r1.status(), StatusCode::CREATED);

    let r2 = app
        .client
        .post(format!("{}/api/auth/signup", app.base))
        .json(&serde_json::json!({ "email": lower, "password": "hunter2hunter2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r2.status(), StatusCode::CONFLICT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs AWS SigV4 bucket-create helper; covered by Playwright against compose stack"]
async fn signup_login_upload_list_round_trip() {
    // Kept as a marker for the full flow; reactivate once we add a pure-Rust SigV4 CreateBucket.
    let _ = spawn_app().await;
}
