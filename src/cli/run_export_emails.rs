// src/cli/run_export_emails.rs
use crate::{models::CliApp, Result};
use chrono::Utc;
use dialoguer::{theme::ColorfulTheme, Confirm, Select};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubscriptionStatus {
    #[serde(rename = "subscribed")]
    Subscribed,
    #[serde(rename = "unsubscribed")]
    Unsubscribed,
    #[serde(rename = "never_subscribed")]
    NeverSubscribed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainCategory {
    #[serde(rename = "web3")]
    Web3,
    #[serde(rename = "ai")]
    AI,
    #[serde(rename = "fintech")]
    Fintech,
    #[serde(rename = "saas")]
    SaaS,
    #[serde(rename = "enterprise")]
    Enterprise,
    #[serde(rename = "other")]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompanySize {
    #[serde(rename = "startup")]
    Startup,
    #[serde(rename = "scale-up")]
    ScaleUp,
    #[serde(rename = "enterprise")]
    Enterprise,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailExport {
    pub email: String,
    pub name: Option<String>, 
    pub first_name: Option<String>,
    pub status: String,
    pub consent_timestamp: String,
    pub source: String,
    pub domain_category: String,
    pub tags: String,
    pub company_size: String,
    pub industry: String,
    pub engagement_score: u8,
    pub project_url: String,
    pub repository_created: String,
    pub commit_count: String,
}

impl CliApp {
    pub async fn run_export_emails(&self) -> Result<()> {
        println!("\n📧 Email Export System");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Export type selection
        let export_types = vec![
            "📊 All Valid Emails (Real emails only)",
            "🎯 High-Value Projects (Recent + Active)",
            "🚀 Startup Founders (Early commits + ownership)",
            "🏢 Enterprise Contacts (Large repos + teams)",
            "🔥 Web3/AI/Fintech Focus",
            "📈 Custom Filtered Export",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select export type")
            .items(&export_types)
            .interact()?;

        let export_config = self.build_export_config(selection).await?;
        let emails = self.extract_emails(&export_config).await?;

        if emails.is_empty() {
            println!("❌ No emails found matching criteria");
            return Ok(());
        }

        // Show preview
        println!("\n📋 Export Preview:");
        println!("━━━━━━━━━━━━━━━━━━━━━");
        for (i, email) in emails.iter().take(5).enumerate() {
            println!("{}. {} ({})", i + 1, email.email, email.domain_category);
        }
        if emails.len() > 5 {
            println!("   ... and {} more", emails.len() - 5);
        }

        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!("Export {} emails to CSV?", emails.len()))
            .interact()?;

        if !proceed {
            println!("❌ Export cancelled");
            return Ok(());
        }

        // Export to CSV
        let filename = format!(
            "out/emails_export_{}.csv",
            Utc::now().format("%Y%m%d_%H%M%S")
        );

        self.export_emails_to_csv(&emails, &filename).await?;

        println!("\n✅ Email export completed!");
        println!("📁 File: {}", filename);
        println!("📊 Total emails: {}", emails.len());
        self.print_export_stats(&emails);

        Ok(())
    }

    async fn build_export_config(&self, selection: usize) -> Result<ExportConfig> {
        match selection {
            0 => Ok(ExportConfig {
                // title: "All Valid Emails".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%')".to_string(),
                // min_engagement_score: 10,
                // focus_domains: vec![],
            }),
            1 => Ok(ExportConfig {
                // title: "High-Value Projects".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%') AND (repository_created > '2022-01-01' OR first_commit_date > '2022-01-01') AND total_commits > 5".to_string(),
                // min_engagement_score: 50,
                // focus_domains: vec![],
            }),
            2 => Ok(ExportConfig {
                // title: "Startup Founders".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%') AND total_commits > 20 AND repository_created > '2020-01-01'".to_string(),
                // min_engagement_score: 70,
                // focus_domains: vec![],
            }),
            3 => Ok(ExportConfig {
                // title: "Enterprise Contacts".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%') AND total_commits > 100".to_string(),
                // min_engagement_score: 60,
                // focus_domains: vec![],
            }),
            4 => Ok(ExportConfig {
                // title: "Web3/AI/Fintech Focus".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%') AND (LOWER(description) LIKE '%blockchain%' OR LOWER(description) LIKE '%ai%' OR LOWER(description) LIKE '%ml%' OR LOWER(description) LIKE '%fintech%' OR LOWER(description) LIKE '%defi%' OR LOWER(url) LIKE '%web3%')".to_string(),
                // min_engagement_score: 40,
                // focus_domains: vec!["web3".to_string(), "ai".to_string(), "fintech".to_string()],
            }),
            _ => Ok(ExportConfig {
                // title: "Custom Export".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%')".to_string(),
                // min_engagement_score: 30,
                // focus_domains: vec![],
            }),
        }
    }

    async fn extract_emails(&self, config: &ExportConfig) -> Result<Vec<EmailExport>> {
        let conn = self.db_pool.get().await?;

        let sql = format!(
            r#"
            SELECT DISTINCT
                p.email,
                COALESCE(c.name, p.owner) as name,  -- Use contributor name, fallback to owner
                p.url,
                p.description,
                p.repository_created,
                p.first_commit_date,
                p.total_commits,
                p.owner,
                p.repo_name,
                p.source_repository
            FROM projects p
            {}
            ORDER BY p.total_commits DESC NULLS LAST, p.repository_created DESC NULLS LAST
            "#,
            config.sql_filter
        );

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(RawEmailData {
                email: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                description: row.get::<_, Option<String>>(3)?,
                repository_created: row.get::<_, Option<String>>(4)?,
                first_commit_date: row.get::<_, Option<String>>(5)?,
                total_commits: row.get::<_, Option<i32>>(6)?,
                owner: row.get::<_, Option<String>>(7)?,
                repo_name: row.get::<_, Option<String>>(8)?,
                source_repository: row.get(9)?,
            })
        })?;

        let mut emails = Vec::new();
        let mut seen_emails = HashSet::new();

        for row in rows {
            let raw = row?;

            // Deduplicate by email
            if !seen_emails.insert(raw.email.clone()) {
                continue;
            }

            // Validate email format
            if !self.is_valid_email(&raw.email) {
                continue;
            }

            let export = self.process_email_data(raw, config).await?;
            emails.push(export);
        }

        Ok(emails)
    }

    async fn process_email_data(
        &self,
        raw: RawEmailData,
        _config: &ExportConfig,
    ) -> Result<EmailExport> {
        // Extract first name for personalization
        let (name, first_name) = self.extract_names(&raw);
        
        let domain = self.extract_domain(&raw.email);
        let domain_category = self.classify_domain(&domain, &raw.description);
        let company_size = self.estimate_company_size(&raw);
        let engagement_score = self.calculate_engagement_score(&raw);
        let tags = self.generate_tags(&raw, &domain_category);
        let industry = self.detect_industry(&raw.description, &domain);

        Ok(EmailExport {
            email: raw.email.clone(),
            name,
            first_name,
            status: "never_subscribed".to_string(),
            consent_timestamp: Utc::now().to_rfc3339(),
            source: format!("github_scraper_{}", raw.source_repository.replace('/', "_")),
            domain_category: format!("{:?}", domain_category).to_lowercase(),
            tags: tags.join(","),
            company_size: format!("{:?}", company_size).to_lowercase(),
            industry,
            engagement_score,
            project_url: raw.url,
            repository_created: raw.repository_created.unwrap_or_default(),
            commit_count: raw.total_commits.map(|c| c.to_string()).unwrap_or_default(),
        })
    }

    fn classify_domain(&self, domain: &str, description: &Option<String>) -> DomainCategory {
        let domain_lower = domain.to_lowercase();
        let desc_lower = description.as_deref().unwrap_or("").to_lowercase();

        // Web3 indicators
        if domain_lower.contains("chainlink")
            || domain_lower.contains("ethereum")
            || domain_lower.contains("polygon")
            || domain_lower.contains("solana")
            || desc_lower.contains("blockchain")
            || desc_lower.contains("defi")
            || desc_lower.contains("web3")
            || desc_lower.contains("crypto")
        {
            return DomainCategory::Web3;
        }

        // AI indicators
        if domain_lower.contains("openai")
            || domain_lower.contains("anthropic")
            || domain_lower.contains("huggingface")
            || desc_lower.contains(" ai ")
            || desc_lower.contains("machine learning")
            || desc_lower.contains("llm")
        {
            return DomainCategory::AI;
        }

        // Fintech indicators
        if domain_lower.contains("stripe")
            || domain_lower.contains("plaid")
            || domain_lower.contains("square")
            || desc_lower.contains("fintech")
            || desc_lower.contains("payment")
            || desc_lower.contains("banking")
        {
            return DomainCategory::Fintech;
        }

        // Enterprise indicators
        if domain_lower.contains("microsoft")
            || domain_lower.contains("google")
            || domain_lower.contains("amazon")
            || domain_lower.contains("oracle")
            || domain_lower.contains("ibm")
        {
            return DomainCategory::Enterprise;
        }

        // SaaS indicators
        if desc_lower.contains("saas")
            || desc_lower.contains("platform")
            || desc_lower.contains("api")
            || desc_lower.contains("service")
        {
            return DomainCategory::SaaS;
        }

        DomainCategory::Other
    }

    fn estimate_company_size(&self, raw: &RawEmailData) -> CompanySize {
        let commits = raw.total_commits.unwrap_or(0);

        if commits > 500 {
            CompanySize::Enterprise
        } else if commits > 50 {
            CompanySize::ScaleUp
        } else {
            CompanySize::Startup
        }
    }

    fn calculate_engagement_score(&self, raw: &RawEmailData) -> u8 {
        let mut score = 0u8;

        // Commit activity (0-40 points)
        if let Some(commits) = raw.total_commits {
            score += (commits.min(200) / 5) as u8; // Max 40 points
        }

        // Recent activity (0-20 points)
        if let Some(created) = &raw.repository_created {
            if **created > *"2023-01-01" {
                score += 20;
            } else if **created > *"2022-01-01" {
                score += 10;
            }
        }

        // Project quality indicators (0-40 points)
        if let Some(desc) = &raw.description {
            if desc.len() > 50 {
                score += 10;
            } // Has description
            if desc.to_lowercase().contains("api") {
                score += 10;
            }
            if desc.to_lowercase().contains("open source") {
                score += 10;
            }
            if desc.to_lowercase().contains("production") {
                score += 10;
            }
        }

        score.min(100)
    }

    fn generate_tags(&self, raw: &RawEmailData, category: &DomainCategory) -> Vec<String> {
        let mut tags = Vec::new();

        // Add category-specific tags
        match category {
            DomainCategory::Web3 => {
                tags.extend(vec!["blockchain".to_string(), "cryptocurrency".to_string()])
            }
            DomainCategory::AI => tags.extend(vec![
                "artificial-intelligence".to_string(),
                "machine-learning".to_string(),
            ]),
            DomainCategory::Fintech => tags.extend(vec![
                "financial-technology".to_string(),
                "payments".to_string(),
            ]),
            _ => {}
        }

        // Add activity level tags
        if let Some(commits) = raw.total_commits {
            if commits > 100 {
                tags.push("high-activity".to_string());
            }
        }

        // Add recency tags
        if let Some(created) = &raw.repository_created {
            if **created > *"2023-01-01" {
                tags.push("recent-project".to_string());
            }
        }

        tags.truncate(10); // Max 10 tags
        tags
    }

    fn detect_industry(&self, description: &Option<String>, domain: &str) -> String {
        let desc = description.as_deref().unwrap_or("").to_lowercase();
        let domain_lower = domain.to_lowercase();

        if desc.contains("healthcare") || desc.contains("medical") {
            "healthcare".to_string()
        } else if desc.contains("education") || desc.contains("learning") {
            "education".to_string()
        } else if desc.contains("ecommerce") || desc.contains("retail") {
            "retail".to_string()
        } else if domain_lower.contains("gov") || desc.contains("government") {
            "government".to_string()
        } else {
            "technology".to_string()
        }
    }

    fn extract_domain(&self, email: &str) -> String {
        email.split('@').nth(1).unwrap_or("unknown").to_string()
    }

    fn is_valid_email(&self, email: &str) -> bool {
        email.contains('@')
            && !email.contains("noreply")
            && !email.contains("[bot]")
            && email.len() > 5
            && email.len() < 255
    }

    fn extract_names(&self, raw: &RawEmailData) -> (Option<String>, Option<String>) {
        let full_name = raw.name.clone().or_else(|| {
            // Fallback: try to extract name from owner field
            raw.owner.clone()
        });
        
        let first_name = full_name.as_ref().map(|name| {
            // Extract first name (everything before first space)
            name.split_whitespace().next().unwrap_or(name).to_string()
        });
        
        (full_name, first_name)
    }

    async fn export_emails_to_csv(&self, emails: &[EmailExport], filename: &str) -> Result<()> {
        use std::io::Write;
        let mut file = std::fs::File::create(filename)?;

        // Write CSV header
        writeln!(file, "email,status,consent_timestamp,source,domain_category,tags,company_size,industry,engagement_score,project_url,repository_created,commit_count")?;

        // Write data rows
        for email in emails {
            writeln!(
                file,
                "{},{},{},{},{},{},{},{},{},{},{},{}",
                email.email,
                email.status,
                email.consent_timestamp,
                email.source,
                email.domain_category,
                email.tags,
                email.company_size,
                email.industry,
                email.engagement_score,
                email.project_url,
                email.repository_created,
                email.commit_count
            )?;
        }

        Ok(())
    }

    fn print_export_stats(&self, emails: &[EmailExport]) {
        let mut category_counts: HashMap<String, usize> = HashMap::new();
        let mut size_counts: HashMap<String, usize> = HashMap::new();

        for email in emails {
            *category_counts
                .entry(email.domain_category.clone())
                .or_insert(0) += 1;
            *size_counts.entry(email.company_size.clone()).or_insert(0) += 1;
        }

        println!("\n📊 Export Statistics:");
        println!("━━━━━━━━━━━━━━━━━━━━━");

        println!("🏷️  By Category:");
        for (category, count) in category_counts {
            println!(
                "   {} {}: {}",
                match category.as_str() {
                    "web3" => "🪙",
                    "ai" => "🤖",
                    "fintech" => "💳",
                    "enterprise" => "🏢",
                    "saas" => "☁️",
                    _ => "📦",
                },
                category,
                count
            );
        }

        println!("\n🏢 By Company Size:");
        for (size, count) in size_counts {
            println!(
                "   {} {}: {}",
                match size.as_str() {
                    "startup" => "🚀",
                    "scale-up" => "📈",
                    "enterprise" => "🏢",
                    _ => "❓",
                },
                size,
                count
            );
        }

        let avg_engagement: f64 = emails
            .iter()
            .map(|e| e.engagement_score as f64)
            .sum::<f64>()
            / emails.len() as f64;

        println!("\n⭐ Average Engagement Score: {:.1}", avg_engagement);
    }
}

#[derive(Debug)]
struct ExportConfig {
    // title: String,
    sql_filter: String,
    // min_engagement_score: u8,
    // focus_domains: Vec<String>,
}

#[derive(Debug)]
struct RawEmailData {
    email: String,
    name: Option<String>, 
    url: String,
    description: Option<String>,
    repository_created: Option<String>,
    first_commit_date: Option<String>,
    total_commits: Option<i32>,
    owner: Option<String>,
    repo_name: Option<String>,
    source_repository: String,
}
