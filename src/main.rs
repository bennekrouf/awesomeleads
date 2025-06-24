// 4. UPDATE: src/main.rs - Add email_sender module
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

mod cli;
mod config;
mod database;
mod email_export;
mod email_sender; // NEW: Add this line
mod models;
mod scraper_util;
mod sources;

use config::{load_config, Config};
use database::create_db_pool;
use models::CliApp;
use tokio::signal;

// Use the correct Result type
type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    dotenv::dotenv().ok();

    // Load configuration
    let config = match load_config("config.yml").await {
        Ok(config) => config,
        Err(e) => {
            warn!("Failed to load config.yml: {}. Using defaults.", e);
            Config::default()
        }
    };

    // Setup enhanced logging with DEBUG level
    std::env::set_var("RUST_LOG", "lead_scraper=debug,hyper=warn,octocrab=warn");
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("lead_scraper=debug".parse().unwrap()),
        )
        .with_max_level(tracing::Level::DEBUG)
        .with_target(true)
        .with_line_number(true)
        .init();

    debug!("🚀 Application starting with debug logging enabled");
    debug!(
        "📁 Working directory: {:?}",
        std::env::current_dir().unwrap_or_default()
    );

    // Create output directory
    debug!("📁 Creating output directory: {}", config.output.directory);
    tokio::fs::create_dir_all(&config.output.directory)
        .await
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

    // Initialize database
    info!("Initializing database...");
    debug!("🗄️ About to create database pool...");

    let db_pool = create_db_pool("data/projects.db").await?;
    debug!("✅ Database pool created successfully");

    // Initialize and run CLI app
    debug!("🏗️ About to create CliApp...");
    let app = CliApp::new(config, db_pool).await?;
    debug!("✅ CliApp created successfully");

    debug!("🎯 About to run CliApp...");

    // Add graceful shutdown
    tokio::select! {
        result = app.run() => {
            result?;
            debug!("✅ Application completed successfully");
        }
        _ = signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down gracefully...");
        }
    }

    debug!("👋 Application shutdown complete");
    Ok(())
}
