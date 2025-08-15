// src/api/stats.rs
use crate::database::get_database_stats;
use crate::models::Phase2Progress;
use crate::server::ServerState;
use rocket::{get, serde::json::Json, State};
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

#[derive(Serialize)]
pub struct StatsOverview {
    pub total_github_projects: i64,
    pub total_non_github_projects: i64,
    pub projects_with_email: i64,
    pub projects_with_github_data: i64,
    pub projects_with_contributor_data: i64,
    pub projects_with_commit_stats: i64,
    pub avg_commits_per_project: f64,
    pub crawled_emails_found: i64,
    pub completion_percentage: f64,
}

#[derive(Serialize)]
pub struct EmailStats {
    pub total_first_contact: i64,
    pub total_followup: i64,
    pub unique_contacts: i64,
    pub debug_emails: i64,
    pub recent_7_days: i64,
    pub followup_rate: f64,
}

#[get("/stats")]
pub async fn get_stats(state: &State<ServerState>) -> Json<ApiResponse<StatsOverview>> {
    match get_database_stats(&state.db_pool).await {
        Ok(stats) => {
            let total_projects = stats.total_github_projects + stats.total_non_github_projects;
            let completion_percentage = if total_projects > 0 {
                (stats.projects_with_github_data as f64 / stats.total_github_projects as f64)
                    * 100.0
            } else {
                0.0
            };

            let overview = StatsOverview {
                total_github_projects: stats.total_github_projects,
                total_non_github_projects: stats.total_non_github_projects,
                projects_with_email: stats.projects_with_email,
                projects_with_github_data: stats.projects_with_github_data,
                projects_with_contributor_data: stats.projects_with_contributor_data,
                projects_with_commit_stats: stats.projects_with_commit_stats,
                avg_commits_per_project: stats.avg_commits_per_project,
                crawled_emails_found: stats.crawled_emails_found,
                completion_percentage,
            };

            Json(ApiResponse::success(overview))
        }
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[get("/stats/phase2")]
pub async fn get_phase2_progress(state: &State<ServerState>) -> Json<ApiResponse<Phase2Progress>> {
    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let total: i64 = match conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0)) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let complete: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE email IS NOT NULL AND email != '' AND first_commit_date IS NOT NULL AND repository_created IS NOT NULL", 
        [],
        |row| row.get(0)
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let partial: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE ((email IS NOT NULL AND email != '') OR (first_commit_date IS NOT NULL) OR (repository_created IS NOT NULL)) AND NOT (email IS NOT NULL AND email != '' AND first_commit_date IS NOT NULL AND repository_created IS NOT NULL)", 
        [],
        |row| row.get(0)
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let untouched = total - complete - partial;
    let completion_rate = if total > 0 {
        (complete as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let progress = Phase2Progress {
        complete,
        partial,
        untouched,
        total,
        completion_rate,
    };

    Json(ApiResponse::success(progress))
}

#[get("/stats/email")]
pub async fn get_email_stats(state: &State<ServerState>) -> Json<ApiResponse<EmailStats>> {
    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let total_first: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE template_name = 'investment_proposal'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_followup: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE template_name = 'follow_up'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let unique_contacts: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT email) FROM email_tracking",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let debug_emails: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE campaign_type LIKE 'debug_%'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let recent_7_days: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE sent_at > ?",
            [&(chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339()],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let followup_rate = if total_first > 0 {
        (total_followup as f64 / total_first as f64) * 100.0
    } else {
        0.0
    };

    let stats = EmailStats {
        total_first_contact: total_first,
        total_followup,
        unique_contacts,
        debug_emails,
        recent_7_days,
        followup_rate,
    };

    Json(ApiResponse::success(stats))
}
