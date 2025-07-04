use dialoguer::{theme::ColorfulTheme, Select};

use crate::{
    cli::cli::MenuAction,
    models::{CliApp, Result},
};
use tracing::error;

impl CliApp {
    pub async fn run(&self) -> Result<()> {
        println!("\n🚀 Welcome to Lead Scraper!");
        println!("═══════════════════════════════════════");

        // Show initial stats
        self.show_database_stats().await?;

        loop {
            let actions = vec![
                MenuAction::Phase1ScrapeUrls,
                MenuAction::Phase2SmartBatch, // NEW: Prioritize this option
                MenuAction::Phase2FetchGithubData,
                MenuAction::Phase3ExportResults,
                MenuAction::AnalyzeSingleRepo,
                MenuAction::ShowStats,
                MenuAction::ShowPhase2Progress, // NEW: Progress trackin
                MenuAction::ExportEmails,
                MenuAction::Exit,
            ];

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("\nSelect an action")
                .default(1) // Default to smart batch
                .items(&actions)
                .interact()?;

            match &actions[selection] {
                MenuAction::Phase1ScrapeUrls => {
                    if let Err(e) = self.run_phase1().await {
                        error!("Phase 1 failed: {}", e);
                    }
                }
                MenuAction::Phase2FetchGithubData => {
                    if let Err(e) = self.run_phase2().await {
                        error!("Phase 2 failed: {}", e);
                    }
                }
                MenuAction::Phase2SmartBatch => {
                    // NEW
                    if let Err(e) = self.run_phase2_smart_batch().await {
                        error!("Smart Phase 2 failed: {}", e);
                    }
                }
                MenuAction::Phase3ExportResults => {
                    if let Err(e) = self.run_phase3().await {
                        error!("Phase 3 failed: {}", e);
                    }
                }
                MenuAction::AnalyzeSingleRepo => {
                    if let Err(e) = self.run_analyze_single_repo().await {
                        error!("Single repo analysis failed: {}", e);
                    }
                }
                MenuAction::ShowStats => {
                    if let Err(e) = self.show_database_stats().await {
                        error!("Failed to show stats: {}", e);
                    }
                }
                MenuAction::ShowPhase2Progress => {
                    // NEW
                    if let Err(e) = self.show_phase2_progress().await {
                        error!("Failed to show Phase 2 progress: {}", e);
                    }
                }
                MenuAction::ExportEmails => {
                    if let Err(e) = self.run_export_emails().await {
                        error!("Email export failed: {}", e);
                    }
                }
                MenuAction::Exit => {
                    println!("\n👋 Thanks for using Lead Scraper!");
                    break;
                }
            }
        }

        Ok(())
    }
}
