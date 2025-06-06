use crate::models::CliApp;

impl CliApp {
    pub async fn is_low_value_project(
        &self,
        _owner: &str,
        repo: &str,
        description: &Option<String>,
    ) -> bool {
        // let owner_lower = owner.to_lowercase();
        let repo_lower = repo.to_lowercase();
        let desc_lower = description.as_deref().unwrap_or("").to_lowercase();

        // Skip documentation repositories
        if repo_lower.contains("docs")
            || repo_lower.contains("documentation")
            || repo_lower.contains("wiki")
            || repo_lower.contains("guide")
            || desc_lower.contains("documentation")
            || desc_lower.contains("tutorial")
        {
            return true;
        }

        // Skip badge/shield repositories
        if repo_lower.contains("badge")
            || repo_lower.contains("shield")
            || repo_lower.contains("icon")
            || desc_lower.contains("badge")
        {
            return true;
        }

        // Skip archived/deprecated projects
        if repo_lower.contains("archive")
            || repo_lower.contains("deprecated")
            || repo_lower.contains("legacy")
            || desc_lower.contains("archived")
            || desc_lower.contains("deprecated")
            || desc_lower.contains("no longer maintained")
        {
            return true;
        }

        // Skip example/demo repositories
        if repo_lower.contains("example")
            || repo_lower.contains("demo")
            || repo_lower.contains("sample")
            || repo_lower.contains("template")
            || desc_lower.contains("example")
            || desc_lower.contains("demo")
        {
            return true;
        }

        // Skip personal dotfiles
        if repo_lower.contains("dotfiles") || repo_lower.contains("config") {
            return true;
        }

        false
    }
}
