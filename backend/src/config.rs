use anyhow::Context;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub s3_endpoint: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub s3_bucket: String,
    #[allow(dead_code)]
    pub nats_url: Option<String>,
    pub session_secret: Vec<u8>,
    pub public_base_url: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let session_hex = std::env::var("SESSION_SECRET").context(
            "SESSION_SECRET required (hex-encoded ≥ 64 bytes). Generate with: openssl rand -hex 64",
        )?;
        let session_secret = hex::decode(&session_hex).context("SESSION_SECRET must be hex")?;
        anyhow::ensure!(
            session_secret.len() >= 64,
            "SESSION_SECRET must be ≥ 64 bytes"
        );

        Ok(Self {
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL required")?,
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            s3_endpoint: std::env::var("S3_ENDPOINT").context("S3_ENDPOINT required")?,
            s3_access_key: std::env::var("S3_ACCESS_KEY").context("S3_ACCESS_KEY required")?,
            s3_secret_key: std::env::var("S3_SECRET_KEY").context("S3_SECRET_KEY required")?,
            s3_bucket: std::env::var("S3_BUCKET").unwrap_or_else(|_| "simu-uploads".into()),
            nats_url: std::env::var("NATS_URL").ok(),
            session_secret,
            public_base_url: std::env::var("PUBLIC_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:3000".into()),
        })
    }
}
