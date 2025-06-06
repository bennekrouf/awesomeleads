// src/scraper/meta_source.rs - Meta-source processing (discovering awesome lists)
use crate::config::Config;
use crate::sources::AwesomeSource;
use octocrab::Octocrab;
use regex::Regex;
use std::collections::HashSet;
use tracing::{info, warn};

use super::core::Result;
use super::project_handler::ProjectHandler;
use super::utils::UrlUtils;

#[derive(Debug, Clone)]
pub struct DiscoveredSource {
    pub owner: String,
    pub repo: String,
    // pub description: String,
    // pub url: String,
}

#[derive(Debug, Clone)]
pub struct TempAwesomeSource {
    pub owner: String,
    pub repo: String,
    pub source_attribution: String,
}

impl AwesomeSource for TempAwesomeSource {
    fn name(&self) -> &str {
        &self.source_attribution
    }

    fn owner(&self) -> &str {
        &self.owner
    }

    fn repo(&self) -> &str {
        &self.repo
    }

    fn output_filename(&self) -> &str {
        "discovered_projects"
    }

    fn should_process_line(&self, line: &str) -> bool {
        !line.trim().is_empty() && !line.starts_with('#') && line.contains("](https://")
    }

    fn is_valid_project_url(&self, url: &str) -> bool {
        !url.contains("/wiki/")
            && !url.contains("/docs/")
            && !url.contains("/issues/")
            && !url.contains("/pull/")
    }

    fn is_meta_source(&self) -> bool {
        false
    }
}

pub struct MetaSourceProcessor {
    client: Octocrab,
    url_regex: Regex,
    // github_url_regex: Regex,
    config: Config,
    url_utils: UrlUtils,
}

impl MetaSourceProcessor {
    pub fn new(
        client: Octocrab,
        url_regex: Regex,
        github_url_regex: Regex,
        config: Config,
    ) -> Self {
        let url_utils = UrlUtils::new(github_url_regex.clone());
        Self {
            client,
            url_regex,
            // github_url_regex,
            config,
            url_utils,
        }
    }

    pub async fn process_meta_source(
        &self,
        source: &dyn AwesomeSource,
        project_handler: &ProjectHandler,
    ) -> Result<(usize, usize)> {
        info!("Processing meta-source: {}", source.name());
        let readme_content = self
            .fetch_readme_content(source.owner(), source.repo())
            .await?;

        let discovered_sources = self.discover_awesome_lists(&readme_content, source).await?;
        info!(
            "🎯 Discovered {} awesome lists from meta-source",
            discovered_sources.len()
        );

        let mut total_github_projects = 0;
        let mut total_non_github_projects = 0;

        for (i, discovered) in discovered_sources.iter().enumerate() {
            info!(
                "[{}/{}] Scraping discovered list: {}/{}",
                i + 1,
                discovered_sources.len(),
                discovered.owner,
                discovered.repo
            );

            let temp_source = self.create_temp_source(discovered);

            match self
                .scrape_discovered_awesome_list(&temp_source, project_handler)
                .await
            {
                Ok((github_count, non_github_count)) => {
                    total_github_projects += github_count;
                    total_non_github_projects += non_github_count;
                    info!(
                        "✓ {}/{} - Found {} GitHub + {} non-GitHub projects",
                        discovered.owner, discovered.repo, github_count, non_github_count
                    );
                }
                Err(e) => {
                    warn!(
                        "✗ Failed to scrape {}/{}: {}",
                        discovered.owner, discovered.repo, e
                    );
                }
            }

            // Rate limiting between discovered sources
            tokio::time::sleep(tokio::time::Duration::from_millis(
                self.config.scraping.rate_limit_delay_ms * 2,
            ))
            .await;
        }

        Ok((total_github_projects, total_non_github_projects))
    }

    async fn discover_awesome_lists(
        &self,
        readme_content: &str,
        source: &dyn AwesomeSource,
    ) -> Result<Vec<DiscoveredSource>> {
        let mut discovered_sources = Vec::new();
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

                    if let Some((url_owner, url_repo)) = self.url_utils.parse_github_url(&url_str) {
                        // Skip if this URL points to the source repository itself
                        if url_owner == source.owner() && url_repo == source.repo() {
                            continue;
                        }

                        // Check if it looks like an awesome list
                        if self.is_awesome_list(&url_str, desc_str, &url_repo) {
                            discovered_sources.push(DiscoveredSource {
                                owner: url_owner.clone(),
                                repo: url_repo.clone(),
                                // description: desc_str.to_string(),
                                // url: url_str,
                            });
                            info!(
                                "📋 Discovered awesome list: {}/{} - {}",
                                url_owner, url_repo, desc_str
                            );
                        }
                    }
                }
            }
        }

        Ok(discovered_sources)
    }

    async fn scrape_discovered_awesome_list(
        &self,
        temp_source: &TempAwesomeSource,
        project_handler: &ProjectHandler,
    ) -> Result<(usize, usize)> {
        let readme_content = match self
            .fetch_readme_content(&temp_source.owner, &temp_source.repo)
            .await
        {
            Ok(content) => content,
            Err(e) => {
                warn!(
                    "Failed to fetch README for {}/{}: {}",
                    temp_source.owner, temp_source.repo, e
                );
                return Ok((0, 0));
            }
        };

        let mut new_github_projects = 0;
        let mut new_non_github_projects = 0;
        let mut seen_urls = HashSet::new();

        for line in readme_content.lines() {
            if !temp_source.should_process_line(line) {
                continue;
            }

            for caps in self.url_regex.captures_iter(line) {
                if let (Some(desc), Some(url)) = (caps.get(1), caps.get(2)) {
                    let url_str = url.as_str().to_string();
                    let desc_str = desc.as_str().trim();

                    if !temp_source.is_valid_project_url(&url_str)
                        || !seen_urls.insert(url_str.clone())
                    {
                        continue;
                    }

                    // Check if it's a GitHub URL
                    if let Some((url_owner, url_repo)) = self.url_utils.parse_github_url(&url_str) {
                        // Skip if this URL points to the source repository itself
                        if url_owner == temp_source.owner && url_repo == temp_source.repo {
                            continue;
                        }

                        // Skip other awesome lists to avoid infinite recursion
                        if self.is_awesome_list(&url_str, desc_str, &url_repo) {
                            continue;
                        }

                        // Handle GitHub project
                        if let Err(e) = project_handler
                            .handle_github_project(
                                &url_str,
                                desc_str,
                                &url_owner,
                                &url_repo,
                                temp_source,
                            )
                            .await
                        {
                            warn!("Failed to handle GitHub project {}: {}", url_str, e);
                            continue;
                        }

                        new_github_projects += 1;
                    } else {
                        // Handle non-GitHub project
                        if let Err(e) = project_handler
                            .handle_non_github_project(&url_str, desc_str, temp_source)
                            .await
                        {
                            warn!("Failed to handle non-GitHub project {}: {}", url_str, e);
                            continue;
                        }

                        new_non_github_projects += 1;
                    }
                }
            }
        }

        Ok((new_github_projects, new_non_github_projects))
    }

    fn is_awesome_list(&self, url: &str, description: &str, repo_name: &str) -> bool {
        let desc_lower = description.to_lowercase();
        let repo_lower = repo_name.to_lowercase();
        let url_lower = url.to_lowercase();

        repo_lower.contains("awesome")
            || desc_lower.contains("awesome")
            || url_lower.contains("awesome")
            || repo_lower.ends_with("-list")
            || desc_lower.contains("curated list")
            || desc_lower.contains("collection of")
    }

    fn create_temp_source(&self, discovered: &DiscoveredSource) -> TempAwesomeSource {
        TempAwesomeSource {
            owner: discovered.owner.clone(),
            repo: discovered.repo.clone(),
            source_attribution: format!(
                "sindresorhus/awesome -> {}/{}",
                discovered.owner, discovered.repo
            ),
        }
    }

    async fn fetch_readme_content(&self, owner: &str, repo: &str) -> Result<String> {
        use base64::{engine::general_purpose, Engine as _};

        info!("Fetching README for {}/{}", owner, repo);
        let repo_handler = self.client.repos(owner, repo);

        let readme_files = [
            "README.md",
            "readme.md",
            "README",
            "readme",
            "README.rst",
            "readme.rst",
        ];

        for filename in readme_files {
            match repo_handler.get_content().path(filename).send().await {
                Ok(content) => {
                    if let Some(file) = content.items.first() {
                        if let Some(content_str) = &file.content {
                            let decoded =
                                general_purpose::STANDARD.decode(content_str.replace('\n', ""))?;
                            let text = String::from_utf8(decoded)?;
                            info!(
                                "✅ Successfully fetched {} from {}/{}",
                                filename, owner, repo
                            );
                            return Ok(text);
                        }
                    }
                }
                Err(e) => {
                    if filename == "README.md" {
                        // Only log on first attempt
                        match e {
                            octocrab::Error::GitHub { source, .. } if source.status_code == 404 => {
                                warn!("⚠️  Repository {}/{} not found or private", owner, repo);
                            }
                            _ => {
                                warn!("⚠️  Failed to fetch from {}/{}: {}", owner, repo, e);
                            }
                        }
                    }
                    continue;
                }
            }
        }

        Err(format!("No README found for {}/{}", owner, repo).into())
    }
}
