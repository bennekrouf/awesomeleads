use crate::config::Config;
use crate::database::StoredProject;
use crate::models::ContributorInfo;
use chrono::{DateTime, Utc};
use octocrab::Octocrab;
use std::collections::HashMap;
use tracing::{info, warn};

use super::{core::Result, project_handler::ProjectHandler};

#[derive(Debug, Clone)]
pub struct GitHubRepoAnalysis {
    pub owner: String,
    pub repo: String,
    pub repository_created: Option<String>,
    pub first_commit_date: Option<String>,
    pub last_commit_date: Option<String>,
    pub email: Option<String>,
    pub email_source: Option<String>,
    pub top_contributor_email: Option<String>,
    pub top_contributor_commits: Option<i32>,
    pub total_commits: Option<i32>,
    pub top_contributors: Vec<ContributorInfo>,
    pub meets_date_requirements: bool,
    pub skip_reason: Option<String>,
}

pub struct GitHubAnalyzer {
    client: Octocrab,
    config: Config,
}

impl GitHubAnalyzer {
    pub fn new(client: Octocrab, config: Config) -> Self {
        Self { client, config }
    }

    pub async fn fetch_and_update_data(
        &self,
        project: &StoredProject,
        project_handler: &ProjectHandler,
    ) -> Result<()> {
        if let (Some(owner), Some(repo)) = (&project.owner, &project.repo_name) {
            match self.analyze_repo(owner, repo).await {
                Ok(analysis) => {
                    project_handler
                        .update_project_with_github_data(project, analysis)
                        .await?;
                }
                Err(e) => {
                    warn!("Failed to fetch GitHub data for {}/{}: {}", owner, repo, e);
                }
            }
        }
        Ok(())
    }

    fn extract_contributor_stats(
        &self,
        contributors: &[ContributorInfo],
    ) -> (Option<String>, Option<i32>, Option<i32>) {
        if contributors.is_empty() {
            return (None, None, None);
        }

        let top_contributor = &contributors[0];
        let total_commits: i32 = contributors.iter().map(|c| c.commit_count).sum();

        (
            top_contributor.email.clone(),
            Some(top_contributor.commit_count),
            Some(total_commits),
        )
    }

    fn check_date_requirements(
        &self,
        repository_created: &Option<String>,
        first_commit_date: &Option<String>,
    ) -> (bool, Option<String>) {
        // Check repository creation date
        if let Some(ref created_date_str) = repository_created {
            if let Ok(created_datetime) = DateTime::parse_from_rfc3339(created_date_str) {
                let created_utc = created_datetime.with_timezone(&Utc);
                if created_utc < self.config.scraping.min_repository_created_date {
                    return (
                        false,
                        Some(format!(
                            "repository created {} (before {})",
                            created_utc.format("%Y-%m-%d"),
                            self.config
                                .scraping
                                .min_repository_created_date
                                .format("%Y-%m-%d")
                        )),
                    );
                }
            }
        }

        // Check first commit date
        if let Some(ref commit_date_str) = first_commit_date {
            if let Ok(commit_datetime) = DateTime::parse_from_rfc3339(commit_date_str) {
                let commit_utc = commit_datetime.with_timezone(&Utc);
                if commit_utc < self.config.scraping.min_first_commit_date {
                    return (
                        false,
                        Some(format!(
                            "first commit {} (before {})",
                            commit_utc.format("%Y-%m-%d"),
                            self.config
                                .scraping
                                .min_first_commit_date
                                .format("%Y-%m-%d")
                        )),
                    );
                }
            }
        }

        (true, None)
    }

    pub async fn analyze_repo(&self, owner: &str, repo: &str) -> Result<GitHubRepoAnalysis> {
        info!("🔍 Analyzing GitHub repository: {}/{}", owner, repo);

        // Get repository info for creation date
        info!("📅 Fetching repository creation date...");
        let repo_info = self.client.repos(owner, repo).get().await?;
        let repository_created = repo_info.created_at.map(|dt| dt.to_rfc3339());

        // Get commits ONCE and extract all data from them
        info!("📊 Fetching commits for comprehensive analysis...");
        let commits = match self
            .client
            .repos(owner, repo)
            .list_commits()
            .per_page(100)
            .page(1u32)
            .send()
            .await
        {
            Ok(commits) => commits.items,
            Err(e) => {
                warn!("⚠️  Could not fetch commits: {}", e);
                Vec::new()
            }
        };

        // Extract all data from the same commits
        let (first_commit_date, last_commit_date) = self.extract_commit_dates(&commits);
        let (email, email_source) = self.extract_email_from_commits(&commits);
        let (meets_requirements, skip_reason) =
            self.check_date_requirements(&repository_created, &first_commit_date);

        let contributors_data = if meets_requirements && !commits.is_empty() {
            info!("✅ Repository meets requirements, analyzing contributors...");
            self.analyze_contributors_from_commits(&commits).await
        } else {
            Vec::new()
        };

        let (top_contributor_email, top_contributor_commits, total_commits) =
            self.extract_contributor_stats(&contributors_data);

        Ok(GitHubRepoAnalysis {
            owner: owner.to_string(),
            repo: repo.to_string(),
            repository_created,
            first_commit_date,
            last_commit_date,
            email,
            email_source,
            top_contributor_email,
            top_contributor_commits,
            total_commits,
            top_contributors: contributors_data,
            meets_date_requirements: meets_requirements,
            skip_reason,
        })
    }

    // Add these helper methods:
    fn extract_commit_dates(
        &self,
        commits: &[octocrab::models::repos::RepoCommit],
    ) -> (Option<String>, Option<String>) {
        if commits.is_empty() {
            return (None, None);
        }

        let first_commit_date = commits
            .last()
            .and_then(|c| c.commit.author.as_ref())
            .and_then(|a| a.date.as_ref())
            .map(|d| d.to_rfc3339());

        let last_commit_date = commits
            .first()
            .and_then(|c| c.commit.author.as_ref())
            .and_then(|a| a.date.as_ref())
            .map(|d| d.to_rfc3339());

        (first_commit_date, last_commit_date)
    }

    fn extract_email_from_commits(
        &self,
        commits: &[octocrab::models::repos::RepoCommit],
    ) -> (Option<String>, Option<String>) {
        info!("🔍 Checking {} commits for emails...", commits.len());

        for (i, commit) in commits.iter().take(10).enumerate() {
            if let Some(author) = &commit.commit.author {
                let email = &author.email;
                info!("📧 Commit {}: found email '{}'", i + 1, email);
                if email.contains('@')
                    && !email.contains("noreply")
                    && !email.contains("users.noreply")
                {
                    info!("✅ Valid email found: {}", email);
                    return (Some(email.clone()), Some("commit_author".to_string()));
                } else {
                    info!("❌ Email rejected (noreply/invalid): {}", email);
                }
            } else {
                info!("❌ Commit {}: no author", i + 1);
            }
        }

        info!("❌ No valid emails found in commits");
        (None, Some("no_email_found".to_string()))
    }

    async fn analyze_contributors_from_commits(
        &self,
        commits: &[octocrab::models::repos::RepoCommit],
    ) -> Vec<ContributorInfo> {
        // Process the commits we already have instead of fetching more
        let mut contributor_map: HashMap<String, ContributorInfo> = HashMap::new();

        for commit in commits {
            if let Some(author) = &commit.commit.author {
                let email = author.email.clone();
                let name = author.name.clone();
                let commit_date = author.date.map(|d| d.to_rfc3339());

                let contributor = contributor_map
                    .entry(email.clone())
                    .or_insert(ContributorInfo {
                        email: Some(email),
                        name: Some(name),
                        commit_count: 0,
                        first_commit_date: commit_date.clone(),
                        last_commit_date: commit_date.clone(),
                    });

                contributor.commit_count += 1;
                // Update first/last commit dates as needed
            }
        }

        let mut contributors: Vec<ContributorInfo> = contributor_map.into_values().collect();
        contributors.sort_by(|a, b| b.commit_count.cmp(&a.commit_count));
        contributors.truncate(10);
        contributors
    }
}
