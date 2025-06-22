use dialoguer::{theme::ColorfulTheme, Input};

use crate::models::{CliApp, ProjectFilter};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
impl CliApp {
    pub async fn build_project_filter(&self, selection: usize) -> Result<ProjectFilter> {
        let filter = match selection {
            0 => ProjectFilter {
                description: "Rust projects (modern systems programming)".to_string(),
                sql_filter: r#"
                    AND (
                        LOWER(url) LIKE '%rust%' OR 
                        LOWER(description) LIKE '%rust%' OR
                        LOWER(owner) LIKE '%rust%' OR
                        LOWER(repo_name) LIKE '%rust%'
                    )
                    AND LOWER(url) NOT LIKE '%docs%'
                    AND LOWER(url) NOT LIKE '%badge%'
                "#.to_string(),
            },
            1 => ProjectFilter {
                description: "JavaScript/Node.js projects (web ecosystem)".to_string(),
                sql_filter: r#"
                    AND (
                        LOWER(url) LIKE '%javascript%' OR 
                        LOWER(url) LIKE '%node%' OR
                        LOWER(url) LIKE '%js%' OR
                        LOWER(url) LIKE '%react%' OR
                        LOWER(url) LIKE '%vue%' OR
                        LOWER(url) LIKE '%angular%' OR
                        LOWER(description) LIKE '%javascript%' OR
                        LOWER(description) LIKE '%node%'
                    )
                    AND LOWER(url) NOT LIKE '%docs%'
                    AND LOWER(url) NOT LIKE '%badge%'
                "#.to_string(),
            },
            2 => ProjectFilter {
                description: "Python projects (data science, AI/ML)".to_string(),
                sql_filter: r#"
                    AND (
                        LOWER(url) LIKE '%python%' OR 
                        LOWER(description) LIKE '%python%' OR
                        LOWER(description) LIKE '%django%' OR
                        LOWER(description) LIKE '%flask%' OR
                        LOWER(description) LIKE '%machine learning%' OR
                        LOWER(description) LIKE '%data science%'
                    )
                    AND LOWER(url) NOT LIKE '%docs%'
                    AND LOWER(url) NOT LIKE '%badge%'
                "#.to_string(),
            },
            3 => ProjectFilter {
                description: "Go projects (cloud, infrastructure)".to_string(),
                sql_filter: r#"
                    AND (
                        LOWER(url) LIKE '%golang%' OR 
                        LOWER(description) LIKE '%golang%' OR
                        LOWER(description) LIKE '% go %' OR
                        LOWER(url) LIKE '%go-%'
                    )
                    AND LOWER(url) NOT LIKE '%docs%'
                    AND LOWER(url) NOT LIKE '%badge%'
                "#.to_string(),
            },
            4 => ProjectFilter {
                description: "Ruby projects (web development)".to_string(),
                sql_filter: r#"
                    AND (
                        LOWER(url) LIKE '%ruby%' OR 
                        LOWER(description) LIKE '%ruby%' OR
                        LOWER(description) LIKE '%rails%'
                    )
                    AND LOWER(url) NOT LIKE '%docs%'
                    AND LOWER(url) NOT LIKE '%badge%'
                "#.to_string(),
            },
            5 => ProjectFilter {
                description: "Java projects (enterprise)".to_string(),
                sql_filter: r#"
                    AND (
                        LOWER(url) LIKE '%java%' OR 
                        LOWER(description) LIKE '%java%' OR
                        LOWER(description) LIKE '%spring%' OR
                        LOWER(description) LIKE '%maven%'
                    )
                    AND LOWER(url) NOT LIKE '%javascript%'
                    AND LOWER(url) NOT LIKE '%docs%'
                    AND LOWER(url) NOT LIKE '%badge%'
                "#.to_string(),
            },
            6 => ProjectFilter {
                description: "Recent projects (created after 2022)".to_string(),
                sql_filter: r#"
                    AND (repository_created > '2022-01-01' OR repository_created IS NULL)
                    AND LOWER(url) NOT LIKE '%docs%'
                    AND LOWER(url) NOT LIKE '%badge%'
                    AND LOWER(url) NOT LIKE '%archive%'
                "#.to_string(),
            },
            7 => ProjectFilter {
                description: "Popular projects (likely high-quality)".to_string(),
                sql_filter: r#"
                    AND (
                        LOWER(description) LIKE '%popular%' OR
                        LOWER(description) LIKE '%awesome%' OR
                        LOWER(description) LIKE '%starred%' OR
                        LOWER(url) LIKE '%microsoft%' OR
                        LOWER(url) LIKE '%google%' OR
                        LOWER(url) LIKE '%facebook%' OR
                        LOWER(url) LIKE '%apple%' OR
                        LOWER(url) LIKE '%netflix%' OR
                        LOWER(url) LIKE '%uber%'
                    )
                    AND LOWER(url) NOT LIKE '%docs%'
                    AND LOWER(url) NOT LIKE '%badge%'
                "#.to_string(),
            },
            8 => ProjectFilter {
                description: "Mixed high-value batch (Rust, JS, Python, Go, recent)".to_string(),
                sql_filter: r#"
                    AND (
                        LOWER(url) LIKE '%rust%' OR 
                        LOWER(url) LIKE '%javascript%' OR 
                        LOWER(url) LIKE '%python%' OR 
                        LOWER(url) LIKE '%golang%' OR
                        LOWER(url) LIKE '%node%' OR
                        LOWER(url) LIKE '%react%' OR
                        LOWER(description) LIKE '%rust%' OR
                        LOWER(description) LIKE '%javascript%' OR
                        LOWER(description) LIKE '%python%' OR
                        (repository_created > '2022-01-01' OR repository_created IS NULL)
                    )
                    AND LOWER(url) NOT LIKE '%docs%'
                    AND LOWER(url) NOT LIKE '%badge%'
                    AND LOWER(url) NOT LIKE '%archive%'
                "#.to_string(),
            },
            9 => ProjectFilter {
                description: "Cleanup partial projects (have some data, missing others)".to_string(),
                sql_filter: r#"
                    AND (
                        (email IS NOT NULL AND email != '' AND (first_commit_date IS NULL OR repository_created IS NULL)) OR
                        (first_commit_date IS NOT NULL AND first_commit_date != '' AND (email IS NULL OR email = '')) OR
                        (repository_created IS NOT NULL AND repository_created != '' AND (email IS NULL OR email = ''))
                    )
                "#.to_string(),
            },
            10 => {
                let custom: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Enter custom SQL WHERE clause (e.g., 'AND owner = \"microsoft\"')")
                    .interact_text()?;

                ProjectFilter {
                    description: format!("Custom filter: {}", custom),
                    sql_filter: custom,
                }
            },
            _ => ProjectFilter {
                description: "All projects".to_string(),
                sql_filter: "".to_string(),
            },
        };

        Ok(filter)
    }
}
