use crate::{database::get_projects_needing_github_data, models::CliApp};

use tracing::warn;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl CliApp {
    pub async fn run_phase2(&self) -> Result<()> {
        println!("\n📡 Starting Phase 2: Fetching GitHub data for incomplete projects...");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let projects_needing_data = get_projects_needing_github_data(&self.db_pool, 0).await?; // 24 hours

        if projects_needing_data.is_empty() {
            println!("✨ No projects need GitHub data updates!");
            return Ok(());
        }

        println!(
            "Found {} projects needing GitHub data",
            projects_needing_data.len()
        );

        let mut successful_updates = 0;
        let mut failed_updates = 0;

        for (i, project) in projects_needing_data.iter().enumerate() {
            if let (Some(owner), Some(repo)) = (&project.owner, &project.repo_name) {
                println!(
                    "[{}/{}] Fetching: {}/{}",
                    i + 1,
                    projects_needing_data.len(),
                    owner,
                    repo
                );

                match self.scraper.fetch_and_update_github_data(project).await {
                    Ok(_) => {
                        successful_updates += 1;
                        println!("✓ Updated {}/{}", owner, repo);
                    }
                    Err(e) => {
                        failed_updates += 1;
                        warn!("✗ Failed to update {}/{}: {}", owner, repo, e);
                    }
                }

                // Rate limiting
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    self.config.scraping.rate_limit_delay_ms,
                ))
                .await;
            }
        }

        println!("\n🎉 Phase 2 Complete!");
        println!("Successful updates: {}", successful_updates);
        println!("Failed updates: {}", failed_updates);

        Ok(())
    }
}

