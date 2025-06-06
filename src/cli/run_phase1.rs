use crate::{database::update_source_last_scraped, models::CliApp, Result};

use tracing::{error, warn};
impl CliApp {
    pub async fn run_phase1(&self) -> Result<()> {
        println!("\n🔍 Starting Phase 1: Scraping awesome lists for project URLs...");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let mut total_github_projects = 0;
        let mut total_non_github_projects = 0;
        let mut meta_sources_processed = 0;
        let mut regular_sources_processed = 0;

        for (i, source) in self.sources.iter().enumerate() {
            if source.is_meta_source() {
                println!(
                    "\n[{}/{}] 🌐 Processing Meta-Source: {} (will discover sub-lists)",
                    i + 1,
                    self.sources.len(),
                    source.name()
                );
                meta_sources_processed += 1;
            } else {
                println!(
                    "\n[{}/{}] 📋 Processing Regular Source: {}",
                    i + 1,
                    self.sources.len(),
                    source.name()
                );
                regular_sources_processed += 1;
            }

            match self.scraper.scrape_source_urls(source.as_ref()).await {
                Ok((github_count, non_github_count)) => {
                    total_github_projects += github_count;
                    total_non_github_projects += non_github_count;

                    if source.is_meta_source() {
                        println!(
                        "✓ {} - Discovered and scraped {} GitHub + {} non-GitHub projects from sub-lists",
                        source.name(),
                        github_count,
                        non_github_count
                    );
                    } else {
                        println!(
                            "✓ {} - Found {} GitHub + {} non-GitHub projects",
                            source.name(),
                            github_count,
                            non_github_count
                        );
                    }

                    if let Err(e) = update_source_last_scraped(
                        &self.db_pool,
                        source.name(),
                        &format!("{}/{}", source.owner(), source.repo()),
                        github_count as i64,
                        non_github_count as i64,
                    )
                    .await
                    {
                        warn!("Failed to update source timestamp: {}", e);
                    }
                }
                Err(e) => {
                    error!("✗ {} - Failed: {}", source.name(), e);
                }
            }
        }

        println!("\n🎉 Phase 1 Complete!");
        println!("━━━━━━━━━━━━━━━━━━━━━━");
        println!("📊 Processing Summary:");
        println!("  🌐 Meta-sources processed: {}", meta_sources_processed);
        println!(
            "  📋 Regular sources processed: {}",
            regular_sources_processed
        );
        println!(
            "  📦 Total GitHub projects found: {}",
            total_github_projects
        );
        println!(
            "  🌐 Total non-GitHub projects found: {}",
            total_non_github_projects
        );
        println!(
            "  🎯 Grand total: {}",
            total_github_projects + total_non_github_projects
        );

        if meta_sources_processed > 0 {
            println!("\n💡 Note: Meta-sources like sindresorhus/awesome discover and scrape");
            println!("   multiple sub-lists automatically, greatly expanding your dataset!");
        }

        Ok(())
    }
}
