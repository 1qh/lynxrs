use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use object_store::ObjectStore;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

use crate::{events::EventBus, mailer::Mailer};

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub storage: Arc<dyn ObjectStore>,
    pub signer: Arc<object_store::aws::AmazonS3>,
    pub bucket: String,
    pub cookie_key: Key,
    pub bus: EventBus,
    pub mailer: Mailer,
    pub public_base_url: String,
}

impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}
