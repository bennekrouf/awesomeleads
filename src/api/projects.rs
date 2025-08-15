// src/api/projects.rs
use crate::api::stats::ApiResponse;
use crate::server::ServerState;
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, serde::json::Json, State};

#[derive(Serialize, Deserialize)]
pub struct Project {
    pub id: Option<i64>,
    pub url: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub repo_name: Option<String>,
    pub repository_created: Option<String>,
    pub first_commit_date: Option<String>,
    pub last_commit_date: Option<String>,
    pub email: Option<String>,
    pub email_source: Option<String>,
    pub top_contributor_email: Option<String>,
    pub top_contributor_commits: Option<i32>,
    pub total_commits: Option<i32>,
    pub source_repository: String,
    pub scraped_at: String,
    pub last_updated: String,
}

#[derive(Serialize)]
pub struct ProjectsResponse {
    pub projects: Vec<Project>,
    pub total_count: usize,
    pub page: usize,
    pub per_page: usize,
    pub has_more: bool,
}

#[derive(Serialize, Deserialize)]
pub struct NonGithubProject {
    pub id: Option<i64>,
    pub url: String,
    pub description: Option<String>,
    pub domain: Option<String>,
    pub project_type: Option<String>,
    pub source_repository: String,
    pub scraped_at: String,
    pub last_updated: String,
}

#[derive(Serialize)]
pub struct NonGithubProjectsResponse {
    pub projects: Vec<NonGithubProject>,
    pub total_count: usize,
    pub page: usize,
    pub per_page: usize,
}

#[get("/projects?<page>&<per_page>&<owner>&<repo>&<has_email>&<min_commits>")]
pub async fn get_projects(
    state: &State<ServerState>,
    page: Option<usize>,
    per_page: Option<usize>,
    owner: Option<String>,
    repo: Option<String>,
    has_email: Option<bool>,
    min_commits: Option<i32>,
) -> Json<ApiResponse<ProjectsResponse>> {
    let page = page.unwrap_or(1);
    let per_page = per_page.unwrap_or(50).min(1000);
    let offset = (page - 1) * per_page;

    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Build WHERE clause based on filters
    let mut where_conditions = vec!["1=1"];
    let mut params = Vec::new();

    if let Some(owner_filter) = &owner {
        where_conditions.push("owner LIKE ?");
        params.push(format!("%{}%", owner_filter));
    }

    if let Some(repo_filter) = &repo {
        where_conditions.push("repo_name LIKE ?");
        params.push(format!("%{}%", repo_filter));
    }

    if let Some(true) = has_email {
        where_conditions.push("email IS NOT NULL AND email != ''");
    }

    if let Some(min_commits_val) = min_commits {
        where_conditions.push("total_commits >= ?");
        params.push(min_commits_val.to_string());
    }

    let where_clause = where_conditions.join(" AND ");

    let query = format!(
        "SELECT url, description, owner, repo_name, repository_created, 
                first_commit_date, last_commit_date, email, email_source,
                top_contributor_email, top_contributor_commits, total_commits,
                source_repository, scraped_at, last_updated
         FROM projects 
         WHERE {}
         ORDER BY total_commits DESC NULLS LAST, last_updated DESC
         LIMIT {} OFFSET {}",
        where_clause, per_page, offset
    );

    let mut stmt = match conn.prepare(&query) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let project_iter = match stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok(Project {
            id: None,
            url: row.get(0)?,
            description: row.get(1)?,
            owner: row.get(2)?,
            repo_name: row.get(3)?,
            repository_created: row.get(4)?,
            first_commit_date: row.get(5)?,
            last_commit_date: row.get(6)?,
            email: row.get(7)?,
            email_source: row.get(8)?,
            top_contributor_email: row.get(9)?,
            top_contributor_commits: row.get(10)?,
            total_commits: row.get(11)?,
            source_repository: row.get(12)?,
            scraped_at: row.get(13)?,
            last_updated: row.get(14)?,
        })
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut projects = Vec::new();
    for result in project_iter {
        match result {
            Ok(project) => projects.push(project),
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    let len = projects.len();
    let has_more = len == per_page;

    let response = ProjectsResponse {
        projects,
        total_count: len,
        page,
        per_page,
        has_more,
    };

    Json(ApiResponse::success(response))
}

#[get("/projects/search?<q>&<limit>")]
pub async fn search_projects(
    state: &State<ServerState>,
    q: Option<String>,
    limit: Option<usize>,
) -> Json<ApiResponse<Vec<Project>>> {
    let query_term = match q {
        Some(term) if !term.is_empty() => term,
        _ => {
            return Json(ApiResponse::error(
                "Query parameter 'q' is required".to_string(),
            ))
        }
    };

    let limit = limit.unwrap_or(20).min(100);

    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let search_query = format!(
        "SELECT url, description, owner, repo_name, repository_created, 
                first_commit_date, last_commit_date, email, email_source,
                top_contributor_email, top_contributor_commits, total_commits,
                source_repository, scraped_at, last_updated
         FROM projects 
         WHERE (
             url LIKE ?1 OR 
             description LIKE ?1 OR 
             owner LIKE ?1 OR 
             repo_name LIKE ?1
         )
         ORDER BY 
             CASE WHEN owner LIKE ?1 THEN 1 ELSE 2 END,
             CASE WHEN repo_name LIKE ?1 THEN 1 ELSE 2 END,
             total_commits DESC NULLS LAST
         LIMIT {}",
        limit
    );

    let search_param = format!("%{}%", query_term);

    let mut stmt = match conn.prepare(&search_query) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let project_iter = match stmt.query_map([&search_param], |row| {
        Ok(Project {
            id: None,
            url: row.get(0)?,
            description: row.get(1)?,
            owner: row.get(2)?,
            repo_name: row.get(3)?,
            repository_created: row.get(4)?,
            first_commit_date: row.get(5)?,
            last_commit_date: row.get(6)?,
            email: row.get(7)?,
            email_source: row.get(8)?,
            top_contributor_email: row.get(9)?,
            top_contributor_commits: row.get(10)?,
            total_commits: row.get(11)?,
            source_repository: row.get(12)?,
            scraped_at: row.get(13)?,
            last_updated: row.get(14)?,
        })
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut projects = Vec::new();
    for result in project_iter {
        match result {
            Ok(project) => projects.push(project),
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    Json(ApiResponse::success(projects))
}

#[get("/projects/non-github?<page>&<per_page>&<project_type>&<domain>")]
pub async fn get_non_github_projects(
    state: &State<ServerState>,
    page: Option<usize>,
    per_page: Option<usize>,
    project_type: Option<String>,
    domain: Option<String>,
) -> Json<ApiResponse<NonGithubProjectsResponse>> {
    let page = page.unwrap_or(1);
    let per_page = per_page.unwrap_or(50).min(1000);
    let offset = (page - 1) * per_page;

    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Build WHERE clause
    let mut where_conditions = vec!["1=1"];
    let mut params = Vec::new();

    if let Some(type_filter) = &project_type {
        where_conditions.push("project_type = ?");
        params.push(type_filter.clone());
    }

    if let Some(domain_filter) = &domain {
        where_conditions.push("domain LIKE ?");
        params.push(format!("%{}%", domain_filter));
    }

    let where_clause = where_conditions.join(" AND ");

    let query = format!(
        "SELECT url, description, domain, project_type, source_repository, 
                scraped_at, last_updated
         FROM non_github_projects 
         WHERE {}
         ORDER BY last_updated DESC
         LIMIT {} OFFSET {}",
        where_clause, per_page, offset
    );

    let mut stmt = match conn.prepare(&query) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let project_iter = match stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok(NonGithubProject {
            id: None,
            url: row.get(0)?,
            description: row.get(1)?,
            domain: row.get(2)?,
            project_type: row.get(3)?,
            source_repository: row.get(4)?,
            scraped_at: row.get(5)?,
            last_updated: row.get(6)?,
        })
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut projects = Vec::new();
    for result in project_iter {
        match result {
            Ok(project) => projects.push(project),
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    let len = projects.len();

    let response = NonGithubProjectsResponse {
        projects,
        total_count: len,
        page,
        per_page,
    };

    Json(ApiResponse::success(response))
}

