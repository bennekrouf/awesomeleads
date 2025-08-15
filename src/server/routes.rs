// src/server/routes.rs
// This file can contain additional route configurations if needed
// For now, all routes are defined in their respective API modules

pub mod health {
    use rocket::{get, serde::json::Json};
    use serde_json::{json, Value};

    #[get("/health")]
    pub async fn health_check() -> Json<Value> {
        Json(json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "service": "lead-scraper-api"
        }))
    }

    #[get("/")]
    pub async fn index() -> Json<Value> {
        Json(json!({
            "name": "Lead Scraper API",
            "version": "0.1.0",
            "description": "API for accessing scraped project data and leads",
            "endpoints": {
                "health": "/api/health",
                "stats": "/api/stats",
                "projects": "/api/projects",
                "leads": "/api/leads",
                "companies": "/api/companies",
                "crawl": "/api/crawl",
                "sources": "/api/sources"
            }
        }))
    }
}
