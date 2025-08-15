// src/api/leads.rs
use crate::api::stats::ApiResponse;
use crate::server::ServerState;
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, serde::json::Json, State};

#[derive(Serialize, Deserialize)]
pub struct Lead {
    pub email: String,
    pub name: Option<String>,
    pub project_url: String,
    pub repo_name: Option<String>,
    pub owner: Option<String>,
    pub description: Option<String>,
    pub total_commits: Option<i32>,
    pub engagement_score: f64,
    pub contact_source: String,
    pub last_updated: String,
}

#[derive(Serialize)]
pub struct LeadsResponse {
    pub leads: Vec<Lead>,
    pub total_count: usize,
    pub page: usize,
    pub per_page: usize,
}

#[derive(Serialize, Deserialize)]
pub struct EmailStatus {
    pub email: String,
    pub status: String,
    pub templates_sent: Vec<String>,
    pub last_sent: Option<String>,
    pub can_send_first: bool,
    pub can_send_followup: bool,
}

#[derive(Serialize)]
pub struct FollowupCandidate {
    pub email: String,
    pub name: Option<String>,
    pub repo_name: Option<String>,
    pub first_email_sent: String,
    pub days_since_first: i64,
}

#[get("/leads?<page>&<per_page>&<min_commits>&<has_email>&<engagement_min>")]
pub async fn get_leads(
    state: &State<ServerState>,
    page: Option<usize>,
    per_page: Option<usize>,
    min_commits: Option<i32>,
    has_email: Option<bool>,
    engagement_min: Option<f64>,
) -> Json<ApiResponse<LeadsResponse>> {
    let page = page.unwrap_or(1);
    let per_page = per_page.unwrap_or(50).min(1000);
    let offset = (page - 1) * per_page;

    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Build WHERE clause for lead qualification
    let mut where_conditions = vec![
        "email IS NOT NULL",
        "email != ''",
        "email NOT LIKE '%noreply%'",
        "email NOT LIKE '%no-reply%'",
    ];

    let mut params = Vec::new();

    if let Some(min_commits_val) = min_commits {
        where_conditions.push("total_commits >= ?");
        params.push(min_commits_val.to_string());
    } else {
        // Default minimum for qualified leads
        where_conditions.push("total_commits >= ?");
        params.push("5".to_string());
    }

    // Calculate engagement score in query
    let engagement_calc = "
        CASE 
            WHEN total_commits > 100 THEN 0.9
            WHEN total_commits > 50 THEN 0.8
            WHEN total_commits > 20 THEN 0.7
            WHEN total_commits > 10 THEN 0.6
            WHEN total_commits > 5 THEN 0.5
            ELSE 0.3
        END + 
        CASE 
            WHEN repository_created > '2022-01-01' THEN 0.1
            ELSE 0.0
        END
    ";
    let con = format!("({}) >= ?", engagement_calc);
    if let Some(eng_min) = engagement_min {
        where_conditions.push(&con);
        params.push(eng_min.to_string());
    }

    let where_clause = where_conditions.join(" AND ");

    let query = format!(
        "SELECT 
            email,
            COALESCE(owner, substr(email, 1, instr(email, '@') - 1)) as name,
            url as project_url,
            repo_name,
            owner,
            description,
            total_commits,
            ({}) as engagement_score,
            'github_scraper' as contact_source,
            last_updated
         FROM projects 
         WHERE {}
         ORDER BY engagement_score DESC, total_commits DESC, last_updated DESC
         LIMIT {} OFFSET {}",
        engagement_calc, where_clause, per_page, offset
    );

    let mut stmt = match conn.prepare(&query) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let lead_iter = match stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok(Lead {
            email: row.get(0)?,
            name: row.get(1)?,
            project_url: row.get(2)?,
            repo_name: row.get(3)?,
            owner: row.get(4)?,
            description: row.get(5)?,
            total_commits: row.get(6)?,
            engagement_score: row.get(7)?,
            contact_source: row.get(8)?,
            last_updated: row.get(9)?,
        })
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut leads = Vec::new();
    for result in lead_iter {
        match result {
            Ok(lead) => leads.push(lead),
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    let len = leads.len();

    let response = LeadsResponse {
        leads,
        total_count: len,
        page,
        per_page,
    };

    Json(ApiResponse::success(response))
}

#[get("/leads/email-status/<email>")]
pub async fn get_email_status(
    state: &State<ServerState>,
    email: String,
) -> Json<ApiResponse<EmailStatus>> {
    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Check if email_tracking table exists
    let table_exists: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='email_tracking'",
        [],
        |row| row.get(0),
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    if table_exists == 0 {
        // Email tracking table doesn't exist, return default status
        let status = EmailStatus {
            email: email.clone(),
            status: "never_contacted".to_string(),
            templates_sent: Vec::new(),
            last_sent: None,
            can_send_first: true,
            can_send_followup: false,
        };
        return Json(ApiResponse::success(status));
    }

    let mut stmt = match conn.prepare(
        "SELECT template_name, sent_at FROM email_tracking WHERE email = ? ORDER BY sent_at DESC",
    ) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let rows = match stmt.query_map([&email], |row| {
        Ok((
            row.get::<_, String>(0)?, // template_name
            row.get::<_, String>(1)?, // sent_at
        ))
    }) {
        Ok(rows) => rows,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut templates_sent = Vec::new();
    let mut last_sent = None;

    for row in rows {
        match row {
            Ok((template, sent_at)) => {
                templates_sent.push(template);
                if last_sent.is_none() {
                    last_sent = Some(sent_at);
                }
            }
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    let can_send_first = !templates_sent.contains(&"investment_proposal".to_string());
    let can_send_followup = templates_sent.contains(&"investment_proposal".to_string())
        && !templates_sent.contains(&"follow_up".to_string());

    let status_str = if templates_sent.is_empty() {
        "never_contacted"
    } else if can_send_followup {
        "first_sent"
    } else {
        "completed"
    };

    let status = EmailStatus {
        email,
        status: status_str.to_string(),
        templates_sent,
        last_sent,
        can_send_first,
        can_send_followup,
    };

    Json(ApiResponse::success(status))
}

#[get("/leads/followup-candidates?<days_since_first>")]
pub async fn get_followup_candidates(
    state: &State<ServerState>,
    days_since_first: Option<i64>,
) -> Json<ApiResponse<Vec<FollowupCandidate>>> {
    let days_threshold = days_since_first.unwrap_or(7);

    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Check if email_tracking table exists
    let table_exists: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='email_tracking'",
        [],
        |row| row.get(0),
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    if table_exists == 0 {
        return Json(ApiResponse::success(Vec::new()));
    }

    let cutoff_date = (chrono::Utc::now() - chrono::Duration::days(days_threshold)).to_rfc3339();

    let mut stmt = match conn.prepare(
        r#"
        SELECT DISTINCT 
            et.email,
            p.owner as name,
            p.repo_name,
            et.sent_at,
            CAST((julianday('now') - julianday(et.sent_at)) AS INTEGER) as days_since_first
        FROM email_tracking et
        LEFT JOIN projects p ON et.email = p.email
        WHERE et.template_name = 'investment_proposal' 
        AND et.sent_at <= ?1
        AND et.email NOT IN (
            SELECT email FROM email_tracking WHERE template_name = 'follow_up'
        )
        ORDER BY et.sent_at ASC
        LIMIT 100
        "#,
    ) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let rows = match stmt.query_map([cutoff_date], |row| {
        Ok(FollowupCandidate {
            email: row.get(0)?,
            name: row.get(1)?,
            repo_name: row.get(2)?,
            first_email_sent: row.get(3)?,
            days_since_first: row.get(4)?,
        })
    }) {
        Ok(rows) => rows,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut candidates = Vec::new();
    for row in rows {
        match row {
            Ok(candidate) => candidates.push(candidate),
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    Json(ApiResponse::success(candidates))
}

#[get("/leads/business-contacts?<page>&<per_page>&<decision_makers_only>")]
pub async fn get_business_contacts(
    state: &State<ServerState>,
    page: Option<usize>,
    per_page: Option<usize>,
    decision_makers_only: Option<bool>,
) -> Json<ApiResponse<serde_json::Value>> {
    let page = page.unwrap_or(1);
    let per_page = per_page.unwrap_or(50).min(1000);
    let offset = (page - 1) * per_page;

    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Check if business tables exist
    let table_exists: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='business_contacts'",
        [],
        |row| row.get(0),
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    if table_exists == 0 {
        let empty_response = serde_json::json!({
            "contacts": [],
            "total_count": 0,
            "page": page,
            "per_page": per_page,
            "message": "Business contacts table not found. Run business crawler first."
        });
        return Json(ApiResponse::success(empty_response));
    }

    let where_clause = if decision_makers_only.unwrap_or(false) {
        "WHERE bc.is_decision_maker = 1"
    } else {
        "WHERE 1=1"
    };

    let query = format!(
        r#"
        SELECT 
            bc.email,
            bc.full_name,
            bc.job_title,
            bc.role_category,
            bc.seniority_level,
            bc.is_decision_maker,
            bc.confidence,
            c.name as company_name,
            c.domain,
            c.industry,
            bc.discovered_at
        FROM business_contacts bc
        JOIN companies c ON bc.company_id = c.id
        {}
        ORDER BY bc.confidence DESC, bc.is_decision_maker DESC
        LIMIT {} OFFSET {}
        "#,
        where_clause, per_page, offset
    );

    let mut stmt = match conn.prepare(&query) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let contact_iter = match stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "email": row.get::<_, String>(0)?,
            "full_name": row.get::<_, Option<String>>(1)?,
            "job_title": row.get::<_, Option<String>>(2)?,
            "role_category": row.get::<_, Option<String>>(3)?,
            "seniority_level": row.get::<_, Option<String>>(4)?,
            "is_decision_maker": row.get::<_, bool>(5)?,
            "confidence": row.get::<_, f64>(6)?,
            "company_name": row.get::<_, String>(7)?,
            "domain": row.get::<_, String>(8)?,
            "industry": row.get::<_, Option<String>>(9)?,
            "discovered_at": row.get::<_, String>(10)?
        }))
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut contacts = Vec::new();
    for result in contact_iter {
        match result {
            Ok(contact) => contacts.push(contact),
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    let response = serde_json::json!({
        "contacts": contacts,
        "total_count": contacts.len(),
        "page": page,
        "per_page": per_page
    });

    Json(ApiResponse::success(response))
}
