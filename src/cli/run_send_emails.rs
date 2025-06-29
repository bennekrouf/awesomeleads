// src/cli/run_send_emails.rs - COMPLETE VERSION WITH INTEGRATED DEBUG MODE
use crate::email_export::{EmailDatabase, EmailExportConfigBuilder, EmailProcessor};
use crate::email_rate_limiting::{EmailRateLimiter, RateLimitStatus};
use crate::email_sender::{
    extract_repo_name_from_url, generate_specific_aspect, EmailRecipient, EmailTemplate,
    MailgunConfig, MailgunSender,
};
use crate::models::CliApp;
use chrono::Utc;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use tracing::{debug, error};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

// Debug configuration struct
#[derive(Debug, Clone)]
pub struct EmailDebugConfig {
    pub enabled: bool,
    pub debug_email: String,
    pub skip_tracking: bool,
}

impl EmailDebugConfig {
    pub fn from_env() -> Self {
        // Force reload .env to ensure we have latest values
        let _ = dotenv::dotenv();
        // More robust environment variable checking
        let debug_enabled = std::env::var("EMAIL_DEBUG_MODE")
            .map(|v| {
                let val = v.to_lowercase();
                val == "true" || val == "1" || val == "yes" || val == "on"
            })
            .unwrap_or(false);

        let debug_email = std::env::var("EMAIL_DEBUG_ADDRESS")
            .unwrap_or_else(|_| "mohamed.bennekrouf@gmail.com".to_string());

        let skip_tracking = std::env::var("EMAIL_DEBUG_SKIP_TRACKING")
            .map(|v| {
                let val = v.to_lowercase();
                val == "true" || val == "1" || val == "yes" || val == "on"
            })
            .unwrap_or(true);

        // Log the configuration for debugging
        if debug_enabled {
            println!("🐛 DEBUG MODE CONFIGURATION:");
            println!(
                "   EMAIL_DEBUG_MODE = {}",
                std::env::var("EMAIL_DEBUG_MODE").unwrap_or_default()
            );
            println!("   EMAIL_DEBUG_ADDRESS = {}", debug_email);
            println!(
                "   EMAIL_DEBUG_SKIP_TRACKING = {}",
                std::env::var("EMAIL_DEBUG_SKIP_TRACKING").unwrap_or_default()
            );
        }

        Self {
            enabled: debug_enabled,
            debug_email,
            skip_tracking,
        }
    }

    // Helper method to print current environment state
    pub fn print_env_debug() {
        println!("🔍 ENVIRONMENT VARIABLES DEBUG:");

        match std::env::var("EMAIL_DEBUG_MODE") {
            Ok(val) => println!("   EMAIL_DEBUG_MODE = '{}' (found)", val),
            Err(_) => println!("   EMAIL_DEBUG_MODE = not set"),
        }

        match std::env::var("EMAIL_DEBUG_ADDRESS") {
            Ok(val) => println!("   EMAIL_DEBUG_ADDRESS = '{}' (found)", val),
            Err(_) => println!("   EMAIL_DEBUG_ADDRESS = not set"),
        }

        match std::env::var("EMAIL_DEBUG_SKIP_TRACKING") {
            Ok(val) => println!("   EMAIL_DEBUG_SKIP_TRACKING = '{}' (found)", val),
            Err(_) => println!("   EMAIL_DEBUG_SKIP_TRACKING = not set"),
        }

        // Also check if .env file exists
        if std::path::Path::new(".env").exists() {
            println!("   .env file: EXISTS");
            if let Ok(contents) = std::fs::read_to_string(".env") {
                println!("   .env contents (EMAIL_DEBUG related):");
                for line in contents.lines() {
                    if line.contains("EMAIL_DEBUG") || line.contains("DEBUG") {
                        println!("     {}", line);
                    }
                }
            }
        } else {
            println!("   .env file: NOT FOUND");
        }
    }
}

impl CliApp {
    pub async fn send_emails_via_mailgun(&self) -> Result<()> {
        // Force reload .env at the start
        let _ = dotenv::dotenv();
        println!("\n📧 Smart Email Campaign System");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Enhanced debug configuration loading with diagnostics
        println!("🔍 Checking debug configuration...");
        EmailDebugConfig::print_env_debug();

        let debug_config = EmailDebugConfig::from_env();

        if debug_config.enabled {
            println!("\n🐛 ✅ DEBUG MODE IS ENABLED");
            println!(
                "   📧 All emails will be redirected to: {}",
                debug_config.debug_email
            );
            println!("   📊 Tracking disabled: {}", debug_config.skip_tracking);
            println!("   💡 To disable: set EMAIL_DEBUG_MODE=false or remove from .env");
        } else {
            println!("\n🚀 ✅ PRODUCTION MODE");
            println!("   📧 Emails will be sent to actual recipients");
            println!("   📊 Full tracking enabled");
            println!("   💡 To enable debug: set EMAIL_DEBUG_MODE=true in .env");
        }

        // Continue with the rest of your existing logic...
        let mailgun_config = MailgunConfig::from_env().map_err(|e| {
            println!("❌ Mailgun configuration error: {}", e);
            e
        })?;

        let sender = MailgunSender::new(mailgun_config);
        // Campaign type selection with debug options
        let mut campaign_options = vec![
            "🎯 First Contact Campaign (Investment Proposals)",
            "📬 Follow-up Campaign (Second Touch)",
            "📊 Show Email Statistics",
            "🔍 Check Specific Email Status",
        ];

        // Add debug options if debug mode is enabled
        if debug_config.enabled {
            campaign_options.insert(0, "🐛 Debug: Test Single Email");
            campaign_options.insert(1, "🐛 Debug: Test Small Batch (5 emails)");
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!(
                "Select campaign type {}",
                if debug_config.enabled {
                    "(DEBUG MODE)"
                } else {
                    ""
                }
            ))
            .items(&campaign_options)
            .interact()?;

        // Handle debug-specific options
        if debug_config.enabled && selection < 2 {
            match selection {
                0 => self.run_debug_single_email(&sender, &debug_config).await?,
                1 => self.run_debug_batch(&sender, &debug_config).await?,
                _ => {}
            }
            return Ok(());
        }

        // Handle regular options (adjust index if debug options were added)
        let adjusted_selection = if debug_config.enabled {
            selection - 2
        } else {
            selection
        };
        match adjusted_selection {
            0 => {
                self.run_first_contact_campaign(&sender, &debug_config)
                    .await?
            }
            1 => self.run_followup_campaign(&sender, &debug_config).await?,
            2 => self.show_email_statistics(&sender).await?,
            3 => self.check_email_status(&sender).await?,
            _ => return Ok(()),
        }

        Ok(())
    }

    pub async fn debug_environment_check(&self) -> Result<()> {
        println!("\n🔍 Environment Debug Check");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        EmailDebugConfig::print_env_debug();

        let debug_config = EmailDebugConfig::from_env();
        println!("\n📊 Parsed Configuration:");
        println!("   Debug Mode: {}", debug_config.enabled);
        println!("   Debug Email: {}", debug_config.debug_email);
        println!("   Skip Tracking: {}", debug_config.skip_tracking);

        Ok(())
    }

    async fn run_debug_single_email(
        &self,
        sender: &MailgunSender,
        debug_config: &EmailDebugConfig,
    ) -> Result<()> {
        println!("\n🐛 Debug: Single Email Test");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let recipients = self.load_email_recipients_for_campaign(0).await?;
        if recipients.is_empty() {
            println!("❌ No recipients found");
            return Ok(());
        }

        let recipient = &recipients[0];
        println!(
            "📧 Testing with: {} ({})",
            recipient.recipient_name, recipient.email
        );
        println!("📦 Project: {}", recipient.repo_name);

        let template_options = vec!["Investment Proposal Template", "Follow-up Template"];

        let template_selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select template to test")
            .items(&template_options)
            .interact()?;

        let template = match template_selection {
            0 => EmailTemplate::InvestmentProposal,
            1 => EmailTemplate::FollowUp,
            _ => return Ok(()),
        };

        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!(
                "Send debug email to {}?",
                debug_config.debug_email
            ))
            .interact()?
        {
            return Ok(());
        }

        match self
            .send_single_email_with_debug(sender, recipient, template, "debug_test", debug_config)
            .await
        {
            Ok(response) => {
                println!("✅ Debug email sent successfully!");
                println!("   📨 Mailgun ID: {}", response.id);
                println!("   📧 Sent to: {}", debug_config.debug_email);
                println!("   👤 Original recipient: {}", recipient.email);
            }
            Err(e) => {
                println!("❌ Failed to send debug email: {}", e);
            }
        }

        Ok(())
    }

    async fn run_debug_batch(
        &self,
        sender: &MailgunSender,
        debug_config: &EmailDebugConfig,
    ) -> Result<()> {
        println!("\n🐛 Debug: Small Batch Test (5 emails)");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let recipients = self.load_email_recipients_for_campaign(0).await?;
        let batch = recipients.into_iter().take(5).collect::<Vec<_>>();

        if batch.is_empty() {
            println!("❌ No recipients found");
            return Ok(());
        }

        println!("📋 Debug batch preview:");
        for (i, recipient) in batch.iter().enumerate() {
            println!(
                "  {}. {} ({})",
                i + 1,
                recipient.recipient_name,
                recipient.email
            );
        }

        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!(
                "Send {} debug emails to {}?",
                batch.len(),
                debug_config.debug_email
            ))
            .interact()?
        {
            return Ok(());
        }

        self.send_campaign_batch(
            &sender,
            &batch,
            EmailTemplate::InvestmentProposal,
            "debug_batch",
            debug_config,
        )
        .await
    }

    async fn run_first_contact_campaign(
        &self,
        sender: &MailgunSender,
        debug_config: &EmailDebugConfig,
    ) -> Result<()> {
        println!("\n🎯 First Contact Campaign");
        if debug_config.enabled {
            println!("🐛 Running in DEBUG MODE - rate limits bypassed");
        }
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Check rate limits upfront in production mode
        if !debug_config.enabled {
            let rate_limiter =
                EmailRateLimiter::new(self.config.email_limits.clone(), self.db_pool.clone());

            let status = rate_limiter.check_rate_limits(100).await?; // Check for reasonable batch
            rate_limiter.display_status(&status);

            if !status.can_send {
                println!("\n❌ Cannot start campaign due to rate limits");
                return Ok(());
            }
        }

        let all_recipients = self.load_email_recipients_for_campaign(0).await?;
        let first_contact_candidates = if debug_config.enabled {
            all_recipients
        } else {
            println!("🔍 Filtering candidates who haven't received first contact...");
            let mut candidates = Vec::new();
            for recipient in all_recipients {
                let status = sender
                    .check_email_status(&self.db_pool, &recipient.email)
                    .await?;
                if status.can_send_first {
                    candidates.push(recipient);
                }
            }
            candidates
        };

        if first_contact_candidates.is_empty() {
            println!("✅ All eligible recipients have already received first contact emails!");
            return Ok(());
        }

        println!(
            "📋 Found {} candidates for first contact",
            first_contact_candidates.len()
        );
        self.show_campaign_preview(&first_contact_candidates);

        // Suggest safe batch size based on rate limits
        let suggested_batch = if debug_config.enabled {
            3
        } else {
            let rate_limiter =
                EmailRateLimiter::new(self.config.email_limits.clone(), self.db_pool.clone());
            let status = rate_limiter
                .check_rate_limits(first_contact_candidates.len())
                .await?;
            status
                .recommended_batch_size
                .min(first_contact_candidates.len())
        };

        let batch_size: usize = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("How many first contact emails to send?")
            .default(suggested_batch)
            .interact_text()?;

        let batch = first_contact_candidates
            .into_iter()
            .take(batch_size)
            .collect::<Vec<_>>();

        let prompt = if debug_config.enabled {
            format!(
                "Send {} DEBUG investment proposal emails to {}?",
                batch.len(),
                debug_config.debug_email
            )
        } else {
            format!(
                "Send {} investment proposal emails with rate limiting?",
                batch.len()
            )
        };

        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&prompt)
            .interact()?
        {
            return Ok(());
        }

        self.send_campaign_batch(
            &sender,
            &batch,
            EmailTemplate::InvestmentProposal,
            "first_contact",
            debug_config,
        )
        .await
    }

    async fn run_followup_campaign(
        &self,
        sender: &MailgunSender,
        debug_config: &EmailDebugConfig,
    ) -> Result<()> {
        println!("\n📬 Follow-up Campaign");
        if debug_config.enabled {
            println!(
                "🐛 Running in DEBUG MODE - all emails go to {}",
                debug_config.debug_email
            );
        }
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let followup_emails = if debug_config.enabled {
            // In debug mode, get some sample emails for testing
            let all_recipients = self.load_email_recipients_for_campaign(0).await?;
            all_recipients
                .into_iter()
                .take(5)
                .map(|r| r.email)
                .collect()
        } else {
            let days_since_first: i64 = Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Minimum days since first email")
                .default(7)
                .interact_text()?;
            sender
                .get_followup_candidates(&self.db_pool, days_since_first)
                .await?
        };

        if followup_emails.is_empty() {
            let msg = if debug_config.enabled {
                "No sample recipients found for debug mode"
            } else {
                "No follow-up candidates found"
            };
            println!("📭 {}", msg);
            return Ok(());
        }

        println!(
            "📋 Found {} candidates for follow-up",
            followup_emails.len()
        );

        let mut followup_recipients = Vec::new();
        for email in &followup_emails {
            if let Some(recipient) = self.find_recipient_by_email(email).await? {
                followup_recipients.push(recipient);
            }
        }

        if followup_recipients.is_empty() {
            println!("❌ Could not find recipient data for follow-up emails");
            return Ok(());
        }

        let default_batch = if debug_config.enabled {
            3
        } else {
            followup_recipients.len().min(25)
        };
        let batch_size: usize = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("How many follow-up emails to send?")
            .default(default_batch)
            .interact_text()?;

        let batch = followup_recipients
            .into_iter()
            .take(batch_size)
            .collect::<Vec<_>>();

        let prompt = if debug_config.enabled {
            format!(
                "Send {} DEBUG follow-up emails to {}?",
                batch.len(),
                debug_config.debug_email
            )
        } else {
            format!("Send {} follow-up emails?", batch.len())
        };

        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&prompt)
            .interact()?
        {
            return Ok(());
        }

        self.send_campaign_batch(
            &sender,
            &batch,
            EmailTemplate::FollowUp,
            "follow_up",
            debug_config,
        )
        .await
    }

    async fn send_campaign_batch(
        &self,
        sender: &MailgunSender,
        recipients: &[EmailRecipient],
        template: EmailTemplate,
        campaign_type: &str,
        debug_config: &EmailDebugConfig,
    ) -> Result<()> {
        // Create rate limiter
        let rate_limiter =
            EmailRateLimiter::new(self.config.email_limits.clone(), self.db_pool.clone());

        // Check rate limits before starting
        let status = rate_limiter.check_rate_limits(recipients.len()).await?;
        rate_limiter.display_status(&status);

        if !status.can_send && !debug_config.enabled {
            println!("\n❌ Cannot send emails due to rate limits");
            println!(
                "💡 Try again later or reduce batch size to {}",
                status.recommended_batch_size
            );
            return Ok(());
        }

        // In production mode, enforce limits
        let actual_batch_size = if debug_config.enabled {
            recipients.len() // Debug mode ignores limits
        } else {
            recipients.len().min(status.recommended_batch_size)
        };

        if actual_batch_size < recipients.len() {
            println!(
                "\n⚠️  Reducing batch size from {} to {} due to rate limits",
                recipients.len(),
                actual_batch_size
            );

            if !Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(&format!(
                    "Continue with reduced batch size of {}?",
                    actual_batch_size
                ))
                .interact()?
            {
                return Ok(());
            }
        }

        // Ask for confirmation if sending many emails
        if actual_batch_size >= self.config.email_limits.require_confirmation_above
            && !debug_config.enabled
        {
            println!("\n⚠️  You're about to send {} emails", actual_batch_size);
            println!(
                "🛡️  This is above the confirmation threshold of {}",
                self.config.email_limits.require_confirmation_above
            );

            if !Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Are you sure you want to proceed?")
                .default(false)
                .interact()?
            {
                println!("❌ Campaign cancelled for safety");
                return Ok(());
            }
        }

        let final_batch = &recipients[..actual_batch_size];
        let mode_str = if debug_config.enabled { "DEBUG " } else { "" };

        println!("\n🚀 Starting {}email campaign...", mode_str);
        println!(
            "📧 Sending {} emails with smart rate limiting",
            final_batch.len()
        );

        let mut successful = 0;
        let mut failed = 0;
        let mut last_send_time = Utc::now();

        for (i, recipient) in final_batch.iter().enumerate() {
            let display_email = if debug_config.enabled {
                format!(
                    "{} (original: {})",
                    debug_config.debug_email, recipient.email
                )
            } else {
                recipient.email.clone()
            };

            println!(
                "[{}/{}] Sending to {} ({})",
                i + 1,
                final_batch.len(),
                recipient.recipient_name,
                display_email
            );

            match self
                .send_single_email_with_debug(
                    sender,
                    recipient,
                    template,
                    campaign_type,
                    debug_config,
                )
                .await
            {
                Ok(response) => {
                    println!("✅ Sent: {}", response.message);
                    successful += 1;
                }
                Err(e) => {
                    println!("❌ Failed: {}", e);
                    failed += 1;
                }
            }

            // Smart rate limiting with adaptive delays
            if i < final_batch.len() - 1 {
                let optimal_delay = if debug_config.enabled {
                    1000 // Fast in debug mode
                } else {
                    rate_limiter.get_optimal_delay().await
                };

                // Show progress every 10 emails
                if (i + 1) % 10 == 0 {
                    let elapsed = Utc::now().signed_duration_since(last_send_time);
                    let rate = 10.0 / elapsed.num_seconds() as f64 * 60.0; // emails per minute
                    println!(
                        "   📊 Rate: {:.1} emails/min | Next delay: {}ms",
                        rate, optimal_delay
                    );
                    last_send_time = Utc::now();
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(optimal_delay)).await;
            }
        }

        // Results summary with deliverability tips
        println!(
            "\n🎉 {}Campaign Complete!",
            if debug_config.enabled { "Debug " } else { "" }
        );
        println!("✅ Successful: {}", successful);
        println!("❌ Failed: {}", failed);

        if successful > 0 {
            let success_rate = (successful as f64 / final_batch.len() as f64) * 100.0;
            println!("📊 Success rate: {:.1}%", success_rate);

            if success_rate < 95.0 {
                println!("⚠️  Success rate is below 95%. Check your email list quality.");
            }
        }

        if debug_config.enabled && debug_config.skip_tracking {
            println!("💡 No tracking recorded (debug mode)");
        } else {
            // Update rate limiter status
            let final_status = rate_limiter.check_rate_limits(0).await?;
            println!("\n📈 Updated limits:");
            println!(
                "   Daily: {}/{} remaining",
                final_status.remaining_today, final_status.daily_limit
            );
            println!(
                "   Hourly: {}/{} used",
                final_status.hourly_sent, self.config.email_limits.emails_per_hour
            );
        }

        // Deliverability advice
        if !debug_config.enabled && successful > 50 {
            println!("\n💡 Deliverability Tips:");
            println!("   📊 Monitor your Mailgun dashboard for bounces/complaints");
            println!("   📧 Keep bounce rate < 5% and complaint rate < 0.1%");
            println!("   ⏰ Wait 24-48 hours before next large campaign");
            println!("   🎯 Clean your email list regularly");
        }

        Ok(())
    }

    async fn send_single_email_with_debug(
        &self,
        sender: &MailgunSender,
        recipient: &EmailRecipient,
        template: EmailTemplate,
        campaign_type: &str,
        debug_config: &EmailDebugConfig,
    ) -> Result<crate::email_sender::MailgunResponse> {
        // In debug mode, skip the duplicate check
        if !debug_config.enabled {
            // Normal mode: Check if we've already sent this template
            let status = sender
                .check_email_status(&self.db_pool, &recipient.email)
                .await?;

            match template {
                EmailTemplate::InvestmentProposal if !status.can_send_first => {
                    return Err("Already sent investment proposal to this email".into());
                }
                EmailTemplate::FollowUp if !status.can_send_followup => {
                    return Err("Cannot send follow-up: no investment proposal sent or follow-up already sent".into());
                }
                _ => {}
            }
        }

        // Create debug recipient if debug mode is enabled
        let actual_recipient = if debug_config.enabled {
            EmailRecipient {
                email: debug_config.debug_email.clone(),
                recipient_name: format!("DEBUG: {}", recipient.recipient_name),
                repo_name: recipient.repo_name.clone(),
                specific_aspect: recipient.specific_aspect.clone(),
                contact_email: recipient.contact_email.clone(),
                contact_phone: recipient.contact_phone.clone(),
                engagement_score: recipient.engagement_score,
                domain_category: recipient.domain_category.clone(),
                company_size: recipient.company_size.clone(),
            }
        } else {
            recipient.clone()
        };

        // Generate subject with debug prefix if needed
        let subject = match template {
            EmailTemplate::InvestmentProposal => {
                if debug_config.enabled {
                    format!(
                        "[DEBUG for {}] Exploring Your {} Project with FabInvest",
                        recipient.email, recipient.repo_name
                    )
                } else {
                    format!(
                        "Exploring Your {} Project with FabInvest",
                        recipient.repo_name
                    )
                }
            }
            EmailTemplate::FollowUp => {
                if debug_config.enabled {
                    format!(
                        "[DEBUG for {}] Following Up on {} - FabInvest",
                        recipient.email, recipient.repo_name
                    )
                } else {
                    format!("Following Up on {} - FabInvest", recipient.repo_name)
                }
            }
        };

        // Update config to use the correct template
        let mut config = sender.config.clone();
        config.template_name = template.mailgun_name().to_string();
        let sender_with_template = MailgunSender::new(config);

        // Send the email with enhanced variables for debug mode
        let response = if debug_config.enabled {
            self.send_email_with_debug_variables(
                &sender_with_template,
                &actual_recipient,
                &subject,
                recipient,
            )
            .await?
        } else {
            sender_with_template
                .send_email(&actual_recipient, &subject)
                .await?
        };

        // Track the sent email only if not in debug mode or if tracking is not skipped
        if !debug_config.enabled || !debug_config.skip_tracking {
            self.track_sent_email(
                &recipient.email, // Always track the original email, not debug email
                template.db_name(),
                &if debug_config.enabled {
                    format!("debug_{}", campaign_type)
                } else {
                    campaign_type.to_string()
                },
                &response.id,
            )
            .await?;
        }

        Ok(response)
    }

    async fn send_email_with_debug_variables(
        &self,
        sender: &MailgunSender,
        recipient: &EmailRecipient,
        subject: &str,
        original_recipient: &EmailRecipient,
    ) -> Result<crate::email_sender::MailgunResponse> {
        use serde_json::json;
        use std::collections::HashMap;

        let url = format!(
            "{}/{}/messages",
            sender.config.base_url, sender.config.domain
        );

        debug!("Preparing DEBUG email for {}: {}", recipient.email, subject);

        // Create Mailgun variables JSON with original recipient data and debug info
        let variables = json!({
            "recipient_name": original_recipient.recipient_name,
            "repo_name": original_recipient.repo_name,
            "specific_aspect": original_recipient.specific_aspect,
            "contact_email": original_recipient.contact_email,
            "contact_phone": original_recipient.contact_phone,
            "debug_original_email": original_recipient.email,
            "debug_mode": "This is a DEBUG email. Original recipient: ".to_string() + &original_recipient.email
        });

        debug!("Template variables: {}", variables);

        let mut form_data = HashMap::new();
        form_data.insert(
            "from",
            format!("{} <{}>", sender.config.from_name, sender.config.from_email),
        );
        form_data.insert(
            "to",
            format!("{} <{}>", recipient.recipient_name, recipient.email),
        );
        form_data.insert("subject", subject.to_string());
        form_data.insert("template", sender.config.template_name.clone());
        form_data.insert("h:X-Mailgun-Variables", variables.to_string());

        // Add tracking parameters
        form_data.insert("o:tracking", "yes".to_string());
        form_data.insert("o:tracking-clicks", "yes".to_string());
        form_data.insert("o:tracking-opens", "yes".to_string());

        // Add custom tags for analytics with debug prefix
        form_data.insert(
            "o:tag",
            format!("debug-campaign-{}", chrono::Utc::now().format("%Y-%m")),
        );
        form_data.insert(
            "o:tag",
            format!("debug-category-{}", original_recipient.domain_category),
        );
        form_data.insert(
            "o:tag",
            format!("debug-score-{}", original_recipient.engagement_score),
        );

        debug!("Sending POST request to: {}", url);

        let response = sender
            .client
            .post(&url)
            .basic_auth("api", Some(&sender.config.api_key))
            .form(&form_data)
            .send()
            .await?;

        debug!("Mailgun response status: {}", response.status());

        if response.status().is_success() {
            let mailgun_response: crate::email_sender::MailgunResponse = response.json().await?;
            debug!("Mailgun success response: {:?}", mailgun_response);
            Ok(mailgun_response)
        } else {
            let error_text = response.text().await?;
            error!("Mailgun API error: {}", error_text);
            Err(format!("Mailgun error: {}", error_text).into())
        }
    }

    async fn track_sent_email(
        &self,
        email: &str,
        template_name: &str,
        campaign_type: &str,
        mailgun_id: &str,
    ) -> Result<()> {
        use chrono::Utc;
        use rusqlite::params;

        let conn = self.db_pool.get().await?;
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

    async fn show_email_statistics(&self, sender: &MailgunSender) -> Result<()> {
        println!("\n📊 Email Campaign Statistics");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let conn = self.db_pool.get().await?;

        let total_first: i64 = conn.query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE template_name = 'investment_proposal'",
            [],
            |row| row.get(0),
        )?;

        let total_followup: i64 = conn.query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE template_name = 'follow_up'",
            [],
            |row| row.get(0),
        )?;

        let unique_contacts: i64 = conn.query_row(
            "SELECT COUNT(DISTINCT email) FROM email_tracking",
            [],
            |row| row.get(0),
        )?;

        let debug_emails: i64 = conn.query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE campaign_type LIKE 'debug_%'",
            [],
            |row| row.get(0),
        )?;

        println!("📧 Investment Proposals Sent: {}", total_first);
        println!("📬 Follow-ups Sent: {}", total_followup);
        println!("👥 Unique Contacts: {}", unique_contacts);
        println!("🐛 Debug Emails: {}", debug_emails);

        if total_first > 0 {
            let followup_rate = (total_followup as f64 / total_first as f64) * 100.0;
            println!("📈 Follow-up Rate: {:.1}%", followup_rate);
        }

        // Recent activity
        let recent: i64 = conn.query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE sent_at > ?",
            [&(chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339()],
            |row| row.get(0),
        )?;

        println!("🕐 Emails sent in last 7 days: {}", recent);

        Ok(())
    }

    async fn check_email_status(&self, sender: &MailgunSender) -> Result<()> {
        let email: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter email to check")
            .interact_text()?;

        let status = sender.check_email_status(&self.db_pool, &email).await?;

        println!("\n📋 Status for {}", email);
        println!("🎯 Can send first email: {}", status.can_send_first);
        println!("📬 Can send follow-up: {}", status.can_send_followup);

        if let Some(last_sent) = status.last_sent {
            println!("🕐 Last email sent: {}", last_sent);
        }

        if !status.templates_sent.is_empty() {
            println!("📨 Templates sent: {}", status.templates_sent.join(", "));
        }

        Ok(())
    }

    // FROM YOUR ORIGINAL CODE - Load email recipients for campaign
    async fn load_email_recipients_for_campaign(
        &self,
        campaign_type: usize,
    ) -> Result<Vec<EmailRecipient>> {
        debug!("Loading recipients for campaign type: {}", campaign_type);

        // Use your existing email export logic
        let config_builder = EmailExportConfigBuilder::new();
        let export_config = config_builder.build_config(campaign_type).await?;

        let database = EmailDatabase::new(self.db_pool.clone());
        let processor = EmailProcessor::new();

        debug!("Extracting raw emails from database...");
        let raw_emails = database.extract_raw_emails(&export_config).await?;
        debug!("Found {} raw email records", raw_emails.len());

        let mut recipients = Vec::new();

        for raw_email in raw_emails {
            debug!("Processing email: {}", raw_email.email);

            let processed = processor
                .process_email_data(raw_email.clone(), &export_config)
                .await?;

            let recipient_name = processed.name.or(processed.first_name).unwrap_or_else(|| {
                // Extract name from email if no name available
                raw_email
                    .email
                    .split('@')
                    .next()
                    .unwrap_or("Developer")
                    .to_string()
            });

            let repo_name = extract_repo_name_from_url(&raw_email.url);
            let specific_aspect =
                generate_specific_aspect(raw_email.total_commits, &raw_email.description);

            recipients.push(EmailRecipient {
                email: processed.email,
                recipient_name,
                repo_name,
                specific_aspect,
                contact_email: std::env::var("CONTACT_EMAIL")
                    .unwrap_or_else(|_| "info@fabinvest.com".to_string()),
                contact_phone: std::env::var("CONTACT_PHONE")
                    .unwrap_or_else(|_| "+44 20 4572 2916".to_string()),
                engagement_score: processed.engagement_score,
                domain_category: processed.domain_category,
                company_size: processed.company_size,
            });
        }

        debug!("Processed {} recipients", recipients.len());
        Ok(recipients)
    }

    // FROM YOUR ORIGINAL CODE - Show campaign preview
    fn show_campaign_preview(&self, recipients: &[EmailRecipient]) {
        println!("\n📋 Campaign Preview:");
        println!("━━━━━━━━━━━━━━━━━━━━━");

        for (i, recipient) in recipients.iter().take(5).enumerate() {
            println!(
                "{}. {} ({})",
                i + 1,
                recipient.recipient_name,
                recipient.email
            );
            println!("   📦 Project: {}", recipient.repo_name);
            println!(
                "   🎯 Aspect: {}",
                if recipient.specific_aspect.len() > 60 {
                    format!("{}...", &recipient.specific_aspect[..60])
                } else {
                    recipient.specific_aspect.clone()
                }
            );
            println!(
                "   📊 Score: {} pts | 🏷️ {}",
                recipient.engagement_score, recipient.domain_category
            );
            if i < 4 {
                println!();
            }
        }

        if recipients.len() > 5 {
            println!("   ... and {} more recipients", recipients.len() - 5);
        }
    }

    // Helper method to find recipient by email
    async fn find_recipient_by_email(&self, email: &str) -> Result<Option<EmailRecipient>> {
        // Load all recipients and find the matching one
        let recipients = self.load_email_recipients_for_campaign(0).await?;
        Ok(recipients.into_iter().find(|r| r.email == email))
    }
}
