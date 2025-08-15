// src/main.rs - Complete version with server support
use tracing::{debug, info, warn};
use tracing_subscriber::EnvFilter;

mod api;
mod cli;
mod config;
mod database;
mod email_export;
mod email_rate_limiting;
mod email_sender;
mod models;
mod scraper_util;
mod server;
mod sources;
mod web_crawler;

use config::{load_config, Config};
use database::create_db_pool;
use models::CliApp;
use tokio::signal;

type AppResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> AppResult<()> {
    // Load .env FIRST, before anything else
    match dotenv::dotenv() {
        Ok(path) => {
            println!("✅ Loaded .env from: {:?}", path);

            // Verify critical debug variables are loaded
            if let Ok(debug_mode) = std::env::var("EMAIL_DEBUG_MODE") {
                println!("🐛 EMAIL_DEBUG_MODE loaded: {}", debug_mode);
            } else {
                println!("⚠️  EMAIL_DEBUG_MODE not found in environment");
            }

            if let Ok(debug_email) = std::env::var("EMAIL_DEBUG_ADDRESS") {
                println!("📧 EMAIL_DEBUG_ADDRESS loaded: {}", debug_email);
            } else {
                println!("⚠️  EMAIL_DEBUG_ADDRESS not found in environment");
            }
        }
        Err(e) => {
            println!("❌ Failed to load .env: {}", e);
            println!(
                "💡 Make sure .env file exists in: {:?}",
                std::env::current_dir().unwrap_or_default()
            );
        }
    }

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
        .with_max_level(tracing::Level::INFO)
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

    // Check for --server argument
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--server") {
        info!("🚀 Starting in server mode...");
        let rocket = server::build_rocket(config, db_pool);
        rocket
            .launch()
            .await
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?;
        return Ok(());
    }

    // Default CLI mode
    info!("🖥️  Starting in CLI mode...");
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

