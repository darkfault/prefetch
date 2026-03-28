//! Daemon mode with filesystem watching and usage prediction.
//!
//! This crate will be expanded in Phase 3 to include:
//! - Filesystem watching for new/changed GGUF files
//! - Unix domain socket IPC for CLI communication
//! - Usage history tracking in SQLite
//! - Time-based model prediction and proactive warming

pub mod security;

/// Placeholder for daemon service loop.
/// Will be implemented in Phase 3.
pub async fn run_daemon(_config: prefetch_config::AppConfig) -> anyhow::Result<()> {
    tracing::info!("daemon mode is not yet implemented — coming in Phase 3");
    tracing::info!("for now, use `prefetch warm <model>` to manually warm models");

    // Keep running until interrupted
    tokio::signal::ctrl_c().await?;
    tracing::info!("shutting down");
    Ok(())
}
