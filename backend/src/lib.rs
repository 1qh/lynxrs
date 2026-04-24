//! Library crate for simu-backend. The `main.rs` bin thinly boots this.
//! Integration tests consume `simu_backend::app` directly instead of #[path] re-declaring modules.

pub mod admin;
pub mod audit;
pub mod auth;
pub mod config;
pub mod entity;
pub mod error;
pub mod events;
pub mod files;
pub mod housekeeping;
pub mod mailer;
pub mod migration;
pub mod state;
pub mod telemetry;
pub mod tokens;
pub mod api;
