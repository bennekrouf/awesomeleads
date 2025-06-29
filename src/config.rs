use crate::email_rate_limiting::EmailLimitsConfig;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub scraping: ScrapingConfig,
    pub logging: LoggingConfig,
    pub output: OutputConfig,
    pub email_limits: EmailLimitsConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScrapingConfig {
    #[serde(deserialize_with = "deserialize_date")]
    pub min_first_commit_date: DateTime<Utc>,

    #[serde(deserialize_with = "deserialize_date")]
    pub min_repository_created_date: DateTime<Utc>,

    pub rate_limit_delay_ms: u64,
    pub api_timeout_seconds: u64,
    pub max_projects_per_source: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    pub level: String,
    pub progress_interval: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    pub directory: String,
    pub pretty_json: bool,
}

// Custom deserializer for flexible date formats
fn deserialize_date<'de, D>(deserializer: D) -> std::result::Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    // Try full date format first (YYYY-MM-DD)
    if let Ok(naive_date) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        if let Some(datetime) = naive_date.and_hms_opt(0, 0, 0) {
            return Ok(datetime.and_utc());
        }
    }

    // Try year-only format (YYYY) - defaults to January 1st
    if let Ok(year) = s.parse::<i32>() {
        if let Some(naive_date) = NaiveDate::from_ymd_opt(year, 1, 1) {
            if let Some(datetime) = naive_date.and_hms_opt(0, 0, 0) {
                return Ok(datetime.and_utc());
            }
        }
    }

    Err(serde::de::Error::custom(format!(
        "Invalid date format: {}",
        s
    )))
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scraping: ScrapingConfig {
                min_first_commit_date: NaiveDate::from_ymd_opt(2020, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc(),
                min_repository_created_date: NaiveDate::from_ymd_opt(2019, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_utc(),
                rate_limit_delay_ms: 100,
                api_timeout_seconds: 10,
                max_projects_per_source: 0,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                progress_interval: 10,
            },
            output: OutputConfig {
                directory: "out".to_string(),
                pretty_json: true,
            },
            email_limits: EmailLimitsConfig::default(),
        }
    }
}

pub async fn load_config(
    path: &str,
) -> std::result::Result<Config, Box<dyn std::error::Error + Send + Sync>> {
    let content = tokio::fs::read_to_string(path).await?;
    let config: Config = serde_yaml::from_str(&content)?;
    Ok(config)
}
