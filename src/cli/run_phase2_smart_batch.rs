use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

use crate::models::CliApp;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
impl CliApp {
    pub async fn run_phase2_smart_batch(&self) -> Result<()> {
        println!("\n🎯 Smart Phase 2: High-Value Project Batch Processing");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // First, show current progress
        let progress = self.get_phase2_progress_summary().await?;
        println!("📊 Current Status:");
        println!("  ✅ Complete: {} projects", progress.complete);
        println!("  🔄 Partial: {} projects", progress.partial);
        println!("  ⏳ Untouched: {} projects", progress.untouched);
        println!("  📈 Completion Rate: {:.1}%", progress.completion_rate);

        if progress.untouched == 0 && progress.partial == 0 {
            println!("\n🎉 All projects are already complete!");
            return Ok(());
        }

        // Project type selection
        let priority_options = vec![
            "🦀 Rust projects (high-value, modern)",
            "🟨 JavaScript/Node.js projects (popular ecosystem)",
            "🐍 Python projects (data science, AI/ML)",
            "⚡ Go projects (cloud, infrastructure)",
            "💎 Ruby projects (web development)",
            "☕ Java projects (enterprise)",
            "🔥 Recent projects (created after 2022)",
            "⭐ Popular projects (likely high-quality)",
            "🎯 Mixed high-value batch (best of all)",
            "🧹 Cleanup partial projects",
            "📝 Custom filter",
        ];

        let priority_selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select project priority type")
            .default(8) // Default to mixed high-value
            .items(&priority_options)
            .interact()?;

        // Batch size selection
        let suggested_batch_size = if progress.untouched > 5000 {
            200
        } else if progress.untouched > 1000 {
            500
        } else {
            progress.untouched.min(1000)
        };

        let batch_size: usize = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Batch size (projects to process)")
            .default(suggested_batch_size)
            .interact_text()?
            .try_into()
            .unwrap();

        // Build filter based on selection
        let filter_info = self.build_project_filter(priority_selection).await?;

        println!("\n🔍 Filter: {}", filter_info.description);
        println!("📦 Batch size: {} projects", batch_size);

        // Get the filtered projects
        let projects = self
            .get_prioritized_projects(&filter_info.sql_filter, batch_size)
            .await?;

        if projects.is_empty() {
            println!("🤷 No projects match the selected criteria!");
            return Ok(());
        }

        println!("\n📋 Found {} projects matching criteria", projects.len());

        // Show sample of what will be processed
        println!("\n🔍 Sample projects to process:");
        for (i, project) in projects.iter().take(5).enumerate() {
            if let (Some(owner), Some(repo)) = (&project.owner, &project.repo_name) {
                let desc = project.description.as_deref().unwrap_or("No description");
                println!(
                    "  {}. {}/{} - {}",
                    i + 1,
                    owner,
                    repo,
                    desc.chars().take(60).collect::<String>()
                );
            }
        }
        if projects.len() > 5 {
            println!("  ... and {} more", projects.len() - 5);
        }

        // Confirm before processing
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Proceed with processing this batch?")
            .default(true)
            .interact()?;

        if !proceed {
            println!("👍 Batch cancelled");
            return Ok(());
        }

        // Process the batch
        let mut successful_updates = 0;
        let mut failed_updates = 0;
        let mut skipped_updates = 0;

        println!("\n🚀 Processing batch...");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        for (i, project) in projects.iter().enumerate() {
            if let (Some(owner), Some(repo)) = (&project.owner, &project.repo_name) {
                println!("[{}/{}] 🔍 {}/{}", i + 1, projects.len(), owner, repo);

                // Skip low-value projects if detected
                if self
                    .is_low_value_project(owner, repo, &project.description)
                    .await
                {
                    skipped_updates += 1;
                    println!("   ⏭️  Skipped (low-value: docs/badges/archive)");
                    continue;
                }

                match self.scraper.fetch_and_update_github_data(project).await {
                    Ok(_) => {
                        successful_updates += 1;
                        println!("   ✅ Updated successfully");
                    }
                    Err(e) => {
                        failed_updates += 1;
                        let error_msg = e.to_string();
                        if error_msg.contains("404") || error_msg.contains("not found") {
                            println!("   ❌ Repository not found or private");
                        } else if error_msg.contains("rate limit") {
                            println!("   ⏰ Rate limited - will retry next run");
                        } else {
                            println!(
                                "   ⚠️  Failed: {}",
                                error_msg.chars().take(50).collect::<String>()
                            );
                        }
                    }
                }

                // Adaptive rate limiting based on success rate
                let success_rate = if i > 10 {
                    successful_updates as f64 / (i + 1) as f64
                } else {
                    1.0
                };
                let delay_ms = if success_rate > 0.8 {
                    self.config.scraping.rate_limit_delay_ms
                } else {
                    self.config.scraping.rate_limit_delay_ms * 2 // Slow down if many failures
                };

                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

                // Progress update every 25 projects
                if (i + 1) % 25 == 0 {
                    let current_success_rate = successful_updates as f64 / (i + 1) as f64 * 100.0;
                    println!(
                        "   📊 Progress: {}/{} ({:.1}% success rate)",
                        i + 1,
                        projects.len(),
                        current_success_rate
                    );
                }
            }
        }

        // Results summary
        println!("\n🎉 Smart Batch Complete!");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("✅ Successful updates: {}", successful_updates);
        println!("❌ Failed updates: {}", failed_updates);
        println!("⏭️  Skipped (low-value): {}", skipped_updates);

        let total_processed = successful_updates + failed_updates + skipped_updates;
        let success_rate = if total_processed > 0 {
            successful_updates as f64 / total_processed as f64 * 100.0
        } else {
            0.0
        };

        println!("📊 Success rate: {:.1}%", success_rate);

        // Show updated progress
        let new_progress = self.get_phase2_progress_summary().await?;
        let improvement = new_progress.completion_rate - progress.completion_rate;
        if improvement > 0.0 {
            println!(
                "📈 Completion rate improved by {:.1}% (now {:.1}%)",
                improvement, new_progress.completion_rate
            );
        }

        // Recommendations for next steps
        if new_progress.untouched > 0 || new_progress.partial > 0 {
            println!("\n💡 Next Steps:");
            if new_progress.untouched > 1000 {
                println!("  • Run another smart batch to continue with high-value projects");
            }
            if new_progress.partial > 100 {
                println!("  • Use 'Cleanup partial projects' to complete failed attempts");
            }
            if success_rate < 70.0 {
                println!("  • Consider checking GitHub token and rate limits");
            }
            println!("  • Use 'Show Phase 2 detailed progress' to see current status");
        } else {
            println!("\n🎯 Congratulations! All projects are now complete!");
        }

        Ok(())
    }
}
