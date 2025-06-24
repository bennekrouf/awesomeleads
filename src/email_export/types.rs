// src/email_export/types.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscriptionStatus {
    #[serde(rename = "subscribed")]
    Subscribed,
    #[serde(rename = "unsubscribed")]
    Unsubscribed,
    #[serde(rename = "never_subscribed")]
    NeverSubscribed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainCategory {
    #[serde(rename = "web3")]
    Web3,
    #[serde(rename = "ai")]
    AI,
    #[serde(rename = "fintech")]
    Fintech,
    #[serde(rename = "saas")]
    SaaS,
    #[serde(rename = "enterprise")]
    Enterprise,
    #[serde(rename = "other")]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompanySize {
    #[serde(rename = "startup")]
    Startup,
    #[serde(rename = "scale-up")]
    ScaleUp,
    #[serde(rename = "enterprise")]
    Enterprise,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailExport {
    pub email: String,
    pub name: Option<String>,
    pub first_name: Option<String>,
    pub status: String,
    pub consent_timestamp: String,
    pub source: String,
    pub domain_category: String,
    pub tags: String,
    pub company_size: String,
    pub industry: String,
    pub engagement_score: u8,
    pub project_url: String,
    pub repository_created: String,
    pub commit_count: String,
}

#[derive(Debug, Clone)]
pub struct RawEmailData {
    pub email: String,
    pub name: Option<String>,
    pub url: String,
    pub description: Option<String>,
    pub repository_created: Option<String>,
    pub total_commits: Option<i32>,
    pub owner: Option<String>,
    pub source_repository: String,
}

#[derive(Debug)]
pub struct ExportConfig {
    pub title: String,
    pub sql_filter: String,
    pub min_engagement_score: u8,
}

#[derive(Debug, Clone)]
pub struct ExportStats {
    pub total_emails: usize,
    pub by_category: std::collections::HashMap<String, usize>,
    pub by_company_size: std::collections::HashMap<String, usize>,
    pub average_engagement: f64,
}
