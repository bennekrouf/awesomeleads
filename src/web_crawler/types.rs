// src/web_crawler/types.rs
use serde::{Deserialize, Serialize};
// use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawledPage {
    pub id: String,
    pub url: String,
    pub title: String,
    pub clean_text: String,
    pub metadata: PageMetadata,
    pub contacts: Vec<ContactInfo>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageMetadata {
    pub word_count: usize,
    pub has_contact_keywords: bool,
    pub page_type: String,
    pub links_count: usize,
    pub domain: String,
    pub is_contact_page: bool,
    pub is_about_page: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInfo {
    pub contact_type: ContactType,
    pub value: String,
    pub context: String,
    pub confidence: f32,
    pub source_url: String,
}

#[derive(Hash, Eq, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub enum ContactType {
    Email,
    Phone,
    LinkedIn,
    Twitter,
    Address,
    ContactForm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    pub original_url: String,
    pub pages_crawled: usize,
    pub contacts_found: usize,
    pub pages: Vec<CrawledPage>,
    pub best_contacts: Vec<ContactInfo>,
    pub crawl_duration_ms: u64,
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CrawlConfig {
    pub max_pages: u32,
    pub delay_ms: u64,
    pub timeout_seconds: u64,
    pub respect_robots: bool,
    pub contact_pages_only: bool,
    pub follow_external_links: bool,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            max_pages: 5,
            delay_ms: 2000,
            timeout_seconds: 60,
            respect_robots: true,
            contact_pages_only: false,
            follow_external_links: false,
        }
    }
}
