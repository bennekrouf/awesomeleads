// src/api/mod.rs
pub mod companies;
pub mod crawl;
pub mod leads;
pub mod projects;
pub mod sources;
pub mod stats;

// Re-export all route functions
pub use companies::*;
pub use crawl::*;
pub use leads::*;
pub use projects::*;
pub use sources::*;
pub use stats::*;
