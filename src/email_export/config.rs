// src/email_export/config.rs - Fixed SQL generation
use super::types::ExportConfig;
use dialoguer::{theme::ColorfulTheme, Input, Select};

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
                sql_filter: "WHERE (p.email IS NOT NULL AND p.email != '' AND p.email NOT LIKE '%noreply%')".to_string(),
            },
            1 => ExportConfig {
                title: "High-Value Projects".to_string(),
                sql_filter: "WHERE (p.email IS NOT NULL AND p.email != '' AND p.email NOT LIKE '%noreply%') AND (p.repository_created > '2022-01-01' OR p.first_commit_date > '2022-01-01') AND p.total_commits > 5".to_string(),
            },
            2 => ExportConfig {
                title: "Startup Founders".to_string(),
                sql_filter: "WHERE (p.email IS NOT NULL AND p.email != '' AND p.email NOT LIKE '%noreply%') AND p.total_commits > 20 AND p.repository_created > '2020-01-01'".to_string(),
            },
            3 => ExportConfig {
                title: "Enterprise Contacts".to_string(),
                sql_filter: "WHERE (p.email IS NOT NULL AND p.email != '' AND p.email NOT LIKE '%noreply%') AND p.total_commits > 100".to_string(),
            },
            4 => ExportConfig {
                title: "Web3/AI/Fintech Focus".to_string(),
                sql_filter: "WHERE (p.email IS NOT NULL AND p.email != '' AND p.email NOT LIKE '%noreply%') AND (LOWER(p.description) LIKE '%blockchain%' OR LOWER(p.description) LIKE '%ai%' OR LOWER(p.description) LIKE '%ml%' OR LOWER(p.description) LIKE '%fintech%' OR LOWER(p.description) LIKE '%defi%' OR LOWER(p.url) LIKE '%web3%')".to_string(),
            },
            5 => {
                let custom: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Enter custom SQL WHERE clause (use 'p.' prefix for project columns)")
                    .with_initial_text("WHERE (p.email IS NOT NULL AND p.email != '' AND p.email NOT LIKE '%noreply%')")
                    .interact_text()?;

                ExportConfig {
                    title: "Custom Export".to_string(),
                    sql_filter: custom,
                }
            },
            _ => ExportConfig {
                title: "Default Export".to_string(),
                sql_filter: "WHERE (p.email IS NOT NULL AND p.email != '' AND p.email NOT LIKE '%noreply%')".to_string(),
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
