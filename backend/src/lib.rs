pub mod api;
pub mod config;
pub mod db;
pub mod security;
pub mod stream;

use std::sync::Arc;
use crate::{config::Config, db::DbPool, stream::StreamEngine};

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub config: Arc<Config>,
    pub stream_engine: StreamEngine,
}
