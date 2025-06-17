// src/email_export/database.rs
use crate::database::DbPool;
use super::types::{ExportConfig, RawEmailData};
use std::collections::HashSet;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub struct EmailDatabase {
    db_pool: DbPool,
}

impl EmailDatabase {
    pub fn new(db_pool: DbPool) -> Self {
        Self { db_pool }
    }

    pub async fn has_contributors_data(&self) -> Result<bool> {
        let conn = self.db_pool.get().await?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM contributors", 
            [], 
            |row| row.get(0)
        )?;
        Ok(count > 0)
    }

    pub async fn extract_raw_emails(&self, config: &ExportConfig) -> Result<Vec<RawEmailData>> {
        let conn = self.db_pool.get().await?;
        
        let sql = if self.has_contributors_data().await? {
            self.build_sql_with_contributors(config)
        } else {
            self.build_sql_simple(config)
        };

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(RawEmailData {
                email: row.get(0)?,
                name: row.get::<_, Option<String>>(1)?,
                url: row.get(2)?,
                description: row.get::<_, Option<String>>(3)?,
                repository_created: row.get::<_, Option<String>>(4)?,
                total_commits: row.get::<_, Option<i32>>(5)?,
                owner: row.get::<_, Option<String>>(6)?,
                source_repository: row.get(7)?,
            })
        })?;

        let mut emails = Vec::new();
        let mut seen_emails = HashSet::new();

        for row in rows {
            let raw = row?;

            // Deduplicate by email
            if !seen_emails.insert(raw.email.clone()) {
                continue;
            }

            // Validate email format
            if !self.is_valid_email(&raw.email) {
                continue;
            }

            emails.push(raw);
        }

        Ok(emails)
    }

    fn build_sql_with_contributors(&self, config: &ExportConfig) -> String {
        format!(
            r#"
            SELECT DISTINCT
                p.email,
                COALESCE(c.name, p.owner) as name,
                p.url,
                p.description,
                p.repository_created,
                p.total_commits,
                p.owner,
                p.source_repository
            FROM projects p
            LEFT JOIN contributors c ON p.url = c.project_url 
                AND p.email = c.email
            {}
            ORDER BY p.total_commits DESC NULLS LAST, p.repository_created DESC NULLS LAST
            "#,
            config.sql_filter
        )
    }

    fn build_sql_simple(&self, config: &ExportConfig) -> String {
        format!(
            r#"
            SELECT DISTINCT
                p.email,
                p.owner as name,
                p.url,
                p.description,
                p.repository_created,
                p.total_commits,
                p.owner,
                p.source_repository
            FROM projects p
            {}
            ORDER BY p.total_commits DESC NULLS LAST, p.repository_created DESC NULLS LAST
            "#,
            config.sql_filter
        )
    }

    fn is_valid_email(&self, email: &str) -> bool {
        email.contains('@')
            && !email.contains("noreply")
            && !email.contains("[bot]")
            && email.len() > 5
            && email.len() < 255
    }
}
