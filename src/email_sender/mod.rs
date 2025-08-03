// src/email_sender/mod.rs - COMPLETE REPLACEMENT
use crate::database::DbPool;
use chrono::Utc;
use reqwest::Client;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::{debug, error};

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

#[derive(Debug, Clone, Copy)]
pub enum EmailTemplate {
    InvestmentProposal,
    FollowUp,
}

impl EmailTemplate {
    pub fn mailgun_name(&self) -> &'static str {
        match self {
            EmailTemplate::InvestmentProposal => "second_round",
            EmailTemplate::FollowUp => "follo-up",
        }
    }

    pub fn db_name(&self) -> &'static str {
        match self {
            EmailTemplate::InvestmentProposal => "investment_proposal",
            EmailTemplate::FollowUp => "follow_up",
        }
    }
}

#[derive(Debug)]
pub struct EmailStatus {
    pub can_send_first: bool,
    pub can_send_followup: bool,
    pub last_sent: Option<String>,
    pub templates_sent: Vec<String>,
}

pub struct MailgunSender {
    pub config: MailgunConfig,
    pub client: Client,
}

impl MailgunSender {
    pub fn new(config: MailgunConfig) -> Self {
        let client = Client::new();
        debug!("Created MailgunSender for domain: {}", config.domain);
        Self { config, client }
    }

    // Check if we can send to this email
    pub async fn check_email_status(
        &self,
        db_pool: &DbPool,
        email: &str,
    ) -> Result<EmailStatus, Box<dyn std::error::Error + Send + Sync>> {
        let conn = db_pool.get().await?;

        let mut stmt = conn.prepare(
            "SELECT template_name, sent_at FROM email_tracking WHERE email = ? ORDER BY sent_at DESC"
        )?;

        let rows = stmt.query_map([email], |row| {
            Ok((
                row.get::<_, String>(0)?, // template_name
                row.get::<_, String>(1)?, // sent_at
            ))
        })?;

        let mut templates_sent = Vec::new();
        let mut last_sent = None;

        for row in rows {
            let (template, sent_at) = row?;
            templates_sent.push(template);
            if last_sent.is_none() {
                last_sent = Some(sent_at);
            }
        }

        let can_send_first = !templates_sent.contains(&"investment_proposal".to_string());
        let can_send_followup = templates_sent.contains(&"investment_proposal".to_string())
            && !templates_sent.contains(&"follow_up".to_string());

        Ok(EmailStatus {
            can_send_first,
            can_send_followup,
            last_sent,
            templates_sent,
        })
    }

    // Get candidates for follow-up emails
    pub async fn get_followup_candidates(
        &self,
        db_pool: &DbPool,
        days_since_first: i64,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let conn = db_pool.get().await?;
        let cutoff_date =
            (chrono::Utc::now() - chrono::Duration::days(days_since_first)).to_rfc3339();

        let mut stmt = conn.prepare(
            r#"
            SELECT DISTINCT email FROM email_tracking 
            WHERE template_name = 'investment_proposal' 
            AND sent_at <= ?1
            AND email NOT IN (
                SELECT email FROM email_tracking WHERE template_name = 'follow_up'
            )
            ORDER BY sent_at ASC
            "#,
        )?;

        let rows = stmt.query_map([cutoff_date], |row| Ok(row.get::<_, String>(0)?))?;

        let mut emails = Vec::new();
        for row in rows {
            emails.push(row?);
        }

        Ok(emails)
    }

    // Enhanced send method with tracking
    pub async fn send_email_with_tracking(
        &self,
        db_pool: &DbPool,
        recipient: &EmailRecipient,
        template: EmailTemplate,
        campaign_type: &str,
    ) -> Result<MailgunResponse, Box<dyn std::error::Error + Send + Sync>> {
        // Check if we've already sent this template
        let status = self.check_email_status(db_pool, &recipient.email).await?;

        match template {
            EmailTemplate::InvestmentProposal if !status.can_send_first => {
                return Err("Already sent investment proposal to this email".into());
            }
            EmailTemplate::FollowUp if !status.can_send_followup => {
                return Err(
                    "Cannot send follow-up: no investment proposal sent or follow-up already sent"
                        .into(),
                );
            }
            _ => {}
        }

        // Generate subject based on template
        let subject = match template {
            EmailTemplate::InvestmentProposal => {
                format!(
                    "Exploring Your {} Project with FabInvest",
                    recipient.repo_name
                )
            }
            EmailTemplate::FollowUp => {
                format!("Following Up on {} - FabInvest", recipient.repo_name)
            }
        };

        // Update config to use the correct template
        let mut config = self.config.clone();
        config.template_name = template.mailgun_name().to_string();
        let sender_with_template = MailgunSender::new(config);

        // Send the email
        let response = sender_with_template.send_email(recipient, &subject).await?;

        // Track the sent email
        self.track_sent_email(
            db_pool,
            &recipient.email,
            template.db_name(),
            campaign_type,
            &response.id,
        )
        .await?;

        Ok(response)
    }

    // Track sent email in database
    async fn track_sent_email(
        &self,
        db_pool: &DbPool,
        email: &str,
        template_name: &str,
        campaign_type: &str,
        mailgun_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let conn = db_pool.get().await?;
        let now = Utc::now().to_rfc3339();

        conn.execute(
            r#"
            INSERT INTO email_tracking (email, template_name, sent_at, campaign_type, mailgun_id)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT (email, template_name) DO UPDATE SET
                sent_at = excluded.sent_at,
                campaign_type = excluded.campaign_type,
                mailgun_id = excluded.mailgun_id
            "#,
            params![email, template_name, now, campaign_type, mailgun_id],
        )?;

        Ok(())
    }

    // Original send_email method (keep for compatibility)
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

    // pub async fn send_batch(
    //     &self,
    //     recipients: &[EmailRecipient],
    //     delay_ms: u64,
    // ) -> Result<Vec<Result<MailgunResponse, String>>, Box<dyn std::error::Error + Send + Sync>>
    // {
    //     let mut results = Vec::new();
    //     info!(
    //         "Starting batch send of {} emails with {}ms delays",
    //         recipients.len(),
    //         delay_ms
    //     );
    //
    //     for (i, recipient) in recipients.iter().enumerate() {
    //         // Generate personalized subject line
    //         let subject = format!(
    //             "Exploring Your {} Project with FabInvest",
    //             recipient.repo_name
    //         );
    //
    //         println!(
    //             "Sending email {}/{} to {} ({})",
    //             i + 1,
    //             recipients.len(),
    //             recipient.recipient_name,
    //             recipient.email
    //         );
    //
    //         match self.send_email(recipient, &subject).await {
    //             Ok(response) => {
    //                 println!("✅ Sent to {}: {}", recipient.email, response.message);
    //                 results.push(Ok(response));
    //             }
    //             Err(e) => {
    //                 eprintln!("❌ Failed to send to {}: {}", recipient.email, e);
    //                 results.push(Err(e.to_string()));
    //             }
    //         }
    //
    //         // Rate limiting between emails
    //         if i < recipients.len() - 1 {
    //             debug!("Waiting {}ms before next email...", delay_ms);
    //             tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
    //         }
    //     }
    //
    //     info!("Batch send complete. {} emails processed", results.len());
    //     Ok(results)
    // }

    // pub async fn test_connection(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    //     let url = format!("{}/{}/stats", self.config.base_url, self.config.domain);
    //
    //     debug!("Testing Mailgun connection: {}", url);
    //
    //     let response = self
    //         .client
    //         .get(&url)
    //         .basic_auth("api", Some(&self.config.api_key))
    //         .send()
    //         .await?;
    //
    //     if response.status().is_success() {
    //         info!("✅ Mailgun connection test successful");
    //         Ok(())
    //     } else {
    //         let error_text = response.text().await?;
    //         error!("❌ Mailgun connection test failed: {}", error_text);
    //         Err(format!("Mailgun connection failed: {}", error_text).into())
    //     }
    // }
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

// Helper functions for email processing (keep existing ones)
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
                "your innovative AI work and commits showing deep technical expertise",
                // commits
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
            format!("your blockchain development skills and contributions to the decentralized ecosystem")
        } else {
            "your pioneering work in blockchain technology".to_string()
        }
    } else if desc.contains("fintech") || desc.contains("payment") || desc.contains("banking") {
        if commits > 50 {
            format!(
                "your fintech expertise and commits demonstrating payment innovation",
                // commits
            )
        } else {
            "your innovative approach to financial technology".to_string()
        }
    } else if desc.contains("rust") {
        if commits > 50 {
            format!(
                "your Rust development expertise and commits showing systems programming mastery"
            )
        } else {
            "your systems programming expertise in Rust".to_string()
        }
    } else if desc.contains("javascript") || desc.contains("react") || desc.contains("node") {
        if commits > 50 {
            format!(
                "your JavaScript development skills and commits in modern web technologies",
                // commits
            )
        } else {
            "your expertise in modern web development".to_string()
        }
    } else if desc.contains("python") {
        if commits > 50 {
            format!("your Python development work and commits demonstrating versatile programming skills")
        } else {
            "your Python development expertise".to_string()
        }
    } else if commits > 200 {
        format!("your exceptionally prolific development work showing extraordinary dedication")
    } else if commits > 100 {
        format!(
            "your prolific development work showing exceptional dedication",
            // commits
        )
    } else if commits > 20 {
        format!(
            "your consistent contributions demonstrating strong technical skills",
            // commits
        )
    } else if commits > 5 {
        format!(
            "your meaningful contributions showing genuine technical involvement",
            // commits
        )
    } else {
        "your technical expertise and innovative approach to development".to_string()
    }
}

impl MailgunSender {
    // Enhanced send method with debug mode support
    // pub async fn send_email_with_debug(
    //     &self,
    //     db_pool: &DbPool,
    //     recipient: &EmailRecipient,
    //     template: EmailTemplate,
    //     campaign_type: &str,
    //     debug_config: &EmailDebugConfig,
    // ) -> Result<MailgunResponse, Box<dyn std::error::Error + Send + Sync>> {
    //     // In debug mode, skip the duplicate check
    //     if !debug_config.enabled {
    //         // Normal mode: Check if we've already sent this template
    //         let status = self.check_email_status(db_pool, &recipient.email).await?;
    //
    //         match template {
    //             EmailTemplate::InvestmentProposal if !status.can_send_first => {
    //                 return Err("Already sent investment proposal to this email".into());
    //             }
    //             EmailTemplate::FollowUp if !status.can_send_followup => {
    //                 return Err("Cannot send follow-up: no investment proposal sent or follow-up already sent".into());
    //             }
    //             _ => {}
    //         }
    //     }
    //
    //     // Create debug recipient if debug mode is enabled
    //     let actual_recipient = if debug_config.enabled {
    //         EmailRecipient {
    //             email: debug_config.debug_email.clone(),
    //             recipient_name: format!("DEBUG: {}", recipient.recipient_name),
    //             repo_name: recipient.repo_name.clone(),
    //             specific_aspect: recipient.specific_aspect.clone(),
    //             contact_email: recipient.contact_email.clone(),
    //             contact_phone: recipient.contact_phone.clone(),
    //             engagement_score: recipient.engagement_score,
    //             domain_category: recipient.domain_category.clone(),
    //             company_size: recipient.company_size.clone(),
    //         }
    //     } else {
    //         recipient.clone()
    //     };
    //
    //     // Generate subject with debug prefix if needed
    //     let subject = match template {
    //         EmailTemplate::InvestmentProposal => {
    //             if debug_config.enabled {
    //                 format!(
    //                     "[DEBUG for {}] Exploring Your {} Project with FabInvest",
    //                     recipient.email, recipient.repo_name
    //                 )
    //             } else {
    //                 format!(
    //                     "Exploring Your {} Project with FabInvest",
    //                     recipient.repo_name
    //                 )
    //             }
    //         }
    //         EmailTemplate::FollowUp => {
    //             if debug_config.enabled {
    //                 format!(
    //                     "[DEBUG for {}] Following Up on {} - FabInvest",
    //                     recipient.email, recipient.repo_name
    //                 )
    //             } else {
    //                 format!("Following Up on {} - FabInvest", recipient.repo_name)
    //             }
    //         }
    //     };
    //
    //     // Update config to use the correct template
    //     let mut config = self.config.clone();
    //     config.template_name = template.mailgun_name().to_string();
    //     let sender_with_template = MailgunSender::new(config);
    //
    //     // Send the email with debug recipient
    //     let response = sender_with_template
    //         .send_email_with_debug_variables(
    //             &actual_recipient,
    //             &subject,
    //             recipient,
    //             debug_config.enabled,
    //         )
    //         .await?;
    //
    //     // Track the sent email only if not in debug mode or if tracking is not skipped
    //     if !debug_config.enabled || !debug_config.skip_tracking {
    //         self.track_sent_email(
    //             db_pool,
    //             &recipient.email, // Always track the original email, not debug email
    //             template.db_name(),
    //             &if debug_config.enabled {
    //                 format!("debug_{}", campaign_type)
    //             } else {
    //                 campaign_type.to_string()
    //             },
    //             &response.id,
    //         )
    //         .await?;
    //     }
    //
    //     Ok(response)
    // }

    // Enhanced send_email method with debug variable support
    // pub async fn send_email_with_debug_variables(
    //     &self,
    //     recipient: &EmailRecipient,
    //     subject: &str,
    //     original_recipient: &EmailRecipient, // Original recipient data for template variables
    //     is_debug: bool,
    // ) -> Result<MailgunResponse, Box<dyn std::error::Error + Send + Sync>> {
    //     let url = format!("{}/{}/messages", self.config.base_url, self.config.domain);
    //
    //     debug!("Preparing email for {}: {}", recipient.email, subject);
    //
    //     // Create Mailgun variables JSON with original recipient data and debug info
    //     let variables = if is_debug {
    //         json!({
    //             "recipient_name": original_recipient.recipient_name,
    //             "repo_name": original_recipient.repo_name,
    //             "specific_aspect": original_recipient.specific_aspect,
    //             "contact_email": original_recipient.contact_email,
    //             "contact_phone": original_recipient.contact_phone,
    //             "debug_original_email": original_recipient.email,
    //             "debug_mode": "This is a DEBUG email. Original recipient: ".to_string() + &original_recipient.email
    //         })
    //     } else {
    //         json!({
    //             "recipient_name": recipient.recipient_name,
    //             "repo_name": recipient.repo_name,
    //             "specific_aspect": recipient.specific_aspect,
    //             "contact_email": recipient.contact_email,
    //             "contact_phone": recipient.contact_phone
    //         })
    //     };
    //
    //     debug!("Template variables: {}", variables);
    //
    //     let mut form_data = HashMap::new();
    //     form_data.insert(
    //         "from",
    //         format!("{} <{}>", self.config.from_name, self.config.from_email),
    //     );
    //     form_data.insert(
    //         "to",
    //         format!("{} <{}>", recipient.recipient_name, recipient.email),
    //     );
    //     form_data.insert("subject", subject.to_string());
    //     form_data.insert("template", self.config.template_name.clone());
    //     form_data.insert("h:X-Mailgun-Variables", variables.to_string());
    //
    //     // Add tracking parameters
    //     form_data.insert("o:tracking", "yes".to_string());
    //     form_data.insert("o:tracking-clicks", "yes".to_string());
    //     form_data.insert("o:tracking-opens", "yes".to_string());
    //     form_data.insert("o:tracking-unsubscribes", "no".to_string());
    //
    //     // Add custom tags for analytics (with debug prefix if needed)
    //     let tag_prefix = if is_debug { "debug-" } else { "" };
    //     form_data.insert(
    //         "o:tag",
    //         format!(
    //             "{}campaign-{}",
    //             tag_prefix,
    //             chrono::Utc::now().format("%Y-%m")
    //         ),
    //     );
    //     form_data.insert(
    //         "o:tag",
    //         format!(
    //             "{}category-{}",
    //             tag_prefix, original_recipient.domain_category
    //         ),
    //     );
    //     form_data.insert(
    //         "o:tag",
    //         format!(
    //             "{}score-{}",
    //             tag_prefix, original_recipient.engagement_score
    //         ),
    //     );
    //
    //     debug!("Sending POST request to: {}", url);
    //
    //     let response = self
    //         .client
    //         .post(&url)
    //         .basic_auth("api", Some(&self.config.api_key))
    //         .form(&form_data)
    //         .send()
    //         .await?;
    //
    //     debug!("Mailgun response status: {}", response.status());
    //
    //     if response.status().is_success() {
    //         let mailgun_response: MailgunResponse = response.json().await?;
    //         debug!("Mailgun success response: {:?}", mailgun_response);
    //         Ok(mailgun_response)
    //     } else {
    //         let error_text = response.text().await?;
    //         error!("Mailgun API error: {}", error_text);
    //         Err(format!("Mailgun error: {}", error_text).into())
    //     }
    // }
}
