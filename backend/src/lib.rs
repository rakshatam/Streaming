pub mod api;
pub mod config;
pub mod db;
pub mod email;
pub mod security;
pub mod stream;

use crate::{config::Config, db::DbPool, stream::StreamEngine};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub config: Arc<Config>,
    pub stream_engine: StreamEngine,
}
