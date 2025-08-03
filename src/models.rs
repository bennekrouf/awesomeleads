use serde::{Deserialize, Serialize};

use crate::{
    config::Config, database::DbPool, scraper_util::AwesomeScraper, sources::AwesomeSource,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectUrl {
    pub url: String,
    pub description: Option<String>,
    pub first_commit_date: Option<String>,
    pub repository_created: Option<String>,
    pub owner: Option<String>,
    pub repo_name: Option<String>,
    pub email: Option<String>,
    pub email_source: Option<String>,
    pub last_commit_date: Option<String>,      // NEW
    pub top_contributor_email: Option<String>, // NEW
    pub top_contributor_commits: Option<i32>,  // NEW
    pub total_commits: Option<i32>,
}

// Helper structs and methods
#[derive(Debug)]
pub struct ProjectFilter {
    pub description: String,
    pub sql_filter: String,
}

#[derive(Debug)]
pub struct Phase2Progress {
    pub complete: i64,
    pub partial: i64,
    pub untouched: i64,
    pub total: i64,
    pub completion_rate: f64,
}

pub struct CliApp {
    pub config: Config,
    pub db_pool: DbPool,
    pub scraper: AwesomeScraper,
    pub sources: Vec<Box<dyn AwesomeSource>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributorInfo {
    pub email: Option<String>,
    pub name: Option<String>,
    pub commit_count: i32,
    pub first_commit_date: Option<String>,
    pub last_commit_date: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScrapedData {
    pub repository: String,
    pub scraped_at: String,
    pub total_urls: usize,
    pub projects: Vec<ProjectUrl>,
}
