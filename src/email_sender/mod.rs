// src/email_sender/mod.rs
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub struct MailgunConfig {
    pub api_key: String,
    pub domain: String,
    pub from_email: String,
    pub from_name: String,
    pub template_name: String,
    pub base_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailRecipient {
    pub email: String,
    pub recipient_name: String,
    pub repo_name: String,
    pub specific_aspect: String,
    pub contact_email: String,
    pub contact_phone: String,
    pub engagement_score: u8,
    pub domain_category: String,
    pub company_size: String,
}

#[derive(Debug, Deserialize)]
pub struct MailgunResponse {
    pub id: String,
    pub message: String,
}

pub struct MailgunSender {
    pub config: MailgunConfig,
    client: Client,
}

impl MailgunSender {
    pub fn new(config: MailgunConfig) -> Self {
        let client = Client::new();
        debug!("Created MailgunSender for domain: {}", config.domain);
        Self { config, client }
    }

    pub async fn send_email(
        &self,
        recipient: &EmailRecipient,
        subject: &str,
    ) -> Result<MailgunResponse, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/{}/messages", self.config.base_url, self.config.domain);

        debug!("Preparing email for {}: {}", recipient.email, subject);

        // Create Mailgun variables JSON (matching your exact curl format)
        let variables = json!({
            "recipient_name": recipient.recipient_name,
            "repo_name": recipient.repo_name,
            "specific_aspect": recipient.specific_aspect,
            "contact_email": recipient.contact_email,
            "contact_phone": recipient.contact_phone
        });

        debug!("Template variables: {}", variables);

        let mut form_data = HashMap::new();
        form_data.insert(
            "from",
            format!("{} <{}>", self.config.from_name, self.config.from_email),
        );
        form_data.insert(
            "to",
            format!("{} <{}>", recipient.recipient_name, recipient.email),
        );
        form_data.insert("subject", subject.to_string());
        form_data.insert("template", self.config.template_name.clone());
        form_data.insert("h:X-Mailgun-Variables", variables.to_string());

        // Add tracking parameters
        form_data.insert("o:tracking", "yes".to_string());
        form_data.insert("o:tracking-clicks", "yes".to_string());
        form_data.insert("o:tracking-opens", "yes".to_string());

        // Add custom tags for analytics
        form_data.insert(
            "o:tag",
            format!("campaign-{}", chrono::Utc::now().format("%Y-%m")),
        );
        form_data.insert("o:tag", format!("category-{}", recipient.domain_category));
        form_data.insert("o:tag", format!("score-{}", recipient.engagement_score));

        debug!("Sending POST request to: {}", url);

        let response = self
            .client
            .post(&url)
            .basic_auth("api", Some(&self.config.api_key))
            .form(&form_data)
            .send()
            .await?;

        debug!("Mailgun response status: {}", response.status());

        if response.status().is_success() {
            let mailgun_response: MailgunResponse = response.json().await?;
            debug!("Mailgun success response: {:?}", mailgun_response);
            Ok(mailgun_response)
        } else {
            let error_text = response.text().await?;
            error!("Mailgun API error: {}", error_text);
            Err(format!("Mailgun error: {}", error_text).into())
        }
    }

    pub async fn send_batch(
        &self,
        recipients: &[EmailRecipient],
        delay_ms: u64,
    ) -> Result<Vec<Result<MailgunResponse, String>>, Box<dyn std::error::Error + Send + Sync>>
    {
        let mut results = Vec::new();
        info!(
            "Starting batch send of {} emails with {}ms delays",
            recipients.len(),
            delay_ms
        );

        for (i, recipient) in recipients.iter().enumerate() {
            // Generate personalized subject line
            let subject = format!(
                "Exploring Your {} Project with FabInvest",
                recipient.repo_name
            );

            println!(
                "Sending email {}/{} to {} ({})",
                i + 1,
                recipients.len(),
                recipient.recipient_name,
                recipient.email
            );

            match self.send_email(recipient, &subject).await {
                Ok(response) => {
                    println!("✅ Sent to {}: {}", recipient.email, response.message);
                    results.push(Ok(response));
                }
                Err(e) => {
                    eprintln!("❌ Failed to send to {}: {}", recipient.email, e);
                    results.push(Err(e.to_string()));
                }
            }

            // Rate limiting between emails
            if i < recipients.len() - 1 {
                debug!("Waiting {}ms before next email...", delay_ms);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
            }
        }

        info!("Batch send complete. {} emails processed", results.len());
        Ok(results)
    }

    pub async fn test_connection(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/{}/stats", self.config.base_url, self.config.domain);

        debug!("Testing Mailgun connection: {}", url);

        let response = self
            .client
            .get(&url)
            .basic_auth("api", Some(&self.config.api_key))
            .send()
            .await?;

        if response.status().is_success() {
            info!("✅ Mailgun connection test successful");
            Ok(())
        } else {
            let error_text = response.text().await?;
            error!("❌ Mailgun connection test failed: {}", error_text);
            Err(format!("Mailgun connection failed: {}", error_text).into())
        }
    }
}

impl MailgunConfig {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Ok(MailgunConfig {
            api_key: std::env::var("MAILGUN_API_KEY")
                .map_err(|_| "MAILGUN_API_KEY environment variable required")?,
            domain: std::env::var("MAILGUN_DOMAIN")
                .unwrap_or_else(|_| "t.fabinvest.com".to_string()),
            from_email: std::env::var("FROM_EMAIL")
                .unwrap_or_else(|_| "support@t.fabinvest.com".to_string()),
            from_name: std::env::var("FROM_NAME").unwrap_or_else(|_| "Gilles Samyn".to_string()),
            template_name: std::env::var("MAILGUN_TEMPLATE")
                .unwrap_or_else(|_| "first message".to_string()),
            base_url: "https://api.mailgun.net/v3".to_string(),
        })
    }
}

// Helper functions for email processing
pub fn extract_repo_name_from_url(url: &str) -> String {
    if let Some(captures) = regex::Regex::new(r"github\.com/([^/]+/[^/?#]+)")
        .unwrap()
        .captures(url)
    {
        captures.get(1).unwrap().as_str().to_string()
    } else {
        "your project".to_string()
    }
}

pub fn generate_specific_aspect(commits: Option<i32>, description: &Option<String>) -> String {
    let commits = commits.unwrap_or(0);
    let desc = description.as_deref().unwrap_or("").to_lowercase();

    // Generate personalized aspects based on their work
    if desc.contains("ai") || desc.contains("machine learning") || desc.contains("neural") {
        if commits > 50 {
            format!(
                "your innovative AI work and {} commits showing deep technical expertise",
                commits
            )
        } else {
            "your cutting-edge artificial intelligence development".to_string()
        }
    } else if desc.contains("blockchain")
        || desc.contains("web3")
        || desc.contains("crypto")
        || desc.contains("defi")
    {
        if commits > 50 {
            format!("your blockchain development skills and {} contributions to the decentralized ecosystem", commits)
        } else {
            "your pioneering work in blockchain technology".to_string()
        }
    } else if desc.contains("fintech") || desc.contains("payment") || desc.contains("banking") {
        if commits > 50 {
            format!(
                "your fintech expertise and {} commits demonstrating payment innovation",
                commits
            )
        } else {
            "your innovative approach to financial technology".to_string()
        }
    } else if desc.contains("rust") {
        if commits > 50 {
            format!("your Rust development expertise and {} commits showing systems programming mastery", commits)
        } else {
            "your systems programming expertise in Rust".to_string()
        }
    } else if desc.contains("javascript") || desc.contains("react") || desc.contains("node") {
        if commits > 50 {
            format!(
                "your JavaScript development skills and {} commits in modern web technologies",
                commits
            )
        } else {
            "your expertise in modern web development".to_string()
        }
    } else if desc.contains("python") {
        if commits > 50 {
            format!("your Python development work and {} commits demonstrating versatile programming skills", commits)
        } else {
            "your Python development expertise".to_string()
        }
    } else if commits > 200 {
        format!("your exceptionally prolific development work with {} commits showing extraordinary dedication", commits)
    } else if commits > 100 {
        format!(
            "your prolific development work with {} commits showing exceptional dedication",
            commits
        )
    } else if commits > 20 {
        format!(
            "your consistent contributions with {} commits demonstrating strong technical skills",
            commits
        )
    } else if commits > 5 {
        format!(
            "your meaningful contributions with {} commits showing genuine technical involvement",
            commits
        )
    } else {
        "your technical expertise and innovative approach to development".to_string()
    }
}
