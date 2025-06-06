use crate::{database::get_database_stats, models::CliApp, Result};
impl CliApp {
    pub async fn show_database_stats(&self) -> Result<()> {
        println!("\n📊 Database Statistics");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let stats = get_database_stats(&self.db_pool).await?;

        println!("🐙 GitHub projects: {}", stats.total_github_projects);
        println!(
            "🌐 Non-GitHub projects: {}",
            stats.total_non_github_projects
        );
        println!(
            "📦 Total projects: {}",
            stats.total_github_projects + stats.total_non_github_projects
        );
        println!(
            "📧 Projects with owner email: {}",
            stats.projects_with_email
        );
        println!(
            "🔗 Projects with GitHub data: {}",
            stats.projects_with_github_data
        );

        // NEW: Enhanced stats
        println!(
            "🏆 Projects with contributor data: {}",
            stats.projects_with_contributor_data
        );
        println!(
            "📈 Projects with commit stats: {}",
            stats.projects_with_commit_stats
        );

        if stats.avg_commits_per_project > 0.0 {
            println!(
                "📊 Average commits per project: {:.1}",
                stats.avg_commits_per_project
            );
        }

        println!("📚 Sources tracked: {}", stats.sources.len());

        if !stats.sources.is_empty() {
            println!("\n📚 Source Details:");
            for source in &stats.sources {
                let last_scraped = source
                    .last_scraped
                    .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
                    .unwrap_or_else(|| "Never".to_string());

                println!(
                    "  • {} ({} GitHub + {} non-GitHub, last: {})",
                    source.name,
                    source.total_github_projects,
                    source.total_non_github_projects,
                    last_scraped
                );
            }
        }

        // Calculate completion percentages
        if stats.total_github_projects > 0 {
            let github_data_percentage =
                (stats.projects_with_github_data * 100) / stats.total_github_projects;
            let contributor_data_percentage =
                (stats.projects_with_contributor_data * 100) / stats.total_github_projects;
            let commit_stats_percentage =
                (stats.projects_with_commit_stats * 100) / stats.total_github_projects;

            println!("\n📈 Data Completion Rates:");
            println!("  📊 GitHub data: {}%", github_data_percentage);
            println!("  🏆 Contributor data: {}%", contributor_data_percentage);
            println!("  📈 Commit statistics: {}%", commit_stats_percentage);
        }

        Ok(())
    }
}
