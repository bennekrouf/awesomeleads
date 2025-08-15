// src/server/mod.rs - Updated with all routes
use crate::api::*;
use crate::config::Config;
use crate::database::DbPool;
use rocket::{routes, Build, Rocket};

pub mod routes;

pub struct ServerState {
    pub config: Config,
    pub db_pool: DbPool,
}

pub fn build_rocket(config: Config, db_pool: DbPool) -> Rocket<Build> {
    let state = ServerState { config, db_pool };

    rocket::build().manage(state).mount(
        "/api",
        routes![
            // Health and info endpoints
            routes::health::health_check,
            routes::health::index,
            // Stats endpoints
            get_stats,
            get_phase2_progress,
            get_email_stats,
            // Projects endpoints
            get_projects,
            search_projects,
            get_non_github_projects,
            // Leads endpoints
            get_leads,
            get_email_status,
            get_followup_candidates,
            get_business_contacts,
            // Companies endpoints
            get_companies,
            get_company_detail,
            search_companies,
            get_investment_targets,
            // Crawl endpoints
            get_crawl_results,
            get_extracted_contacts,
            get_crawl_stats,
            // Sources endpoints
            get_sources,
            get_source_detail,
        ],
    )
}
