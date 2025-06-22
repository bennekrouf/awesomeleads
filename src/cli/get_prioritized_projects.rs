use crate::models::CliApp;
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
impl CliApp {
    pub async fn get_prioritized_projects(
        &self,
        additional_filter: &str,
        limit: usize,
    ) -> Result<Vec<crate::database::StoredProject>> {
        let conn = self.db_pool.get().await?;

        let sql = format!(
            r#"
            SELECT url, description, owner, repo_name, repository_created, 
                   first_commit_date, last_commit_date, email, email_source,
                   top_contributor_email, top_contributor_commits, total_commits,
                   source_repository, scraped_at, last_updated 
            FROM projects 
            WHERE (email IS NULL OR email = '' OR first_commit_date IS NULL OR repository_created IS NULL)
            AND owner IS NOT NULL 
            AND repo_name IS NOT NULL
            {}
            ORDER BY 
                CASE WHEN repository_created IS NOT NULL AND repository_created > '2022-01-01' THEN 1 ELSE 2 END,
                CASE WHEN LOWER(url) LIKE '%rust%' OR LOWER(url) LIKE '%javascript%' OR LOWER(url) LIKE '%python%' THEN 1 ELSE 2 END,
                repository_created DESC NULLS LAST
            LIMIT {}
            "#,
            additional_filter, limit
        );

        let mut stmt = conn.prepare(&sql)?;

        let rows = stmt.query_map([], |row| {
            let get_optional_string = |idx: usize| -> Option<String> {
                match row.get::<_, Option<String>>(idx) {
                    Ok(Some(s)) if !s.is_empty() => Some(s),
                    _ => None,
                }
            };

            let get_optional_i32 = |idx: usize| -> Option<i32> {
                match row.get::<_, Option<i32>>(idx) {
                    Ok(Some(val)) if val != -1 => Some(val),
                    _ => None,
                }
            };

            let scraped_at_str: String = row.get(13)?;
            let last_updated_str: String = row.get(14)?;

            Ok(crate::database::StoredProject {
                id: None,
                url: row.get(0)?,
                description: get_optional_string(1),
                owner: get_optional_string(2),
                repo_name: get_optional_string(3),
                repository_created: get_optional_string(4),
                first_commit_date: get_optional_string(5),
                last_commit_date: get_optional_string(6),
                email: get_optional_string(7),
                email_source: get_optional_string(8),
                top_contributor_email: get_optional_string(9),
                top_contributor_commits: get_optional_i32(10),
                total_commits: get_optional_i32(11),
                source_repository: row.get(12)?,
                scraped_at: chrono::DateTime::parse_from_rfc3339(&scraped_at_str)
                    .unwrap()
                    .with_timezone(&chrono::Utc),
                last_updated: chrono::DateTime::parse_from_rfc3339(&last_updated_str)
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            })
        })?;

        let mut projects = Vec::new();
        for row in rows {
            projects.push(row?);
        }

        Ok(projects)
    }
}
