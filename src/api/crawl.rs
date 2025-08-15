// src/api/crawl.rs
use crate::api::stats::ApiResponse;
use crate::server::ServerState;
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, serde::json::Json, State};

#[derive(Serialize, Deserialize)]
pub struct CrawlResult {
    pub id: i64,
    pub original_url: String,
    pub pages_crawled: i32,
    pub contacts_found: i32,
    pub crawl_duration_ms: i64,
    pub success: bool,
    pub error_message: Option<String>,
    pub crawled_at: String,
}

#[derive(Serialize)]
pub struct CrawlResultsResponse {
    pub results: Vec<CrawlResult>,
    pub total_count: usize,
    pub page: usize,
    pub per_page: usize,
    pub summary: CrawlSummary,
}

#[derive(Serialize)]
pub struct CrawlSummary {
    pub total_crawls: i64,
    pub successful_crawls: i64,
    pub total_contacts_found: i64,
    pub avg_contacts_per_site: f64,
    pub success_rate: f64,
}

#[derive(Serialize)]
pub struct ExtractedContact {
    pub contact_type: String,
    pub value: String,
    pub context: String,
    pub confidence: f32,
    pub source_url: String,
}

#[get("/crawl/results?<page>&<per_page>&<success_only>")]
pub async fn get_crawl_results(
    state: &State<ServerState>,
    page: Option<usize>,
    per_page: Option<usize>,
    success_only: Option<bool>,
) -> Json<ApiResponse<CrawlResultsResponse>> {
    let page = page.unwrap_or(1);
    let per_page = per_page.unwrap_or(50).min(1000);
    let offset = (page - 1) * per_page;
    let success_only = success_only.unwrap_or(false);

    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Check if crawl_results table exists
    let table_exists: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='crawl_results'",
        [],
        |row| row.get(0),
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    if table_exists == 0 {
        let empty_response = CrawlResultsResponse {
            results: Vec::new(),
            total_count: 0,
            page,
            per_page,
            summary: CrawlSummary {
                total_crawls: 0,
                successful_crawls: 0,
                total_contacts_found: 0,
                avg_contacts_per_site: 0.0,
                success_rate: 0.0,
            },
        };
        return Json(ApiResponse::success(empty_response));
    }

    let where_clause = if success_only {
        "WHERE success = 1"
    } else {
        ""
    };

    let query = format!(
        "SELECT id, original_url, pages_crawled, contacts_found, crawl_duration_ms, success, error_message, crawled_at
         FROM crawl_results 
         {}
         ORDER BY crawled_at DESC
         LIMIT {} OFFSET {}",
        where_clause, per_page, offset
    );

    let mut stmt = match conn.prepare(&query) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let result_iter = match stmt.query_map([], |row| {
        Ok(CrawlResult {
            id: row.get(0)?,
            original_url: row.get(1)?,
            pages_crawled: row.get(2)?,
            contacts_found: row.get(3)?,
            crawl_duration_ms: row.get(4)?,
            success: row.get(5)?,
            error_message: row.get(6)?,
            crawled_at: row.get(7)?,
        })
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut results = Vec::new();
    for result in result_iter {
        match result {
            Ok(r) => results.push(r),
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    // Get summary statistics
    let total_crawls: i64 = conn
        .query_row("SELECT COUNT(*) FROM crawl_results", [], |row| row.get(0))
        .unwrap_or(0);

    let successful_crawls: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM crawl_results WHERE success = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_contacts_found: i64 = conn
        .query_row(
            "SELECT SUM(contacts_found) FROM crawl_results WHERE success = 1",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .unwrap_or(Some(0))
        .unwrap_or(0);

    let avg_contacts_per_site = if successful_crawls > 0 {
        total_contacts_found as f64 / successful_crawls as f64
    } else {
        0.0
    };

    let success_rate = if total_crawls > 0 {
        (successful_crawls as f64 / total_crawls as f64) * 100.0
    } else {
        0.0
    };

    let summary = CrawlSummary {
        total_crawls,
        successful_crawls,
        total_contacts_found,
        avg_contacts_per_site,
        success_rate,
    };

    let len = results.len();

    let response = CrawlResultsResponse {
        results,
        total_count: len,
        page,
        per_page,
        summary,
    };

    Json(ApiResponse::success(response))
}

#[get("/crawl/contacts?<source_url>&<contact_type>")]
pub async fn get_extracted_contacts(
    state: &State<ServerState>,
    source_url: Option<String>,
    contact_type: Option<String>,
) -> Json<ApiResponse<Vec<ExtractedContact>>> {
    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Check if crawl_results table exists
    let table_exists: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='crawl_results'",
        [],
        |row| row.get(0),
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    if table_exists == 0 {
        return Json(ApiResponse::success(Vec::new()));
    }

    // Build query based on filters
    let mut where_conditions = vec![
        "success = 1",
        "best_contacts IS NOT NULL",
        "best_contacts != ''",
    ];
    let mut params = Vec::new();

    if let Some(url) = source_url {
        where_conditions.push("original_url = ?");
        params.push(url);
    }

    let where_clause = format!("WHERE {}", where_conditions.join(" AND "));

    let query = format!(
        "SELECT original_url, best_contacts FROM crawl_results {} ORDER BY crawled_at DESC LIMIT 1000",
        where_clause
    );

    let mut stmt = match conn.prepare(&query) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let contact_iter = match stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok((
            row.get::<_, String>(0)?, // original_url
            row.get::<_, String>(1)?, // best_contacts JSON
        ))
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut all_contacts = Vec::new();

    for row in contact_iter {
        match row {
            Ok((source_url, contacts_json)) => {
                // Parse the JSON contacts
                if let Ok(contacts) = serde_json::from_str::<Vec<serde_json::Value>>(&contacts_json)
                {
                    for contact in contacts {
                        if let (
                            Some(contact_type_val),
                            Some(value),
                            Some(context),
                            Some(confidence),
                        ) = (
                            contact.get("contact_type"),
                            contact.get("value"),
                            contact.get("context"),
                            contact.get("confidence"),
                        ) {
                            let contact_type_str = match contact_type_val {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Object(obj) => {
                                    // Handle enum-like structure
                                    if let Some(serde_json::Value::String(variant)) =
                                        obj.values().next()
                                    {
                                        variant.clone()
                                    } else {
                                        "unknown".to_string()
                                    }
                                }
                                _ => "unknown".to_string(),
                            };

                            // Filter by contact type if specified
                            if let Some(ref filter_type) = contact_type {
                                if !contact_type_str
                                    .to_lowercase()
                                    .contains(&filter_type.to_lowercase())
                                {
                                    continue;
                                }
                            }

                            let extracted_contact = ExtractedContact {
                                contact_type: contact_type_str,
                                value: value.as_str().unwrap_or("").to_string(),
                                context: context.as_str().unwrap_or("").to_string(),
                                confidence: confidence.as_f64().unwrap_or(0.0) as f32,
                                source_url: source_url.clone(),
                            };

                            all_contacts.push(extracted_contact);
                        }
                    }
                }
            }
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    // Sort by confidence
    all_contacts.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Json(ApiResponse::success(all_contacts))
}

#[get("/crawl/stats")]
pub async fn get_crawl_stats(state: &State<ServerState>) -> Json<ApiResponse<CrawlSummary>> {
    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Check if crawl_results table exists
    let table_exists: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='crawl_results'",
        [],
        |row| row.get(0),
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    if table_exists == 0 {
        let empty_summary = CrawlSummary {
            total_crawls: 0,
            successful_crawls: 0,
            total_contacts_found: 0,
            avg_contacts_per_site: 0.0,
            success_rate: 0.0,
        };
        return Json(ApiResponse::success(empty_summary));
    }

    let total_crawls: i64 = conn
        .query_row("SELECT COUNT(*) FROM crawl_results", [], |row| row.get(0))
        .unwrap_or(0);

    let successful_crawls: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM crawl_results WHERE success = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_contacts_found: i64 = conn
        .query_row(
            "SELECT SUM(contacts_found) FROM crawl_results WHERE success = 1",
            [],
            |row| row.get::<_, Option<i64>>(0),
        )
        .unwrap_or(Some(0))
        .unwrap_or(0);

    let avg_contacts_per_site = if successful_crawls > 0 {
        total_contacts_found as f64 / successful_crawls as f64
    } else {
        0.0
    };

    let success_rate = if total_crawls > 0 {
        (successful_crawls as f64 / total_crawls as f64) * 100.0
    } else {
        0.0
    };

    let summary = CrawlSummary {
        total_crawls,
        successful_crawls,
        total_contacts_found,
        avg_contacts_per_site,
        success_rate,
    };

    Json(ApiResponse::success(summary))
}
