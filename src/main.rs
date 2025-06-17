// src/main.rs (UPDATED - add the email_export module)
use models::{CliApp, Result};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

mod cli;
mod config;
mod database;
mod email_export;  // ← Add this new module
mod models;
mod scraper_util;
mod sources;

use config::{load_config, Config};
use database::create_db_pool;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    // Load configuration
    let config = match load_config("config.yml").await {
        Ok(config) => config,
        Err(e) => {
            warn!("Failed to load config.yml: {}. Using defaults.", e);
            Config::default()
        }
    };

    // Setup logging
    std::env::set_var("RUST_LOG", "lead_scraper=info,hyper=warn,octocrab=warn");
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("lead_scraper=info".parse().unwrap()),
        )
        .with_max_level(tracing::Level::INFO)
        .init();

    // Create output directory
    tokio::fs::create_dir_all(&config.output.directory).await?;

    // Initialize database
    info!("Initializing database...");
    let db_pool = create_db_pool("data/projects.db").await?;

    // Initialize and run CLI app
    let app = CliApp::new(config, db_pool).await?;

    // Add graceful shutdown
    tokio::select! {
        result = app.run() => {
            result?;
        }
        _ = signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down gracefully...");
        }
    }

    Ok(())
}
