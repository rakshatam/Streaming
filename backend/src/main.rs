use axum::http::{header, Method};
use std::sync::Arc;
use tower_http::{
    compression::CompressionLayer, cors::CorsLayer, set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use streamflow_backend::{api, config::Config, db, stream::StreamEngine, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,streamflow_backend=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting StreamFlow video server...");

    let config = Arc::new(Config::from_env()?);

    // Initialize SQLite database
    let db = db::init_db(&config.database_url).await?;

    // Initialize streaming engine with segment cache
    let stream_engine = StreamEngine::new(
        config.hmac_secret.clone(),
        config.segment_cache_mb,
        config.token_validity_secs,
    );

    let state = AppState {
        db,
        config: config.clone(),
        stream_engine,
    };

    // Restrict browser access to the explicitly configured frontend origins.
    let allowed_origins = config
        .cors_allowed_origins
        .iter()
        .map(|origin| origin.parse::<header::HeaderValue>())
        .collect::<Result<Vec<_>, _>>()?;
    let cors = CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::ACCEPT,
            header::CONTENT_TYPE,
            header::RANGE,
            header::ORIGIN,
        ])
        .expose_headers([
            header::CONTENT_RANGE,
            header::ACCEPT_RANGES,
            header::CONTENT_LENGTH,
            header::CONTENT_TYPE,
            header::HeaderName::from_static("x-cache"),
        ]);

    // Assemble router with middleware
    let app = api::create_router()
        .layer(cors)
        .layer(CompressionLayer::new())
        // Stream signatures are query parameters; log only the path so signed
        // URLs do not end up in request logs.
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<_>| {
                tracing::info_span!(
                    "http_request",
                    method = %request.method(),
                    path = %request.uri().path()
                )
            }),
        )
        // Standard security headers
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("x-content-type-options"),
            header::HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("x-frame-options"),
            header::HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::HeaderName::from_static("referrer-policy"),
            header::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .with_state(state);

    let host = if config.server_host.contains(':') && !config.server_host.starts_with('[') {
        format!("[{}]", config.server_host)
    } else {
        config.server_host.clone()
    };
    let bind_addr = format!("{}:{}", host, config.server_port);
    info!("StreamFlow API server listening on http://{}", bind_addr);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Graceful shutdown signal received. Releasing resources...");
}
