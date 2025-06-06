use crate::database::{
    get_non_github_project_by_url, get_project_by_url, upsert_contributors,
    upsert_non_github_project, upsert_project, DbPool, StoredNonGithubProject, StoredProject,
};
use crate::models::{ProjectUrl, ScrapedData};
use crate::sources::AwesomeSource;
use chrono::Utc;
use tracing::info;

use super::core::Result;
use super::github::GitHubRepoAnalysis;

pub struct ProjectHandler {
    db_pool: DbPool,
}

impl ProjectHandler {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub async fn handle_github_project(
        &self,
        url: &str,
        description: &str,
        owner: &str,
        repo: &str,
        source: &dyn AwesomeSource,
    ) -> Result<()> {
        // Check if we already have this GitHub project
        if get_project_by_url(&self.db_pool, url).await?.is_some() {
            info!("GitHub project already exists in DB: {}", url);
            return Ok(());
        }

        let project = StoredProject {
            id: None,
            url: url.to_string(),
            description: if description.is_empty() {
                None
            } else {
                Some(description.to_string())
            },
            owner: Some(owner.to_string()),
            repo_name: Some(repo.to_string()),
            repository_created: None,
            first_commit_date: None,
            last_commit_date: None,
            email: None,
            email_source: None,
            top_contributor_email: None,
            top_contributor_commits: None,
            total_commits: None,
            source_repository: format!("{}/{}", source.owner(), source.repo()),
            scraped_at: Utc::now(),
            last_updated: Utc::now(),
        };

        upsert_project(&self.db_pool, &project).await?;
        Ok(())
    }

    pub async fn handle_non_github_project(
        &self,
        url: &str,
        description: &str,
        source: &dyn AwesomeSource,
    ) -> Result<()> {
        // Check if we already have this non-GitHub project
        if get_non_github_project_by_url(&self.db_pool, url)
            .await?
            .is_some()
        {
            info!("Non-GitHub project already exists in DB: {}", url);
            return Ok(());
        }

        // Extract domain and determine project type
        let domain = self.extract_domain(url);
        let project_type = self.determine_project_type(url, description);

        let project = StoredNonGithubProject {
            id: None,
            url: url.to_string(),
            description: if description.is_empty() {
                None
            } else {
                Some(description.to_string())
            },
            domain: Some(domain),
            project_type: Some(project_type),
            source_repository: format!("{}/{}", source.owner(), source.repo()),
            scraped_at: Utc::now(),
            last_updated: Utc::now(),
        };

        upsert_non_github_project(&self.db_pool, &project).await?;
        Ok(())
    }

    pub async fn update_project_with_github_data(
        &self,
        project: &StoredProject,
        analysis: GitHubRepoAnalysis,
    ) -> Result<()> {
        let updated_project = StoredProject {
            id: project.id,
            url: project.url.clone(),
            description: project.description.clone(),
            owner: project.owner.clone(),
            repo_name: project.repo_name.clone(),
            repository_created: analysis.repository_created,
            first_commit_date: analysis.first_commit_date,
            last_commit_date: analysis.last_commit_date,
            email: analysis.email,
            email_source: analysis.email_source,
            top_contributor_email: analysis.top_contributor_email,
            top_contributor_commits: analysis.top_contributor_commits,
            total_commits: analysis.total_commits,
            source_repository: project.source_repository.clone(),
            scraped_at: project.scraped_at,
            last_updated: Utc::now(),
        };

        upsert_project(&self.db_pool, &updated_project).await?;

        // Store detailed contributor data
        if !analysis.top_contributors.is_empty() {
            upsert_contributors(&self.db_pool, &project.url, &analysis.top_contributors).await?;
        }

        Ok(())
    }

    pub async fn export_source_data(&self, source: &dyn AwesomeSource) -> Result<ScrapedData> {
        let conn = self.db_pool.get().await?;
        let source_repo = format!("{}/{}", source.owner(), source.repo());

        let mut stmt = conn.prepare(
            r#"
            SELECT url, description, owner, repo_name, repository_created, 
                   first_commit_date, last_commit_date, email, email_source,
                   top_contributor_email, top_contributor_commits, total_commits
            FROM projects 
            WHERE source_repository = ?
            ORDER BY url
            "#,
        )?;

        let rows = stmt.query_map([&source_repo], |row| {
            Ok(ProjectUrl {
                url: row.get(0)?,
                description: row.get(1)?,
                repository_created: row.get(4)?,
                first_commit_date: row.get(5)?,
                last_commit_date: row.get(6)?,
                owner: row.get(2)?,
                repo_name: row.get(3)?,
                email: row.get(7)?,
                email_source: row.get(8)?,
                top_contributor_email: row.get(9)?,
                top_contributor_commits: row
                    .get::<_, Option<String>>(10)?
                    .and_then(|s| s.parse().ok()),
                total_commits: row
                    .get::<_, Option<String>>(11)?
                    .and_then(|s| s.parse().ok()),
            })
        })?;

        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }

        Ok(ScrapedData {
            repository: source_repo,
            scraped_at: Utc::now().to_rfc3339(),
            total_urls: projects.len(),
            projects,
        })
    }

    fn extract_domain(&self, url: &str) -> String {
        if let Ok(parsed_url) = url::Url::parse(url) {
            if let Some(host) = parsed_url.host_str() {
                return host.to_string();
            }
        }
        "unknown".to_string()
    }

    fn determine_project_type(&self, url: &str, description: &str) -> String {
        let url_lower = url.to_lowercase();
        let desc_lower = description.to_lowercase();

        if url_lower.contains("shields.io") || url_lower.contains("badge") {
            "badge".to_string()
        } else if url_lower.contains("docs.")
            || url_lower.contains("/docs/")
            || desc_lower.contains("documentation")
        {
            "documentation".to_string()
        } else if url_lower.contains("api.") || desc_lower.contains("api") {
            "api".to_string()
        } else if url_lower.contains("blog.") || desc_lower.contains("blog") {
            "blog".to_string()
        } else if desc_lower.contains("tool") || desc_lower.contains("service") {
            "tool".to_string()
        } else if url_lower.starts_with("https://www.") || url_lower.starts_with("https://") {
            "website".to_string()
        } else {
            "other".to_string()
        }
    }
}
