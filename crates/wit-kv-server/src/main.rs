//! wit-kv HTTP API server.

mod server;

use axum::Router;
use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use tokio::signal;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use server::{AppState, Config, CorsConfig, init_logging, router};

/// wit-kv HTTP API server.
#[derive(Parser, Debug)]
#[command(name = "wit-kv-server")]
#[command(about = "HTTP API server for wit-kv typed key-value store")]
struct Args {
    /// Path to the configuration file.
    #[arg(short, long, default_value = "wit-kv-server.toml")]
    config: PathBuf,
}

/// Build CORS layer from configuration.
fn build_cors_layer(config: &CorsConfig) -> CorsLayer {
    if !config.enabled {
        // Deny all cross-origin requests by default
        return CorsLayer::new();
    }

    let mut cors = CorsLayer::new();

    // Configure allowed origins
    if config.allow_origins.iter().any(|o| o == "*") {
        cors = cors.allow_origin(Any);
    } else {
        let origins: Vec<_> = config
            .allow_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        cors = cors.allow_origin(origins);
    }

    // Configure allowed methods
    let methods: Vec<_> = config
        .allow_methods
        .iter()
        .filter_map(|m| m.parse().ok())
        .collect();
    cors = cors.allow_methods(methods);

    // Configure allowed headers
    let headers: Vec<_> = config
        .allow_headers
        .iter()
        .filter_map(|h| h.parse().ok())
        .collect();
    cors = cors.allow_headers(headers);

    // Configure credentials
    if config.allow_credentials {
        cors = cors.allow_credentials(true);
    }

    // Configure max age
    cors = cors.max_age(Duration::from_secs(config.max_age));

    cors
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Load configuration first (needed for logging setup)
    let config = Config::from_file(&args.config)?;
    let bind_addr = config.bind_addr();

    // Initialize logging from config
    init_logging(&config.logging)?;

    tracing::info!("Loading databases...");
    for db in &config.databases {
        tracing::info!("  {} -> {}", db.name, db.path);
    }

    // Create application state
    let state = AppState::from_config(&config)?;

    // Build router with API routes
    let mut app = router(state);

    // Add static file serving if configured
    if let Some(static_path) = &config.server.static_path {
        tracing::info!("Serving static files from: {}", static_path);
        app = app.fallback_service(ServeDir::new(static_path));
    }

    // Apply CORS layer
    let cors = build_cors_layer(&config.cors);
    if config.cors.enabled {
        tracing::info!(
            "CORS enabled with {} allowed origin(s)",
            config.cors.allow_origins.len()
        );
    } else {
        tracing::info!("CORS disabled (denying cross-origin requests)");
    }

    // Apply middleware layers
    let app: Router = app.layer(cors).layer(TraceLayer::new_for_http());

    // Parse bind address
    let addr: SocketAddr = bind_addr.parse()?;

    tracing::info!("Starting server on {}", addr);

    // Create the listener
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Run server with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shutdown complete");

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = signal::ctrl_c().await {
            tracing::error!("Failed to install Ctrl+C handler: {}", e);
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(e) => {
                tracing::error!("Failed to install signal handler: {}", e);
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received");
}
