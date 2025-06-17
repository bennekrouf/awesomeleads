// src/cli/run_export_emails.rs (REFACTORED VERSION)
use crate::{models::CliApp, Result};
use crate::email_export::{
    EmailDatabase, EmailProcessor, EmailExportConfigBuilder, EmailExporter
};
use dialoguer::{theme::ColorfulTheme, Confirm};

impl CliApp {
    pub async fn run_export_emails(&self) -> Result<()> {
        println!("\n📧 Email Export System");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Initialize components
        let config_builder = EmailExportConfigBuilder::new();
        let database = EmailDatabase::new(self.db_pool.clone());
        let processor = EmailProcessor::new();
        let exporter = EmailExporter::new();

        // Select export type
        let selection = config_builder.select_export_type().await?;
        let export_config = config_builder.build_config(selection).await?;

        println!("\n🔍 Export Type: {}", export_config.title);

        // Extract raw email data
        println!("📊 Extracting email data from database...");
        let raw_emails = database.extract_raw_emails(&export_config).await?;

        if raw_emails.is_empty() {
            println!("❌ No emails found matching criteria");
            return Ok(());
        }

        // Process emails
        println!("⚙️  Processing {} raw email records...", raw_emails.len());
        let mut processed_emails = Vec::new();

        for raw_email in raw_emails {
            match processor.process_email_data(raw_email, &export_config).await {
                Ok(processed) => processed_emails.push(processed),
                Err(e) => {
                    eprintln!("⚠️  Failed to process email: {}", e);
                }
            }
        }

        if processed_emails.is_empty() {
            println!("❌ No valid emails after processing");
            return Ok(());
        }

        // Show preview
        self.show_export_preview(&processed_emails);

        // Confirm export
        let proceed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(&format!("Export {} emails to CSV?", processed_emails.len()))
            .interact()?;

        if !proceed {
            println!("❌ Export cancelled");
            return Ok(());
        }

        // Export to CSV
        let filename = exporter.generate_filename();
        exporter.export_to_csv(&processed_emails, &filename).await?;

        // Show results
        let stats = exporter.generate_stats(&processed_emails);
        
        println!("\n✅ Email export completed!");
        println!("📁 File: {}", filename);
        println!("📊 Total emails: {}", stats.total_emails);
        
        exporter.print_stats(&stats);

        Ok(())
    }

    fn show_export_preview(&self, emails: &[crate::email_export::EmailExport]) {
        println!("\n📋 Export Preview:");
        println!("━━━━━━━━━━━━━━━━━━━━━");
        
        for (i, email) in emails.iter().take(5).enumerate() {
            let name_display = email.name.as_deref()
                .or(email.first_name.as_deref())
                .unwrap_or("Unknown");
            
            println!(
                "{}. {} ({}) - {} - {}",
                i + 1,
                email.email,
                name_display,
                email.domain_category,
                email.company_size
            );
        }
        
        if emails.len() > 5 {
            println!("   ... and {} more", emails.len() - 5);
        }
    }
}
