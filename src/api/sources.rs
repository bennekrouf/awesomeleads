// src/api/sources.rs
use crate::api::stats::ApiResponse;
use crate::server::ServerState;
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, serde::json::Json, State};

#[derive(Serialize, Deserialize)]
pub struct SourceInfo {
    pub name: String,
    pub repository: String,
    pub last_scraped: Option<String>,
    pub total_github_projects: i64,
    pub total_non_github_projects: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct SourcesResponse {
    pub sources: Vec<SourceInfo>,
    pub total_count: usize,
    pub summary: SourcesSummary,
}

#[derive(Serialize)]
pub struct SourcesSummary {
    pub total_sources: usize,
    pub total_github_projects: i64,
    pub total_non_github_projects: i64,
    pub recently_scraped: usize, // Within last 7 days
}

#[get("/sources")]
pub async fn get_sources(state: &State<ServerState>) -> Json<ApiResponse<SourcesResponse>> {
    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Check if sources table exists
    let table_exists: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sources'",
        [],
        |row| row.get(0),
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    if table_exists == 0 {
        let empty_response = SourcesResponse {
            sources: Vec::new(),
            total_count: 0,
            summary: SourcesSummary {
                total_sources: 0,
                total_github_projects: 0,
                total_non_github_projects: 0,
                recently_scraped: 0,
            },
        };
        return Json(ApiResponse::success(empty_response));
    }

    let mut stmt = match conn.prepare(
        "SELECT name, repository, last_scraped, total_github_projects, total_non_github_projects, created_at, updated_at
         FROM sources 
         ORDER BY last_scraped DESC NULLS LAST, name"
    ) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let source_iter = match stmt.query_map([], |row| {
        Ok(SourceInfo {
            name: row.get(0)?,
            repository: row.get(1)?,
            last_scraped: row.get(2)?,
            total_github_projects: row.get(3)?,
            total_non_github_projects: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut sources = Vec::new();
    let mut total_github = 0i64;
    let mut total_non_github = 0i64;
    let mut recently_scraped = 0usize;

    let seven_days_ago = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();

    for source in source_iter {
        match source {
            Ok(s) => {
                total_github += s.total_github_projects;
                total_non_github += s.total_non_github_projects;

                if let Some(ref last_scraped) = s.last_scraped {
                    if last_scraped > &seven_days_ago {
                        recently_scraped += 1;
                    }
                }

                sources.push(s);
            }
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }
    let len = sources.len();

    let summary = SourcesSummary {
        total_sources: len,
        total_github_projects: total_github,
        total_non_github_projects: total_non_github,
        recently_scraped,
    };

    let response = SourcesResponse {
        sources,
        total_count: len,
        summary,
    };

    Json(ApiResponse::success(response))
}

#[get("/sources/<source_name>")]
pub async fn get_source_detail(
    state: &State<ServerState>,
    source_name: String,
) -> Json<ApiResponse<SourceInfo>> {
    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let source_info = match conn.query_row(
        "SELECT name, repository, last_scraped, total_github_projects, total_non_github_projects, created_at, updated_at
         FROM sources WHERE name = ?",
        [&source_name],
        |row| {
            Ok(SourceInfo {
                name: row.get(0)?,
                repository: row.get(1)?,
                last_scraped: row.get(2)?,
                total_github_projects: row.get(3)?,
                total_non_github_projects: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        }
    ) {
        Ok(info) => info,
        Err(_) => return Json(ApiResponse::error("Source not found".to_string())),
    };

    Json(ApiResponse::success(source_info))
}
