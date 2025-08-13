use tracing::info;

use crate::config::Config;
use crate::database::DbPool;
use crate::models::CliApp;
use crate::scraper_util::AwesomeScraper;
use crate::sources::{load_sources_from_yaml, AwesomeSource};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone)]
pub enum MenuAction {
    Phase1ScrapeUrls,
    Phase2FetchGithubData,
    Phase2SmartBatch,
    Phase3ExportResults,
    AnalyzeSingleRepo,
    WebCrawlerContactDiscovery,
    BusinessContactDiscovery,  // NEW: Add business-focused option
    AutomatedDailyCampaign,
    SendEmailCampaign,
    ShowStats,
    ShowPhase2Progress,
    ExportEmails,
    DebugEnvironmentCheck,
    Exit,
}

impl std::fmt::Display for MenuAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MenuAction::Phase1ScrapeUrls => {
                write!(f, "🔍 Phase 1: Scrape awesome lists for project URLs")
            }
            MenuAction::Phase2FetchGithubData => {
                write!(f, "📡 Phase 2: Fetch GitHub data (all projects)")
            }
            MenuAction::Phase2SmartBatch => {
                write!(f, "🎯 Phase 2: Smart batch processing (high-value first)")
            }
            MenuAction::Phase3ExportResults => {
                write!(f, "📤 Phase 3: Export results to JSON files")
            }
            MenuAction::AnalyzeSingleRepo => {
                write!(f, "🧪 Analyze Single GitHub Repository")
            }
            MenuAction::WebCrawlerContactDiscovery => {
                write!(f, "🕷️  Web Crawler: General contact discovery")
            }
            MenuAction::BusinessContactDiscovery => {  // NEW: Add this
                write!(f, "🏢 Business Discovery: Find companies & decision-makers")
            }
            MenuAction::AutomatedDailyCampaign => {
                write!(f, "🤖 Automated Daily Campaign (300 emails)")
            }
            MenuAction::SendEmailCampaign => {
                write!(f, "📧 Send Email Campaign via Mailgun")
            }
            MenuAction::ShowStats => write!(f, "📊 Show database statistics"),
            MenuAction::ShowPhase2Progress => write!(f, "📈 Show Phase 2 detailed progress"),
            MenuAction::ExportEmails => write!(f, "📧 Export Lead Emails to CSV"),
            MenuAction::DebugEnvironmentCheck => write!(f, "🔍 Debug Environment Check"),
            MenuAction::Exit => write!(f, "🚪 Exit"),
        }
    }
}

impl CliApp {
    pub async fn new(config: Config, db_pool: DbPool) -> Result<Self> {
        // Initialize scraper
        let scraper = AwesomeScraper::new(config.clone(), db_pool.clone()).await?;

        // Load sources from YAML
        info!("Loading sources from configuration...");
        let yaml_sources = load_sources_from_yaml("sources.yml").await?;

        // Convert to trait objects
        let sources: Vec<Box<dyn AwesomeSource>> = yaml_sources
            .into_iter()
            .map(|s| Box::new(s) as Box<dyn AwesomeSource>)
            .collect();

        info!("Loaded {} sources from configuration", sources.len());

        Ok(Self {
            config,
            db_pool,
            scraper,
            sources,
        })
    }
}
