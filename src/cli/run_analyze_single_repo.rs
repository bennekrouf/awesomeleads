use dialoguer::{theme::ColorfulTheme, Input};

use crate::models::CliApp;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
impl CliApp {
    pub async fn run_analyze_single_repo(&self) -> Result<()> {
        println!("\n🧪 Single GitHub Repository Analysis");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Get GitHub URL from user
        let github_url: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter GitHub repository URL")
            .with_initial_text("https://github.com/")
            .interact_text()?;

        if github_url.trim().is_empty() || github_url.trim() == "https://github.com/" {
            println!("❌ No URL provided");
            return Ok(());
        }

        // Parse the GitHub URL
        let (owner, repo) = match self.scraper.parse_github_url(&github_url) {
            Ok((owner, repo)) => (owner, repo),
            Err(e) => {
                println!("❌ {}", e);
                println!("💡 Example: https://github.com/microsoft/vscode");
                return Ok(());
            }
        };

        println!("\n🔍 Analyzing repository: {}/{}", owner, repo);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Analyze the repository
        match self.scraper.analyze_github_repo(&owner, &repo).await {
            Ok(analysis) => {
                self.display_analysis_results(&analysis);
            }
            Err(e) => {
                println!("❌ Analysis failed: {}", e);

                // Check if it's a common error and provide helpful suggestions
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("not found") || error_str.contains("404") {
                    println!("💡 The repository might not exist or might be private");
                } else if error_str.contains("rate limit") {
                    println!("💡 GitHub rate limit reached. Try again later or set GITHUB_TOKEN");
                } else if error_str.contains("timeout") {
                    println!("💡 Request timed out. The repository might be very large");
                }
            }
        }

        Ok(())
    }
}
