// src/email_export/mod.rs
pub mod types;
pub mod database;
pub mod processor;
pub mod config;
pub mod exporter;

// Re-export main types for convenience
pub use types::{EmailExport};
pub use database::EmailDatabase;
pub use processor::EmailProcessor;
pub use config::EmailExportConfigBuilder;
pub use exporter::EmailExporter;

