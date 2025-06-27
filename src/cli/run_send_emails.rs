// src/cli/run_send_emails.rs - COMPLETE REPLACEMENT
use crate::email_export::{EmailDatabase, EmailExportConfigBuilder, EmailProcessor};
use crate::email_sender::{
    extract_repo_name_from_url, generate_specific_aspect, EmailRecipient, EmailTemplate,
    MailgunConfig, MailgunSender,
};
use crate::models::CliApp;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use tracing::debug;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
// use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

impl CliApp {
    pub async fn send_emails_via_mailgun(&self) -> Result<()> {
        println!("\n📧 Smart Email Campaign System");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let mailgun_config = MailgunConfig::from_env().map_err(|e| {
            println!("❌ Mailgun configuration error: {}", e);
            e
        })?;

        let sender = MailgunSender::new(mailgun_config);

        // Campaign type selection
        let campaign_options = vec![
            "🎯 First Contact Campaign (Investment Proposals)",
            "📬 Follow-up Campaign (Second Touch)",
            "📊 Show Email Statistics",
            "🔍 Check Specific Email Status",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select campaign type")
            .items(&campaign_options)
            .interact()?;

        match selection {
            0 => self.run_first_contact_campaign(&sender).await?,
            1 => self.run_followup_campaign(&sender).await?,
            2 => self.show_email_statistics(&sender).await?,
            3 => self.check_email_status(&sender).await?,
            _ => return Ok(()),
        }

        Ok(())
    }

    async fn run_first_contact_campaign(&self, sender: &MailgunSender) -> Result<()> {
        println!("\n🎯 First Contact Campaign");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Load recipients who haven't received the first email
        let all_recipients = self.load_email_recipients_for_campaign(0).await?;
        let mut first_contact_candidates = Vec::new();

        println!("🔍 Filtering candidates who haven't received first contact...");
        for recipient in all_recipients {
            let status = sender
                .check_email_status(&self.db_pool, &recipient.email)
                .await?;
            if status.can_send_first {
                first_contact_candidates.push(recipient);
            }
        }

        if first_contact_candidates.is_empty() {
            println!("✅ All eligible recipients have already received first contact emails!");
            return Ok(());
        }

        println!(
            "📋 Found {} candidates for first contact",
            first_contact_candidates.len()
        );
        self.show_campaign_preview(&first_contact_candidates);

        let batch_size: usize = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("How many first contact emails to send?")
            .default(first_contact_candidates.len().min(50))
            .interact_text()?;

        let batch = first_contact_candidates
            .into_iter()
            .take(batch_size)
            .collect::<Vec<_>>();

        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!("Send {} investment proposal emails?", batch.len()))
            .interact()?
        {
            return Ok(());
        }

        self.send_campaign_batch(
            &sender,
            &batch,
            EmailTemplate::InvestmentProposal,
            "first_contact",
        )
        .await
    }

    async fn run_followup_campaign(&self, sender: &MailgunSender) -> Result<()> {
        println!("\n📬 Follow-up Campaign");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let days_since_first: i64 = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Minimum days since first email")
            .default(7)
            .interact_text()?;

        let followup_emails = sender
            .get_followup_candidates(&self.db_pool, days_since_first)
            .await?;

        if followup_emails.is_empty() {
            println!(
                "📭 No follow-up candidates found (need {} days since first email)",
                days_since_first
            );
            return Ok(());
        }

        println!(
            "📋 Found {} candidates for follow-up",
            followup_emails.len()
        );

        // Convert emails back to recipients (you might want to store more data in tracking)
        let mut followup_recipients = Vec::new();
        for email in &followup_emails {
            // Simplified - you might want to fetch full recipient data from your database
            if let Some(recipient) = self.find_recipient_by_email(email).await? {
                followup_recipients.push(recipient);
            }
        }

        if followup_recipients.is_empty() {
            println!("❌ Could not find recipient data for follow-up emails");
            return Ok(());
        }

        let batch_size: usize = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("How many follow-up emails to send?")
            .default(followup_recipients.len().min(25))
            .interact_text()?;

        let batch = followup_recipients
            .into_iter()
            .take(batch_size)
            .collect::<Vec<_>>();

        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!("Send {} follow-up emails?", batch.len()))
            .interact()?
        {
            return Ok(());
        }

        self.send_campaign_batch(&sender, &batch, EmailTemplate::FollowUp, "follow_up")
            .await
    }

    async fn send_campaign_batch(
        &self,
        sender: &MailgunSender,
        recipients: &[crate::email_sender::EmailRecipient],
        template: EmailTemplate,
        campaign_type: &str,
    ) -> Result<()> {
        println!("\n🚀 Sending {} emails...", recipients.len());

        let mut successful = 0;
        let mut failed = 0;

        for (i, recipient) in recipients.iter().enumerate() {
            println!(
                "[{}/{}] Sending to {} ({})",
                i + 1,
                recipients.len(),
                recipient.recipient_name,
                recipient.email
            );

            match sender
                .send_email_with_tracking(&self.db_pool, recipient, template.clone(), campaign_type)
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

            // Rate limiting
            if i < recipients.len() - 1 {
                tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;
            }
        }

        println!("\n🎉 Campaign Complete!");
        println!("✅ Successful: {}", successful);
        println!("❌ Failed: {}", failed);

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

        println!("📧 Investment Proposals Sent: {}", total_first);
        println!("📬 Follow-ups Sent: {}", total_followup);
        println!("👥 Unique Contacts: {}", unique_contacts);

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

