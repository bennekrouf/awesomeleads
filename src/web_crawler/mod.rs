pub mod contact_extractor;
pub mod crawler;
pub mod types;
pub mod business_extractor;

// Re-export the main types for easy importing
// pub use contact_extractor::ContactExtractor;
pub use crawler::WebCrawler;
pub use types::{CrawlResult, CrawlConfig}; // ContactInfo, CrawledPage, PageMetadata
// pub use business_extractor::BusinessContactExtractor;
