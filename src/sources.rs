use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceConfig {
    pub name: String,
    pub owner: String,
    pub repo: String,
    pub output_filename: String,
    pub filters: FilterConfig,
    pub rules: RuleConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FilterConfig {
    pub exclude_patterns: Vec<String>,
    pub allow_patterns: Vec<String>,
    pub skip_line_patterns: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RuleConfig {
    pub require_https_links: bool,
    pub skip_headers: bool,
    pub skip_empty_lines: bool,
    #[serde(default)]
    pub be_inclusive: bool,
    #[serde(default)]
    pub is_meta_source: bool, // NEW: Flag for meta-sources
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourcesConfig {
    pub sources: Vec<SourceConfig>,
}

pub trait AwesomeSource {
    fn name(&self) -> &str;
    fn owner(&self) -> &str;
    fn repo(&self) -> &str;
    fn output_filename(&self) -> &str;
    fn should_process_line(&self, line: &str) -> bool;
    fn is_valid_project_url(&self, url: &str) -> bool;
    fn is_meta_source(&self) -> bool;
}

pub struct YamlSource {
    config: SourceConfig,
}

impl YamlSource {
    pub fn new(config: SourceConfig) -> Self {
        Self { config }
    }
}

impl AwesomeSource for YamlSource {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn owner(&self) -> &str {
        &self.config.owner
    }

    fn repo(&self) -> &str {
        &self.config.repo
    }

    fn output_filename(&self) -> &str {
        &self.config.output_filename
    }

    fn is_meta_source(&self) -> bool {
        self.config.rules.is_meta_source
    }

    fn should_process_line(&self, line: &str) -> bool {
        // Apply basic rules
        if self.config.rules.skip_empty_lines && line.trim().is_empty() {
            return false;
        }

        if self.config.rules.skip_headers && line.starts_with('#') {
            return false;
        }

        if self.config.rules.require_https_links && !line.contains("](https://") {
            return false;
        }

        // Check skip patterns
        let line_lower = line.to_lowercase();
        for pattern in &self.config.filters.skip_line_patterns {
            if line_lower.contains(&pattern.to_lowercase()) {
                return false;
            }
        }

        // If be_inclusive is true, be more permissive
        if self.config.rules.be_inclusive {
            return true;
        }

        true
    }

    fn is_valid_project_url(&self, url: &str) -> bool {
        // Check allow patterns first (these override excludes)
        for pattern in &self.config.filters.allow_patterns {
            if url.contains(pattern) {
                return true;
            }
        }

        // Check exclude patterns
        for pattern in &self.config.filters.exclude_patterns {
            if url.contains(pattern) {
                return false;
            }
        }

        true
    }
}

pub async fn load_sources_from_yaml(
    path: &str,
) -> std::result::Result<Vec<YamlSource>, Box<dyn std::error::Error + Send + Sync>> {
    let content = tokio::fs::read_to_string(path).await?;
    let config: SourcesConfig = serde_yaml::from_str(&content)?;

    Ok(config.sources.into_iter().map(YamlSource::new).collect())
}
