// src/email_export/processor.rs
use super::types::{CompanySize, DomainCategory, EmailExport, ExportConfig, RawEmailData};
use chrono::Utc;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct EmailProcessor;

impl EmailProcessor {
    pub fn new() -> Self {
        Self
    }

    pub async fn process_email_data(
        &self,
        raw: RawEmailData,
        _config: &ExportConfig,
    ) -> Result<EmailExport> {
        let domain = self.extract_domain(&raw.email);
        let domain_category = self.classify_domain(&domain, &raw.description);
        let company_size = self.estimate_company_size(&raw);
        let engagement_score = self.calculate_engagement_score(&raw);
        let tags = self.generate_tags(&raw, &domain_category);
        let industry = self.detect_industry(&raw.description, &domain);
        let (name, first_name) = self.extract_names(&raw);

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
}
