// src/email_export/exporter.rs
use super::types::{EmailExport, ExportStats};
use chrono::Utc;
use std::collections::HashMap;
use std::io::Write;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct EmailExporter;

impl EmailExporter {
    pub fn new() -> Self {
        Self
    }

    pub async fn export_to_csv(&self, emails: &[EmailExport], filename: &str) -> Result<()> {
        // Ensure directory exists
        if let Some(parent) = std::path::Path::new(filename).parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut file = std::fs::File::create(filename)?;

        // Write CSV header
        writeln!(
            file,
            "email,name,first_name,status,consent_timestamp,source,domain_category,tags,company_size,industry,engagement_score,project_url,repository_created,commit_count"
        )?;

        // Write data rows
        for email in emails {
            writeln!(
                file,
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                email.email,
                email.name.as_deref().unwrap_or(""),
                email.first_name.as_deref().unwrap_or(""),
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

    pub fn generate_stats(&self, emails: &[EmailExport]) -> ExportStats {
        let mut category_counts: HashMap<String, usize> = HashMap::new();
        let mut size_counts: HashMap<String, usize> = HashMap::new();

        for email in emails {
            *category_counts
                .entry(email.domain_category.clone())
                .or_insert(0) += 1;
            *size_counts.entry(email.company_size.clone()).or_insert(0) += 1;
        }

        let avg_engagement: f64 = if emails.is_empty() {
            0.0
        } else {
            emails
                .iter()
                .map(|e| e.engagement_score as f64)
                .sum::<f64>()
                / emails.len() as f64
        };

        ExportStats {
            total_emails: emails.len(),
            by_category: category_counts,
            by_company_size: size_counts,
            average_engagement: avg_engagement,
        }
    }

    pub fn print_stats(&self, stats: &ExportStats) {
        println!("\n📊 Export Statistics:");
        println!("━━━━━━━━━━━━━━━━━━━━━");

        println!("🏷️  By Category:");
        for (category, count) in &stats.by_category {
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
        for (size, count) in &stats.by_company_size {
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

        println!(
            "\n⭐ Average Engagement Score: {:.1}",
            stats.average_engagement
        );
    }

    pub fn generate_filename(&self) -> String {
        format!(
            "out/emails_export_{}.csv",
            Utc::now().format("%Y%m%d_%H%M%S")
        )
    }
}
