//! wit-kv HTTP API server.

use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::signal;
use tower_http::trace::TraceLayer;

use wit_kv::server::{router, AppState, Config};

/// wit-kv HTTP API server.
#[derive(Parser, Debug)]
#[command(name = "wit-kv-server")]
#[command(about = "HTTP API server for wit-kv typed key-value store")]
struct Args {
    /// Path to the configuration file.
    #[arg(short, long, default_value = "wit-kv-server.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    // Load configuration
    let config = Config::from_file(&args.config)?;
    let bind_addr = config.bind_addr();

    tracing::info!("Loading databases...");
    for db in &config.databases {
        tracing::info!("  {} -> {}", db.name, db.path);
    }

    // Create application state
    let state = AppState::from_config(&config)?;

    // Build router
    let app = router(state).layer(TraceLayer::new_for_http());

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
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
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

    tracing::info!("Shutdown signal received");
}
