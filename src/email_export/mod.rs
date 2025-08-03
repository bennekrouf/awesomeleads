// src/email_export/mod.rs
pub mod config;
pub mod database;
pub mod exporter;
pub mod processor;
pub mod types;

// Re-export main types for convenience
pub use config::EmailExportConfigBuilder;
pub use database::EmailDatabase;
pub use exporter::EmailExporter;
pub use processor::EmailProcessor;
pub use types::EmailExport;
