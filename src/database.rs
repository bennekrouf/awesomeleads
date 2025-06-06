use duckdb::{Connection, Result as DuckResult, params};
use mobc::{Manager, Pool};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::info;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProject {
    pub id: Option<i64>,
    pub url: String,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub repo_name: Option<String>,
    pub repository_created: Option<String>,
    pub first_commit_date: Option<String>,
    pub email: Option<String>,
    pub email_source: Option<String>,
    pub source_repository: String,
    pub scraped_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
    pub last_commit_date: Option<String>,
    pub top_contributor_email: Option<String>,
    pub top_contributor_commits: Option<i32>,
    pub total_commits: Option<i32>,
}

pub struct DuckDBManager {
    db_path: String,
}

impl DuckDBManager {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }
}

#[async_trait::async_trait]
impl Manager for DuckDBManager {
    type Connection = Connection;
    type Error = duckdb::Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let conn = Connection::open(&self.db_path)?;
        
        // Initialize tables if they don't exist
        init_database(&conn)?;
        
        Ok(conn)
    }

    async fn check(&self, conn: Self::Connection) -> Result<Self::Connection, Self::Error> {
        // Simple check - try to execute a basic query
        conn.execute("SELECT 1", [])?;
        Ok(conn)
    }
}

pub type DbPool = Pool<DuckDBManager>;

pub async fn create_db_pool(db_path: &str) -> Result<DbPool, Box<dyn std::error::Error + Send + Sync>> {
    // Ensure directory exists
    if let Some(parent) = Path::new(db_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let manager = DuckDBManager::new(db_path.to_string());
    let pool = Pool::builder()
        .max_open(10)
        .max_idle(5)
        .build(manager);
    
    info!("✓ DuckDB connection pool created: {}", db_path);
    Ok(pool)
}

fn init_database(conn: &Connection) -> DuckResult<()> {
    info!("Initializing database schema...");
    
    // Create tables directly - DuckDB will handle IF NOT EXISTS
    create_projects_table(conn)?;
    create_contributors_table(conn)?;
    create_non_github_projects_table(conn)?;
    create_sources_table(conn)?;
    create_indexes(conn)?;

    info!("✓ Database schema initialized");
    Ok(())
}

fn create_projects_table(conn: &Connection) -> DuckResult<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS projects (
            id INTEGER,
            url VARCHAR UNIQUE NOT NULL,
            description VARCHAR,
            owner VARCHAR,
            repo_name VARCHAR,
            repository_created VARCHAR,
            first_commit_date VARCHAR,
            last_commit_date VARCHAR,
            email VARCHAR,
            email_source VARCHAR,
            top_contributor_email VARCHAR,
            top_contributor_commits INTEGER,
            total_commits INTEGER,
            source_repository VARCHAR NOT NULL,
            scraped_at VARCHAR NOT NULL,
            last_updated VARCHAR NOT NULL
        )
        "#,
        [],
    )?;
    Ok(())
}

// fn migrate_projects_table(conn: &Connection) -> DuckResult<()> {
//     // Add new columns if they don't exist
//     let columns_to_add = [
//         ("last_commit_date", "VARCHAR"),
//         ("top_contributor_email", "VARCHAR"),
//         ("top_contributor_commits", "INTEGER"),
//         ("total_commits", "INTEGER"),
//     ];
//
//     for (column_name, column_type) in columns_to_add {
//         let result = conn.execute(
//             &format!("ALTER TABLE projects ADD COLUMN {} {}", column_name, column_type),
//             [],
//         );
//
//         // Ignore errors if column already exists
//         if let Err(e) = result {
//             if !e.to_string().contains("already exists") && !e.to_string().contains("duplicate") {
//                 return Err(e);
//             }
//         }
//     }
//
//     Ok(())
// }

fn create_contributors_table(conn: &Connection) -> DuckResult<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS contributors (
            id INTEGER,
            project_url VARCHAR NOT NULL,
            email VARCHAR,
            name VARCHAR,
            commit_count INTEGER NOT NULL,
            first_commit_date VARCHAR,
            last_commit_date VARCHAR,
            created_at VARCHAR NOT NULL
        )
        "#,
        [],
    )?;
    Ok(())
}

fn create_non_github_projects_table(conn: &Connection) -> DuckResult<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS non_github_projects (
            id INTEGER,
            url VARCHAR UNIQUE NOT NULL,
            description VARCHAR,
            domain VARCHAR,
            project_type VARCHAR,
            source_repository VARCHAR NOT NULL,
            scraped_at VARCHAR NOT NULL,
            last_updated VARCHAR NOT NULL
        )
        "#,
        [],
    )?;
    Ok(())
}

fn create_sources_table(conn: &Connection) -> DuckResult<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS sources (
            id INTEGER,
            name VARCHAR UNIQUE NOT NULL,
            repository VARCHAR NOT NULL,
            last_scraped VARCHAR,
            total_github_projects INTEGER DEFAULT 0,
            total_non_github_projects INTEGER DEFAULT 0,
            created_at VARCHAR NOT NULL,
            updated_at VARCHAR NOT NULL
        )
        "#,
        [],
    )?;
    Ok(())
}

fn create_indexes(conn: &Connection) -> DuckResult<()> {
    // Create indexes for better performance
    let indexes = [
        "CREATE INDEX IF NOT EXISTS idx_projects_url ON projects(url)",
        "CREATE INDEX IF NOT EXISTS idx_projects_owner_repo ON projects(owner, repo_name)",
        "CREATE INDEX IF NOT EXISTS idx_projects_source ON projects(source_repository)",
        "CREATE INDEX IF NOT EXISTS idx_non_github_projects_url ON non_github_projects(url)",
        "CREATE INDEX IF NOT EXISTS idx_non_github_projects_domain ON non_github_projects(domain)",
        "CREATE INDEX IF NOT EXISTS idx_contributors_project_url ON contributors(project_url)",
        "CREATE INDEX IF NOT EXISTS idx_contributors_commit_count ON contributors(commit_count DESC)",
    ];

    for index_sql in indexes {
        conn.execute(index_sql, [])?;
    }

    Ok(())
}

pub async fn upsert_project(
    pool: &DbPool,
    project: &StoredProject,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get().await?;
    let now = Utc::now();
    
    // Handle Option fields
    let description = project.description.as_deref().unwrap_or("");
    let owner = project.owner.as_deref().unwrap_or("");
    let repo_name = project.repo_name.as_deref().unwrap_or("");
    let repository_created = project.repository_created.as_deref().unwrap_or("");
    let first_commit_date = project.first_commit_date.as_deref().unwrap_or("");
    let last_commit_date = project.last_commit_date.as_deref().unwrap_or("");
    let email = project.email.as_deref().unwrap_or("");
    let email_source = project.email_source.as_deref().unwrap_or("");
    let top_contributor_email = project.top_contributor_email.as_deref().unwrap_or("");

    conn.execute(
        r#"
        INSERT INTO projects (
            url, description, owner, repo_name, repository_created,
            first_commit_date, last_commit_date, email, email_source, 
            top_contributor_email, top_contributor_commits, total_commits,
            source_repository, scraped_at, last_updated
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        ON CONFLICT (url) DO UPDATE SET
            description = EXCLUDED.description,
            owner = EXCLUDED.owner,
            repo_name = EXCLUDED.repo_name,
            repository_created = EXCLUDED.repository_created,
            first_commit_date = EXCLUDED.first_commit_date,
            last_commit_date = EXCLUDED.last_commit_date,
            email = EXCLUDED.email,
            email_source = EXCLUDED.email_source,
            top_contributor_email = EXCLUDED.top_contributor_email,
            top_contributor_commits = EXCLUDED.top_contributor_commits,
            total_commits = EXCLUDED.total_commits,
            last_updated = EXCLUDED.last_updated
        "#,
        params![
            project.url,
            description,
            owner,
            repo_name,
            repository_created,
            first_commit_date,
            last_commit_date,
            email,
            email_source,
            top_contributor_email,
            project.top_contributor_commits,
            project.total_commits,
            project.source_repository,
            project.scraped_at.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;
    
    Ok(())
}

// FIXED: Proper column retrieval for integer fields
pub async fn get_project_by_url(
    pool: &DbPool,
    url: &str,
) -> Result<Option<StoredProject>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get().await?;
    
    let mut stmt = conn.prepare(
        "SELECT url, description, owner, repo_name, repository_created, 
                first_commit_date, last_commit_date, email, email_source,
                top_contributor_email, top_contributor_commits, total_commits,
                source_repository, scraped_at, last_updated 
         FROM projects WHERE url = ?"
    )?;
    
    let mut rows = stmt.query([url])?;
    
    if let Some(row) = rows.next()? {
        // Helper function to safely get optional string
        let get_optional_string = |idx: usize| -> Option<String> {
            match row.get::<_, String>(idx) {
                Ok(s) if !s.is_empty() => Some(s),
                _ => None,
            }
        };

        // Helper function to safely get optional integer
        let get_optional_i32 = |idx: usize| -> Option<i32> {
            row.get::<_, Option<i32>>(idx).unwrap_or(None)
        };

        let scraped_at_str: String = row.get(13)?;
        let last_updated_str: String = row.get(14)?;
        
        Ok(Some(StoredProject {
            id: None,
            url: row.get(0)?,                               // url
            description: get_optional_string(1),            // description
            owner: get_optional_string(2),                  // owner
            repo_name: get_optional_string(3),              // repo_name
            repository_created: get_optional_string(4),     // repository_created
            first_commit_date: get_optional_string(5),      // first_commit_date
            last_commit_date: get_optional_string(6),       // last_commit_date
            email: get_optional_string(7),                  // email
            email_source: get_optional_string(8),           // email_source
            top_contributor_email: get_optional_string(9),  // top_contributor_email
            top_contributor_commits: get_optional_i32(10),  // top_contributor_commits: INTEGER
            total_commits: get_optional_i32(11),            // total_commits: INTEGER
            source_repository: row.get(12)?,                // source_repository
            scraped_at: DateTime::parse_from_rfc3339(&scraped_at_str)?.with_timezone(&Utc),
            last_updated: DateTime::parse_from_rfc3339(&last_updated_str)?.with_timezone(&Utc),
        }))
    } else {
        Ok(None)
    }
}

// FIXED: Proper retrieval for projects needing GitHub data
pub async fn get_projects_needing_github_data(
    pool: &DbPool,
    max_age_hours: i64,
) -> Result<Vec<StoredProject>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get().await?;
    let cutoff_time = Utc::now() - chrono::Duration::hours(max_age_hours);
    
    let mut stmt = conn.prepare(
        r#"
        SELECT url, description, owner, repo_name, repository_created, 
               first_commit_date, last_commit_date, email, email_source,
               top_contributor_email, top_contributor_commits, total_commits,
               source_repository, scraped_at, last_updated 
        FROM projects 
        WHERE (email IS NULL OR email = '' OR first_commit_date IS NULL OR repository_created IS NULL)
        AND (last_updated < ? OR last_updated IS NULL)
        AND owner IS NOT NULL 
        AND repo_name IS NOT NULL
        ORDER BY last_updated ASC NULLS FIRST
        "#
    )?;
    
    let rows = stmt.query_map([cutoff_time.to_rfc3339()], |row| {
        // Helper functions
        let get_optional_string = |idx: usize| -> Option<String> {
            match row.get::<_, String>(idx) {
                Ok(s) if !s.is_empty() => Some(s),
                _ => None,
            }
        };

        let get_optional_i32 = |idx: usize| -> Option<i32> {
            row.get::<_, Option<i32>>(idx).unwrap_or(None)
        };

        let scraped_at_str: String = row.get(13)?;
        let last_updated_str: String = row.get(14)?;
        
        Ok(StoredProject {
            id: None,
            url: row.get(0)?,                               // url
            description: get_optional_string(1),            // description
            owner: get_optional_string(2),                  // owner
            repo_name: get_optional_string(3),              // repo_name
            repository_created: get_optional_string(4),     // repository_created
            first_commit_date: get_optional_string(5),      // first_commit_date
            last_commit_date: get_optional_string(6),       // last_commit_date
            email: get_optional_string(7),                  // email
            email_source: get_optional_string(8),           // email_source
            top_contributor_email: get_optional_string(9),  // top_contributor_email
            top_contributor_commits: get_optional_i32(10),  // top_contributor_commits
            total_commits: get_optional_i32(11),            // total_commits
            source_repository: row.get(12)?,                // source_repository
            scraped_at: DateTime::parse_from_rfc3339(&scraped_at_str).unwrap().with_timezone(&Utc),
            last_updated: DateTime::parse_from_rfc3339(&last_updated_str).unwrap().with_timezone(&Utc),
        })
    })?;
    
    let mut projects = Vec::new();
    for row in rows {
        projects.push(row?);
    }
    
    Ok(projects)
}

pub async fn upsert_contributors(
    pool: &DbPool,
    project_url: &str,
    contributors: &[crate::models::ContributorInfo],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get().await?;
    let now = Utc::now();
    
    // Clear existing contributors for this project
    conn.execute(
        "DELETE FROM contributors WHERE project_url = ?1",
        params![project_url],
    )?;
    
    // Insert new contributors
    for contributor in contributors {
        let email = contributor.email.as_deref().unwrap_or("");
        let name = contributor.name.as_deref().unwrap_or("");
        let first_commit_date = contributor.first_commit_date.as_deref().unwrap_or("");
        let last_commit_date = contributor.last_commit_date.as_deref().unwrap_or("");
        
        conn.execute(
            r#"
            INSERT INTO contributors (
                project_url, email, name, commit_count, 
                first_commit_date, last_commit_date, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                project_url,
                email,
                name,
                contributor.commit_count,
                first_commit_date,
                last_commit_date,
                now.to_rfc3339(),
            ],
        )?;
    }
    
    Ok(())
}

// Rest of the database functions remain the same...
pub async fn update_source_last_scraped(
    pool: &DbPool,
    source_name: &str,
    repository: &str,
    github_project_count: i64,
    non_github_project_count: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get().await?;
    let now = Utc::now();
    
    conn.execute(
        r#"
        INSERT OR REPLACE INTO sources (name, repository, last_scraped, total_github_projects, total_non_github_projects, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            source_name,
            repository,
            now.to_rfc3339(),
            github_project_count,
            non_github_project_count,
            now.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;
    
    Ok(())
}

pub async fn get_database_stats(
    pool: &DbPool,
) -> Result<DatabaseStats, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get().await?;
    
    let total_github_projects: i64 = conn.query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))?;
    let total_non_github_projects: i64 = conn.query_row("SELECT COUNT(*) FROM non_github_projects", [], |row| row.get(0))?;
    let projects_with_email: i64 = conn.query_row("SELECT COUNT(*) FROM projects WHERE email IS NOT NULL AND email != ''", [], |row| row.get(0))?;
    let projects_with_github_data: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE (repository_created IS NOT NULL AND repository_created != '') AND (first_commit_date IS NOT NULL AND first_commit_date != '')", 
        [], 
        |row| row.get(0)
    )?;
    let projects_with_contributor_data: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE top_contributor_email IS NOT NULL AND top_contributor_email != ''", 
        [], 
        |row| row.get(0)
    )?;
    let projects_with_commit_stats: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE total_commits IS NOT NULL AND total_commits > 0", 
        [], 
        |row| row.get(0)
    )?;
    let avg_commits_per_project: f64 = conn.query_row(
        "SELECT AVG(CAST(total_commits AS FLOAT)) FROM projects WHERE total_commits IS NOT NULL AND total_commits > 0", 
        [], 
        |row| row.get(0)
    ).unwrap_or(0.0);
    
    // Get source info
    let mut stmt = conn.prepare("SELECT name, last_scraped, total_github_projects, total_non_github_projects FROM sources ORDER BY last_scraped DESC")?;
    let source_rows = stmt.query_map([], |row| {
        Ok(SourceInfo {
            name: row.get(0)?,
            last_scraped: row.get::<_, Option<String>>(1)?.map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
            total_github_projects: row.get(2)?,
            total_non_github_projects: row.get(3)?,
        })
    })?;
    
    let mut sources = Vec::new();
    for row in source_rows {
        sources.push(row?);
    }
    
    Ok(DatabaseStats {
        total_github_projects,
        total_non_github_projects,
        projects_with_email,
        projects_with_github_data,
        projects_with_contributor_data,
        projects_with_commit_stats,
        avg_commits_per_project,
        sources,
    })
}

#[derive(Debug, Serialize)]
pub struct DatabaseStats {
    pub total_github_projects: i64,
    pub total_non_github_projects: i64,
    pub projects_with_email: i64,
    pub projects_with_github_data: i64,
    pub projects_with_contributor_data: i64,
    pub projects_with_commit_stats: i64,
    pub avg_commits_per_project: f64,
    pub sources: Vec<SourceInfo>,
}

#[derive(Debug, Serialize)]
pub struct SourceInfo {
    pub name: String,
    pub last_scraped: Option<DateTime<Utc>>,
    pub total_github_projects: i64,
    pub total_non_github_projects: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredNonGithubProject {
    pub id: Option<i64>,
    pub url: String,
    pub description: Option<String>,
    pub domain: Option<String>,
    pub project_type: Option<String>,
    pub source_repository: String,
    pub scraped_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

pub async fn upsert_non_github_project(
    pool: &DbPool,
    project: &StoredNonGithubProject,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get().await?;
    let now = Utc::now();
    
    let description = project.description.as_deref().unwrap_or("");
    let domain = project.domain.as_deref().unwrap_or("");
    let project_type = project.project_type.as_deref().unwrap_or("");
    
    conn.execute(
        r#"
        INSERT INTO non_github_projects (
            url, description, domain, project_type, source_repository,
            scraped_at, last_updated
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT (url) DO UPDATE SET
            description = EXCLUDED.description,
            domain = EXCLUDED.domain,
            project_type = EXCLUDED.project_type,
            last_updated = EXCLUDED.last_updated
        "#,
        params![
            project.url,
            description,
            domain,
            project_type,
            project.source_repository,
            project.scraped_at.to_rfc3339(),
            now.to_rfc3339(),
        ],
    )?;
    
    Ok(())
}

pub async fn get_non_github_project_by_url(
    pool: &DbPool,
    url: &str,
) -> Result<Option<StoredNonGithubProject>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get().await?;
    
    let mut stmt = conn.prepare(
        "SELECT url, description, domain, project_type, source_repository, 
                scraped_at, last_updated 
         FROM non_github_projects WHERE url = ?"
    )?;
    
    let mut rows = stmt.query([url])?;
    
    if let Some(row) = rows.next()? {
        let get_optional_string = |idx: usize| -> Option<String> {
            match row.get::<_, String>(idx) {
                Ok(s) if !s.is_empty() => Some(s),
                _ => None,
            }
        };

        let scraped_at_str: String = row.get(5)?;
        let last_updated_str: String = row.get(6)?;
        
        Ok(Some(StoredNonGithubProject {
            id: None,
            url: row.get(0)?,
            description: get_optional_string(1),
            domain: get_optional_string(2),
            project_type: get_optional_string(3),
            source_repository: row.get(4)?,
            scraped_at: DateTime::parse_from_rfc3339(&scraped_at_str)?.with_timezone(&Utc),
            last_updated: DateTime::parse_from_rfc3339(&last_updated_str)?.with_timezone(&Utc),
        }))
    } else {
        Ok(None)
    }
}
