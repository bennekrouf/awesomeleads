// src/email_export/config.rs
use dialoguer::{theme::ColorfulTheme, Input, Select};
use super::types::ExportConfig;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct EmailExportConfigBuilder;

impl EmailExportConfigBuilder {
    pub fn new() -> Self {
        Self
    }

    pub async fn build_config(&self, selection: usize) -> Result<ExportConfig> {
        let config = match selection {
            0 => ExportConfig {
                title: "All Valid Emails".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%')".to_string(),
                min_engagement_score: 10,
            },
            1 => ExportConfig {
                title: "High-Value Projects".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%') AND (repository_created > '2022-01-01' OR first_commit_date > '2022-01-01') AND total_commits > 5".to_string(),
                min_engagement_score: 50,
            },
            2 => ExportConfig {
                title: "Startup Founders".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%') AND total_commits > 20 AND repository_created > '2020-01-01'".to_string(),
                min_engagement_score: 70,
            },
            3 => ExportConfig {
                title: "Enterprise Contacts".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%') AND total_commits > 100".to_string(),
                min_engagement_score: 60,
            },
            4 => ExportConfig {
                title: "Web3/AI/Fintech Focus".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%') AND (LOWER(description) LIKE '%blockchain%' OR LOWER(description) LIKE '%ai%' OR LOWER(description) LIKE '%ml%' OR LOWER(description) LIKE '%fintech%' OR LOWER(description) LIKE '%defi%' OR LOWER(url) LIKE '%web3%')".to_string(),
                min_engagement_score: 40,
            },
            5 => {
                let custom: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Enter custom SQL WHERE clause")
                    .with_initial_text("WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%')")
                    .interact_text()?;

                ExportConfig {
                    title: "Custom Export".to_string(),
                    sql_filter: custom,
                    min_engagement_score: 30,
                }
            },
            _ => ExportConfig {
                title: "Default Export".to_string(),
                sql_filter: "WHERE (email IS NOT NULL AND email != '' AND email NOT LIKE '%noreply%')".to_string(),
                min_engagement_score: 30,
            },
        };

        Ok(config)
    }

    pub fn get_export_type_options(&self) -> Vec<&'static str> {
        vec![
            "📊 All Valid Emails (Real emails only)",
            "🎯 High-Value Projects (Recent + Active)",
            "🚀 Startup Founders (Early commits + ownership)",
            "🏢 Enterprise Contacts (Large repos + teams)",
            "🔥 Web3/AI/Fintech Focus",
            "📈 Custom Filtered Export",
        ]
    }

    pub async fn select_export_type(&self) -> Result<usize> {
        let export_types = self.get_export_type_options();

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select export type")
            .items(&export_types)
            .interact()?;

        Ok(selection)
    }
}
