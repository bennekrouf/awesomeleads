// src/scraper/core.rs - Main scraper structure and coordination
use crate::config::Config;
use crate::database::DbPool;
use crate::models::ScrapedData;
use crate::sources::AwesomeSource;
use octocrab::Octocrab;
use regex::Regex;
use std::collections::HashSet;
use tracing::{info, warn};

use super::github::{GitHubAnalyzer, GitHubRepoAnalysis};
use super::meta_source::MetaSourceProcessor;
use super::project_handler::ProjectHandler;
use super::utils::UrlUtils;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct AwesomeScraper {
    pub client: Octocrab,
    pub url_regex: Regex,
    // pub github_url_regex: Regex,
    pub config: Config,
    // pub db_pool: DbPool,

    // Specialized processors
    github_analyzer: GitHubAnalyzer,
    meta_processor: MetaSourceProcessor,
    project_handler: ProjectHandler,
    url_utils: UrlUtils,
}

impl AwesomeScraper {
    pub async fn new(config: Config, db_pool: DbPool) -> Result<Self> {
        let client = match std::env::var("GITHUB_TOKEN") {
            Ok(token) => Octocrab::builder().personal_token(token).build()?,
            Err(_) => {
                warn!("No GITHUB_TOKEN found, using unauthenticated client");
                Octocrab::builder().build()?
            }
        };

        let url_regex = regex::Regex::new(r"\[([^\]]+)\]\((https://[^\)]+)\)")?;
        let github_url_regex = regex::Regex::new(r"https://github\.com/([^/]+)/([^/?#]+)")?;

        // Initialize specialized components
        let github_analyzer = GitHubAnalyzer::new(client.clone(), config.clone());
        let meta_processor = MetaSourceProcessor::new(
            client.clone(),
            url_regex.clone(),
            github_url_regex.clone(),
            config.clone(),
        );
        let project_handler = ProjectHandler::new(db_pool.clone());
        let url_utils = UrlUtils::new(github_url_regex.clone());

        Ok(Self {
            client,
            url_regex,
            // github_url_regex,
            config,
            // db_pool,
            github_analyzer,
            meta_processor,
            project_handler,
            url_utils,
        })
    }

    pub async fn scrape_source_urls(&self, source: &dyn AwesomeSource) -> Result<(usize, usize)> {
        info!("Scraping URLs from {}", source.name());

        if source.is_meta_source() {
            return self
                .meta_processor
                .process_meta_source(source, &self.project_handler)
                .await;
        }

        self.scrape_regular_source(source).await
    }

    async fn scrape_regular_source(&self, source: &dyn AwesomeSource) -> Result<(usize, usize)> {
        let readme_content = self
            .fetch_readme_content(source.owner(), source.repo())
            .await?;

        let mut new_github_projects = 0;
        let mut new_non_github_projects = 0;
        let mut seen_urls = HashSet::new();

        for line in readme_content.lines() {
            if !source.should_process_line(line) {
                continue;
            }

            for caps in self.url_regex.captures_iter(line) {
                if let (Some(desc), Some(url)) = (caps.get(1), caps.get(2)) {
                    let url_str = url.as_str().to_string();
                    let desc_str = desc.as_str().trim();

                    if !source.is_valid_project_url(&url_str) || !seen_urls.insert(url_str.clone())
                    {
                        continue;
                    }

                    info!("Valid URL found: {}", url_str);

                    // Check if it's a GitHub URL
                    if let Some((owner, repo)) = self.url_utils.parse_github_url(&url_str) {
                        // Skip if this URL points to the source repository itself
                        if owner == source.owner() && repo == source.repo() {
                            info!("Skipping source repository itself: {}", url_str);
                            continue;
                        }

                        // Handle GitHub project
                        if let Err(e) = self
                            .project_handler
                            .handle_github_project(&url_str, desc_str, &owner, &repo, source)
                            .await
                        {
                            warn!("Failed to handle GitHub project {}: {}", url_str, e);
                            continue;
                        }

                        new_github_projects += 1;
                        info!("✓ Stored GitHub project: {}", url_str);
                    } else {
                        // Handle non-GitHub project
                        if let Err(e) = self
                            .project_handler
                            .handle_non_github_project(&url_str, desc_str, source)
                            .await
                        {
                            warn!("Failed to handle non-GitHub project {}: {}", url_str, e);
                            continue;
                        }

                        new_non_github_projects += 1;
                        info!("✓ Stored non-GitHub project: {}", url_str);
                    }
                }
            }
        }

        info!("Scraping summary for {}:", source.name());
        info!("  GitHub projects added: {}", new_github_projects);
        info!("  Non-GitHub projects added: {}", new_non_github_projects);

        Ok((new_github_projects, new_non_github_projects))
    }

    // Delegate to GitHub analyzer
    pub async fn analyze_github_repo(&self, owner: &str, repo: &str) -> Result<GitHubRepoAnalysis> {
        self.github_analyzer.analyze_repo(owner, repo).await
    }

    pub fn parse_github_url(&self, url: &str) -> Result<(String, String)> {
        self.url_utils.parse_github_url_result(url)
    }

    pub async fn fetch_and_update_github_data(
        &self,
        project: &crate::database::StoredProject,
    ) -> Result<()> {
        self.github_analyzer
            .fetch_and_update_data(project, &self.project_handler)
            .await
    }

    pub async fn export_source_data(&self, source: &dyn AwesomeSource) -> Result<ScrapedData> {
        self.project_handler.export_source_data(source).await
    }

    pub async fn save_to_json(&self, data: &ScrapedData, filename: &str) -> Result<()> {
        let json = if self.config.output.pretty_json {
            serde_json::to_string_pretty(data)?
        } else {
            serde_json::to_string(data)?
        };
        tokio::fs::write(filename, json).await?;
        Ok(())
    }

    async fn fetch_readme_content(&self, owner: &str, repo: &str) -> Result<String> {
        use base64::{engine::general_purpose, Engine as _};
        use tracing::error;

        info!("Fetching README for {}/{}", owner, repo);
        let repo_handler = self.client.repos(owner, repo);

        let readme_files = ["README.md", "readme.md", "README", "readme"];

        for filename in readme_files {
            info!("Trying to fetch {}", filename);
            match repo_handler.get_content().path(filename).send().await {
                Ok(content) => {
                    info!(
                        "Successfully fetched {}, items count: {}",
                        filename,
                        content.items.len()
                    );
                    if let Some(file) = content.items.first() {
                        if let Some(content_str) = &file.content {
                            info!("Content found, decoding base64...");
                            let decoded =
                                general_purpose::STANDARD.decode(content_str.replace('\n', ""))?;
                            let text = String::from_utf8(decoded)?;
                            info!("README decoded successfully, length: {} chars", text.len());
                            return Ok(text);
                        } else {
                            warn!("File {} has no content field", filename);
                        }
                    } else {
                        warn!("No items returned for {}", filename);
                    }
                }
                Err(e) => {
                    error!("Failed to fetch {}: {:?}", filename, e);
                    continue;
                }
            }
        }

        Err("No README file found".into())
    }
}
