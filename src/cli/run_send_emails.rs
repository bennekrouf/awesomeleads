// src/cli/run_send_emails.rs
use crate::email_export::{EmailDatabase, EmailExportConfigBuilder, EmailProcessor};
use crate::email_sender::{
    extract_repo_name_from_url, generate_specific_aspect, EmailRecipient, MailgunConfig,
    MailgunSender,
};
use crate::models::CliApp;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use tracing::{debug, error, info};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl CliApp {
    pub async fn send_emails_via_mailgun(&self) -> Result<()> {
        println!("\n📧 Mailgun Email Campaign System");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Test Mailgun connection first
        info!("Testing Mailgun connection...");
        let mailgun_config = match MailgunConfig::from_env() {
            Ok(config) => {
                debug!(
                    "Mailgun config loaded: domain={}, template={}",
                    config.domain, config.template_name
                );
                config
            }
            Err(e) => {
                error!("Failed to load Mailgun configuration: {}", e);
                println!("❌ Mailgun configuration error: {}", e);
                println!("💡 Make sure your .env file contains:");
                println!("   MAILGUN_API_KEY=your_api_key");
                println!("   MAILGUN_DOMAIN=t.fabinvest.com");
                println!("   MAILGUN_TEMPLATE=first message");
                return Err(e);
            }
        };

        let sender = MailgunSender::new(mailgun_config);

        // Test connection
        match sender.test_connection().await {
            Ok(()) => {
                println!("✅ Mailgun connection successful");
            }
            Err(e) => {
                error!("Mailgun connection test failed: {}", e);
                println!("❌ Failed to connect to Mailgun: {}", e);
                println!("💡 Check your API key and domain configuration");
                return Err(e);
            }
        }

        // Select campaign type
        let campaign_options = vec![
            "🎯 High-Value Projects (Recent + Active) - Recommended",
            "🚀 Startup Founders (Early commits + ownership)",
            "🏢 Enterprise Contacts (Large repos + teams)",
            "🔥 Web3/AI/Fintech Focus",
            "📊 All Valid Emails",
            "📈 Custom Filtered Campaign",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select campaign type")
            .default(0)
            .items(&campaign_options)
            .interact()?;

        println!(
            "\n🔍 Loading recipients for campaign type: {}",
            campaign_options[selection]
        );

        // Load email recipients based on selection
        let recipients = self.load_email_recipients_for_campaign(selection).await?;

        if recipients.is_empty() {
            println!("❌ No recipients found for this campaign type");
            println!("💡 Try running Phase 2 to collect more email data first");
            return Ok(());
        }

        println!("📋 Found {} potential recipients", recipients.len());

        // Show preview
        self.show_campaign_preview(&recipients);

        // Get batch size
        let suggested_batch = if recipients.len() > 100 {
            50
        } else {
            recipients.len()
        };
        let batch_size: usize = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("How many emails to send in this batch?")
            .default(suggested_batch)
            .interact_text()?;
        let recipient_len = recipients.len();

        let batch_recipients = recipients.into_iter().take(batch_size).collect::<Vec<_>>();

        println!("\n📤 Campaign Summary:");
        println!("  📧 Emails to send: {}", batch_recipients.len());
        println!(
            "  ⏱️  Estimated time: {} minutes",
            (batch_recipients.len() * 3) / 60 + 1
        );
        println!("  🎯 Template: {}", sender.config.template_name);
        println!(
            "  📨 From: {} <{}>",
            sender.config.from_name, sender.config.from_email
        );

        // Final confirmation
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!(
                "Send email campaign to {} recipients?",
                batch_recipients.len()
            ))
            .default(false)
            .interact()?;

        if !proceed {
            println!("❌ Campaign cancelled");
            return Ok(());
        }

        println!("\n🚀 Starting email campaign...");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("⏱️  Using 3-second delays between emails for optimal deliverability");
        println!("📊 Progress will be shown in real-time");

        let start_time = std::time::Instant::now();
        let results = sender.send_batch(&batch_recipients, 3000).await?; // 3 second delay
        let duration = start_time.elapsed();

        // Detailed results analysis
        let successful = results.iter().filter(|r| r.is_ok()).count();
        let failed = results.iter().filter(|r| r.is_err()).count();

        println!("\n🎉 Campaign Complete!");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("✅ Successful sends: {}", successful);
        println!("❌ Failed sends: {}", failed);
        println!(
            "📊 Success rate: {:.1}%",
            (successful as f64 / batch_recipients.len() as f64) * 100.0
        );
        println!(
            "⏱️  Total time: {:.1} minutes",
            duration.as_secs() as f64 / 60.0
        );
        println!(
            "📈 Average time per email: {:.1} seconds",
            duration.as_secs() as f64 / batch_recipients.len() as f64
        );

        if failed > 0 {
            println!("\n⚠️  Failed sends breakdown:");
            for (i, result) in results.iter().enumerate() {
                if let Err(error) = result {
                    println!("   • {}: {}", batch_recipients[i].email, error);
                }
            }
            println!("💡 Check Mailgun dashboard for detailed bounce/failure analysis");
        }

        // Analytics summary
        let category_stats = self.calculate_category_stats(&batch_recipients);
        println!("\n📊 Campaign Analytics:");
        for (category, count) in category_stats {
            println!(
                "  {} {}: {} emails",
                self.get_category_emoji(&category),
                category,
                count
            );
        }

        println!("\n🎯 Next Steps:");
        println!("  📈 Monitor open rates in Mailgun dashboard");
        println!("  📧 Watch for email replies in your inbox");
        println!("  📊 Track click-through rates on links");
        if batch_recipients.len() < recipient_len {
            println!(
                "  🔄 Consider sending another batch to remaining {} recipients",
                recipient_len - batch_recipients.len()
            );
        }

        Ok(())
    }

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

    fn calculate_category_stats(&self, recipients: &[EmailRecipient]) -> Vec<(String, usize)> {
        let mut stats = std::collections::HashMap::new();

        for recipient in recipients {
            *stats.entry(recipient.domain_category.clone()).or_insert(0) += 1;
        }

        let mut result: Vec<_> = stats.into_iter().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1)); // Sort by count descending
        result
    }

    fn get_category_emoji(&self, category: &str) -> &'static str {
        match category {
            "ai" => "🤖",
            "web3" => "🪙",
            "fintech" => "💳",
            "enterprise" => "🏢",
            "saas" => "☁️",
            _ => "📦",
        }
    }
}
