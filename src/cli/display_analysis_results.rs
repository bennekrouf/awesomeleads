use crate::{models::CliApp, scraper_util::github::GitHubRepoAnalysis};

impl CliApp {
    pub fn display_analysis_results(&self, analysis: &GitHubRepoAnalysis) {
        println!("\n📊 Analysis Results");
        println!("━━━━━━━━━━━━━━━━━━━━━");

        println!("🏷️  Repository: {}/{}", analysis.owner, analysis.repo);

        // Repository creation date
        match &analysis.repository_created {
            Some(date) => {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date) {
                    println!("📅 Created: {}", dt.format("%Y-%m-%d %H:%M UTC"));
                } else {
                    println!("📅 Created: {}", date);
                }
            }
            None => println!("📅 Created: ❓ Unknown"),
        }

        // First commit date
        match &analysis.first_commit_date {
            Some(date) => {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date) {
                    println!("🏁 First commit: {}", dt.format("%Y-%m-%d %H:%M UTC"));
                } else {
                    println!("🏁 First commit: {}", date);
                }
            }
            None => println!("🏁 First commit: ❓ Unknown"),
        }

        // NEW: Last commit date
        match &analysis.last_commit_date {
            Some(date) => {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(date) {
                    println!("🔚 Last commit: {}", dt.format("%Y-%m-%d %H:%M UTC"));
                } else {
                    println!("🔚 Last commit: {}", date);
                }
            }
            None => println!("🔚 Last commit: ❓ Unknown"),
        }

        // NEW: Commit statistics
        if let Some(total) = analysis.total_commits {
            println!("📈 Total commits: {}", total);
        } else {
            println!("📈 Total commits: ❓ Unknown");
        }

        // Date requirements check
        if analysis.meets_date_requirements {
            println!("✅ Meets date requirements: Yes");
        } else {
            println!("❌ Meets date requirements: No");
            if let Some(reason) = &analysis.skip_reason {
                println!("   Reason: {}", reason);
            }
        }

        // Owner email information
        match &analysis.email {
            Some(email) => {
                println!("📧 Owner email: {}", email);
                if let Some(source) = &analysis.email_source {
                    println!("   Source: {}", source);
                }
            }
            None => {
                println!("📧 Owner email: ❌ None");
                if let Some(source) = &analysis.email_source {
                    if source.starts_with("skipped_") {
                        println!("   (Email fetch skipped due to date requirements)");
                    }
                }
            }
        }

        // NEW: Top contributor information
        match &analysis.top_contributor_email {
            Some(email) => {
                let commits = analysis.top_contributor_commits.unwrap_or(0);
                println!("🏆 Top contributor: {} ({} commits)", email, commits);

                // Show percentage if we have total commits
                if let Some(total) = analysis.total_commits {
                    if total > 0 {
                        let percentage = (commits as f64 / total as f64) * 100.0;
                        println!("   Contribution: {:.1}% of total commits", percentage);
                    }
                }
            }
            None => println!("🏆 Top contributor: ❓ Unknown"),
        }

        // NEW: Top contributors summary
        if !analysis.top_contributors.is_empty() {
            println!("\n👥 Top Contributors");
            println!("━━━━━━━━━━━━━━━━━━━");
            for (i, contributor) in analysis.top_contributors.iter().enumerate().take(5) {
                let email = contributor
                    .email
                    .as_deref()
                    .unwrap_or("unknown@unknown.com");
                let name = contributor.name.as_deref().unwrap_or("Unknown");
                let commits = contributor.commit_count;

                println!("{}. {} <{}> - {} commits", i + 1, name, email, commits);

                // Show date range if available
                if let (Some(first), Some(last)) = (
                    &contributor.first_commit_date,
                    &contributor.last_commit_date,
                ) {
                    if let (Ok(first_dt), Ok(last_dt)) = (
                        chrono::DateTime::parse_from_rfc3339(first),
                        chrono::DateTime::parse_from_rfc3339(last),
                    ) {
                        if first_dt.date_naive() == last_dt.date_naive() {
                            println!("   Active: {}", first_dt.format("%Y-%m-%d"));
                        } else {
                            println!(
                                "   Active: {} to {}",
                                first_dt.format("%Y-%m-%d"),
                                last_dt.format("%Y-%m-%d")
                            );
                        }
                    }
                }
            }

            if analysis.top_contributors.len() > 5 {
                println!(
                    "   ... and {} more contributors",
                    analysis.top_contributors.len() - 5
                );
            }
        }

        // Configuration info
        println!("\n⚙️  Current Configuration");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!(
            "📅 Min repository date: {}",
            self.config
                .scraping
                .min_repository_created_date
                .format("%Y-%m-%d")
        );
        println!(
            "🏁 Min first commit date: {}",
            self.config
                .scraping
                .min_first_commit_date
                .format("%Y-%m-%d")
        );
        println!(
            "⏱️  Rate limit delay: {}ms",
            self.config.scraping.rate_limit_delay_ms
        );
        println!(
            "⏰ API timeout: {}s",
            self.config.scraping.api_timeout_seconds
        );
    }
}
