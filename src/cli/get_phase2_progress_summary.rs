use crate::models::{CliApp, Phase2Progress};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl CliApp {
    pub async fn get_phase2_progress_summary(&self) -> Result<Phase2Progress> {
        let conn = self.db_pool.get().await?;

        let total: i64 = conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))?;
        let complete: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE email IS NOT NULL AND email != '' AND first_commit_date IS NOT NULL AND repository_created IS NOT NULL", 
            [],
            |row| row.get(0)
        )?;
        let partial: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE ((email IS NOT NULL AND email != '') OR (first_commit_date IS NOT NULL) OR (repository_created IS NOT NULL)) AND NOT (email IS NOT NULL AND email != '' AND first_commit_date IS NOT NULL AND repository_created IS NOT NULL)", 
            [],
            |row| row.get(0)
        )?;

        let untouched = total - complete - partial;
        let completion_rate = if total > 0 {
            (complete as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Ok(Phase2Progress {
            complete,
            partial,
            untouched,
            total,
            completion_rate,
        })
    }
}
