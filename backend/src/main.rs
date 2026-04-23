use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum_extra::extract::cookie::Key;
use object_store::aws::AmazonS3Builder;
use sea_orm::{ConnectOptions, Database};
use sea_orm_migration::MigratorTrait;
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use simu_backend::{
    api::{self, BuildOpts},
    config::Config,
    events, housekeeping, mailer,
    migration::Migrator,
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,simu_backend=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    let cfg = Config::from_env()?;

    let mut opts = ConnectOptions::new(cfg.database_url.clone());
    opts.max_connections(20)
        .connect_timeout(Duration::from_secs(10));
    let db = Database::connect(opts).await?;
    Migrator::up(&db, None).await?;

    let storage: Arc<dyn object_store::ObjectStore> = Arc::new(
        AmazonS3Builder::new()
            .with_endpoint(&cfg.s3_endpoint)
            .with_access_key_id(&cfg.s3_access_key)
            .with_secret_access_key(&cfg.s3_secret_key)
            .with_bucket_name(&cfg.s3_bucket)
            .with_region("us-east-1")
            .with_allow_http(true)
            .build()?,
    );

    let bus = events::new_bus(1024);
    let mailer = mailer::Mailer::from_env()?;

    let state = AppState {
        db: db.clone(),
        storage,
        bucket: cfg.s3_bucket.clone(),
        cookie_key: Key::from(&cfg.session_secret),
        bus,
        mailer,
        public_base_url: cfg.public_base_url.clone(),
    };

    // Background housekeeping: sweep expired auth tokens every 5 minutes.
    housekeeping::spawn(db, Duration::from_secs(300));

    let rate_limit_rps: u64 = std::env::var("RATE_LIMIT_RPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let rate_limit_burst: u32 = std::env::var("RATE_LIMIT_BURST")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    let app = api::build(
        state,
        BuildOpts {
            rate_limit_rps,
            rate_limit_burst,
            production_layers: true,
        },
    );

    let addr: SocketAddr = cfg.bind_addr.parse()?;
    tracing::info!(%addr, "simu-backend listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { tracing::info!("received ctrl-c; draining"); }
        _ = terminate => { tracing::info!("received SIGTERM; draining"); }
    }
}
