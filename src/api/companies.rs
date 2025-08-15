// src/api/companies.rs
use crate::api::stats::ApiResponse;
use crate::server::ServerState;
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, serde::json::Json, State};

#[derive(Serialize, Deserialize)]
pub struct Company {
    pub id: i64,
    pub name: String,
    pub domain: String,
    pub website_url: String,
    pub company_type: Option<String>,
    pub industry: Option<String>,
    pub description: Option<String>,
    pub employee_count_estimate: Option<String>,
    pub funding_stage: Option<String>,
    pub location: Option<String>,
    pub founded_year: Option<i32>,
    pub discovered_from: String,
    pub confidence_score: f64,
    pub verified: bool,
    pub created_at: String,
    pub last_updated: String,
}

#[derive(Serialize)]
pub struct CompaniesResponse {
    pub companies: Vec<Company>,
    pub total_count: usize,
    pub page: usize,
    pub per_page: usize,
    pub summary: CompaniesSummary,
}

#[derive(Serialize)]
pub struct CompaniesSummary {
    pub total_companies: i64,
    pub verified_companies: i64,
    pub by_industry: std::collections::HashMap<String, i64>,
    pub by_type: std::collections::HashMap<String, i64>,
    pub avg_confidence: f64,
}

#[derive(Serialize)]
pub struct CompanyDetail {
    pub company: Company,
    pub contacts: Vec<serde_json::Value>,
    pub signals: Vec<serde_json::Value>,
    pub investment_score: Option<serde_json::Value>,
}

#[get("/companies?<page>&<per_page>&<industry>&<company_type>&<verified_only>&<min_confidence>")]
pub async fn get_companies(
    state: &State<ServerState>,
    page: Option<usize>,
    per_page: Option<usize>,
    industry: Option<String>,
    company_type: Option<String>,
    verified_only: Option<bool>,
    min_confidence: Option<f64>,
) -> Json<ApiResponse<CompaniesResponse>> {
    let page = page.unwrap_or(1);
    let per_page = per_page.unwrap_or(50).min(1000);
    let offset = (page - 1) * per_page;

    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Check if companies table exists
    let table_exists: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='companies'",
        [],
        |row| row.get(0),
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    if table_exists == 0 {
        let empty_response = CompaniesResponse {
            companies: Vec::new(),
            total_count: 0,
            page,
            per_page,
            summary: CompaniesSummary {
                total_companies: 0,
                verified_companies: 0,
                by_industry: std::collections::HashMap::new(),
                by_type: std::collections::HashMap::new(),
                avg_confidence: 0.0,
            },
        };
        return Json(ApiResponse::success(empty_response));
    }

    // Build WHERE clause
    let mut where_conditions = vec!["1=1"];
    let mut params = Vec::new();

    if let Some(industry_filter) = &industry {
        where_conditions.push("industry = ?");
        params.push(industry_filter.clone());
    }

    if let Some(type_filter) = &company_type {
        where_conditions.push("company_type = ?");
        params.push(type_filter.clone());
    }

    if let Some(true) = verified_only {
        where_conditions.push("verified = 1");
    }

    if let Some(min_conf) = min_confidence {
        where_conditions.push("confidence_score >= ?");
        params.push(min_conf.to_string());
    }

    let where_clause = where_conditions.join(" AND ");

    let query = format!(
        "SELECT id, name, domain, website_url, company_type, industry, description,
                employee_count_estimate, funding_stage, location, founded_year,
                discovered_from, confidence_score, verified, created_at, last_updated
         FROM companies 
         WHERE {}
         ORDER BY confidence_score DESC, verified DESC, last_updated DESC
         LIMIT {} OFFSET {}",
        where_clause, per_page, offset
    );

    let mut stmt = match conn.prepare(&query) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let company_iter = match stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok(Company {
            id: row.get(0)?,
            name: row.get(1)?,
            domain: row.get(2)?,
            website_url: row.get(3)?,
            company_type: row.get(4)?,
            industry: row.get(5)?,
            description: row.get(6)?,
            employee_count_estimate: row.get(7)?,
            funding_stage: row.get(8)?,
            location: row.get(9)?,
            founded_year: row.get(10)?,
            discovered_from: row.get(11)?,
            confidence_score: row.get(12)?,
            verified: row.get(13)?,
            created_at: row.get(14)?,
            last_updated: row.get(15)?,
        })
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut companies = Vec::new();
    for result in company_iter {
        match result {
            Ok(company) => companies.push(company),
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    // Get summary statistics
    let summary = get_companies_summary(&conn).unwrap_or_else(|_| CompaniesSummary {
        total_companies: 0,
        verified_companies: 0,
        by_industry: std::collections::HashMap::new(),
        by_type: std::collections::HashMap::new(),
        avg_confidence: 0.0,
    });

    let len = companies.len();
    let response = CompaniesResponse {
        companies,
        total_count: len,
        page,
        per_page,
        summary,
    };

    Json(ApiResponse::success(response))
}

#[get("/companies/<company_id>")]
pub async fn get_company_detail(
    state: &State<ServerState>,
    company_id: i64,
) -> Json<ApiResponse<CompanyDetail>> {
    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Get company info
    let company = match conn.query_row(
        "SELECT id, name, domain, website_url, company_type, industry, description,
                employee_count_estimate, funding_stage, location, founded_year,
                discovered_from, confidence_score, verified, created_at, last_updated
         FROM companies WHERE id = ?",
        [company_id],
        |row| {
            Ok(Company {
                id: row.get(0)?,
                name: row.get(1)?,
                domain: row.get(2)?,
                website_url: row.get(3)?,
                company_type: row.get(4)?,
                industry: row.get(5)?,
                description: row.get(6)?,
                employee_count_estimate: row.get(7)?,
                funding_stage: row.get(8)?,
                location: row.get(9)?,
                founded_year: row.get(10)?,
                discovered_from: row.get(11)?,
                confidence_score: row.get(12)?,
                verified: row.get(13)?,
                created_at: row.get(14)?,
                last_updated: row.get(15)?,
            })
        },
    ) {
        Ok(company) => company,
        Err(_) => return Json(ApiResponse::error("Company not found".to_string())),
    };

    // Get business contacts
    let mut contacts = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT email, full_name, job_title, role_category, is_decision_maker, confidence
         FROM business_contacts WHERE company_id = ? ORDER BY confidence DESC",
    ) {
        if let Ok(contact_iter) = stmt.query_map([company_id], |row| {
            Ok(serde_json::json!({
                "email": row.get::<_, String>(0)?,
                "full_name": row.get::<_, Option<String>>(1)?,
                "job_title": row.get::<_, Option<String>>(2)?,
                "role_category": row.get::<_, Option<String>>(3)?,
                "is_decision_maker": row.get::<_, bool>(4)?,
                "confidence": row.get::<_, f64>(5)?
            }))
        }) {
            for result in contact_iter {
                if let Ok(contact) = result {
                    contacts.push(contact);
                }
            }
        }
    }

    // Get company signals
    let mut signals = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT signal_type, signal_value, confidence, detected_at
         FROM company_signals WHERE company_id = ? ORDER BY detected_at DESC",
    ) {
        if let Ok(signal_iter) = stmt.query_map([company_id], |row| {
            Ok(serde_json::json!({
                "signal_type": row.get::<_, String>(0)?,
                "signal_value": row.get::<_, String>(1)?,
                "confidence": row.get::<_, f64>(2)?,
                "detected_at": row.get::<_, String>(3)?
            }))
        }) {
            for result in signal_iter {
                if let Ok(signal) = result {
                    signals.push(signal);
                }
            }
        }
    }

    // Get investment score
    let investment_score = conn
        .query_row(
            "SELECT total_score, growth_signals, tech_maturity, market_potential, 
                team_quality, contact_quality, last_calculated
         FROM investment_scores WHERE company_id = ?",
            [company_id],
            |row| {
                Ok(serde_json::json!({
                    "total_score": row.get::<_, i32>(0)?,
                    "growth_signals": row.get::<_, i32>(1)?,
                    "tech_maturity": row.get::<_, i32>(2)?,
                    "market_potential": row.get::<_, i32>(3)?,
                    "team_quality": row.get::<_, i32>(4)?,
                    "contact_quality": row.get::<_, i32>(5)?,
                    "last_calculated": row.get::<_, String>(6)?
                }))
            },
        )
        .ok();

    let detail = CompanyDetail {
        company,
        contacts,
        signals,
        investment_score,
    };

    Json(ApiResponse::success(detail))
}

#[get("/companies/search?<q>&<limit>")]
pub async fn search_companies(
    state: &State<ServerState>,
    q: Option<String>,
    limit: Option<usize>,
) -> Json<ApiResponse<Vec<Company>>> {
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

    // Check if companies table exists
    let table_exists: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='companies'",
        [],
        |row| row.get(0),
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    if table_exists == 0 {
        return Json(ApiResponse::success(Vec::new()));
    }

    let search_query = format!(
        "SELECT id, name, domain, website_url, company_type, industry, description,
                employee_count_estimate, funding_stage, location, founded_year,
                discovered_from, confidence_score, verified, created_at, last_updated
         FROM companies 
         WHERE (
             name LIKE ?1 OR 
             domain LIKE ?1 OR 
             description LIKE ?1 OR
             industry LIKE ?1
         )
         ORDER BY 
             CASE WHEN name LIKE ?1 THEN 1 ELSE 2 END,
             confidence_score DESC
         LIMIT {}",
        limit
    );

    let search_param = format!("%{}%", query_term);

    let mut stmt = match conn.prepare(&search_query) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let company_iter = match stmt.query_map([&search_param], |row| {
        Ok(Company {
            id: row.get(0)?,
            name: row.get(1)?,
            domain: row.get(2)?,
            website_url: row.get(3)?,
            company_type: row.get(4)?,
            industry: row.get(5)?,
            description: row.get(6)?,
            employee_count_estimate: row.get(7)?,
            funding_stage: row.get(8)?,
            location: row.get(9)?,
            founded_year: row.get(10)?,
            discovered_from: row.get(11)?,
            confidence_score: row.get(12)?,
            verified: row.get(13)?,
            created_at: row.get(14)?,
            last_updated: row.get(15)?,
        })
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut companies = Vec::new();
    for result in company_iter {
        match result {
            Ok(company) => companies.push(company),
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    Json(ApiResponse::success(companies))
}

#[get("/companies/investment-targets?<min_score>&<limit>")]
pub async fn get_investment_targets(
    state: &State<ServerState>,
    min_score: Option<i32>,
    limit: Option<usize>,
) -> Json<ApiResponse<Vec<serde_json::Value>>> {
    let min_score = min_score.unwrap_or(60);
    let limit = limit.unwrap_or(50).min(200);

    let conn = match state.db_pool.get().await {
        Ok(conn) => conn,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    // Check if investment_scores table exists
    let table_exists: i64 = match conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='investment_scores'",
        [],
        |row| row.get(0),
    ) {
        Ok(count) => count,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    if table_exists == 0 {
        return Json(ApiResponse::success(Vec::new()));
    }

    let query = format!(
        r#"
        SELECT 
            c.id,
            c.name,
            c.domain,
            c.industry,
            c.company_type,
            c.funding_stage,
            c.confidence_score,
            i.total_score,
            i.growth_signals,
            i.tech_maturity,
            i.market_potential,
            i.team_quality,
            i.contact_quality,
            COUNT(bc.id) as contact_count,
            SUM(CASE WHEN bc.is_decision_maker = 1 THEN 1 ELSE 0 END) as decision_maker_count
        FROM companies c
        JOIN investment_scores i ON c.id = i.company_id
        LEFT JOIN business_contacts bc ON c.id = bc.company_id
        WHERE i.total_score >= ?
        GROUP BY c.id
        ORDER BY i.total_score DESC
        LIMIT {}
        "#,
        limit
    );

    let mut stmt = match conn.prepare(&query) {
        Ok(stmt) => stmt,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let target_iter = match stmt.query_map([min_score], |row| {
        Ok(serde_json::json!({
            "company_id": row.get::<_, i64>(0)?,
            "name": row.get::<_, String>(1)?,
            "domain": row.get::<_, String>(2)?,
            "industry": row.get::<_, Option<String>>(3)?,
            "company_type": row.get::<_, Option<String>>(4)?,
            "funding_stage": row.get::<_, Option<String>>(5)?,
            "confidence_score": row.get::<_, f64>(6)?,
            "investment_score": {
                "total": row.get::<_, i32>(7)?,
                "growth_signals": row.get::<_, i32>(8)?,
                "tech_maturity": row.get::<_, i32>(9)?,
                "market_potential": row.get::<_, i32>(10)?,
                "team_quality": row.get::<_, i32>(11)?,
                "contact_quality": row.get::<_, i32>(12)?
            },
            "contact_count": row.get::<_, i32>(13)?,
            "decision_maker_count": row.get::<_, i32>(14)?
        }))
    }) {
        Ok(iter) => iter,
        Err(e) => return Json(ApiResponse::error(e.to_string())),
    };

    let mut targets = Vec::new();
    for result in target_iter {
        match result {
            Ok(target) => targets.push(target),
            Err(e) => return Json(ApiResponse::error(e.to_string())),
        }
    }

    Json(ApiResponse::success(targets))
}

fn get_companies_summary(conn: &rusqlite::Connection) -> Result<CompaniesSummary, rusqlite::Error> {
    let total_companies: i64 =
        conn.query_row("SELECT COUNT(*) FROM companies", [], |row| row.get(0))?;

    let verified_companies: i64 = conn.query_row(
        "SELECT COUNT(*) FROM companies WHERE verified = 1",
        [],
        |row| row.get(0),
    )?;

    let avg_confidence: f64 = conn
        .query_row("SELECT AVG(confidence_score) FROM companies", [], |row| {
            row.get::<_, Option<f64>>(0)
        })?
        .unwrap_or(0.0);

    // Get industry breakdown
    let mut by_industry = std::collections::HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT industry, COUNT(*) FROM companies WHERE industry IS NOT NULL GROUP BY industry",
    )?;
    let industry_iter = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    for result in industry_iter {
        let (industry, count) = result?;
        by_industry.insert(industry, count);
    }

    // Get company type breakdown
    let mut by_type = std::collections::HashMap::new();
    let mut stmt = conn.prepare("SELECT company_type, COUNT(*) FROM companies WHERE company_type IS NOT NULL GROUP BY company_type")?;
    let type_iter = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;

    for result in type_iter {
        let (company_type, count) = result?;
        by_type.insert(company_type, count);
    }

    Ok(CompaniesSummary {
        total_companies,
        verified_companies,
        by_industry,
        by_type,
        avg_confidence,
    })
}
