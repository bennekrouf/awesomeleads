use chrono::{DateTime, Utc};
use mobc::{Manager, Pool};
use rusqlite::{params, Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, error, info};

// Add this debug helper
fn log_rusqlite_error(context: &str, err: &rusqlite::Error) {
    error!("🔥 SQLite Error in {}: {:?}", context, err);

    if let rusqlite::Error::ExecuteReturnedResults = err {
        error!(
            "💥 EXECUTE_RETURNED_RESULTS: This means execute() was called on a SELECT statement!"
        );
        error!("🔧 Solution: Use query_row() or query_map() for SELECT statements");
    }
}

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

pub struct SqliteManager {
    db_path: String,
}

impl SqliteManager {
    pub fn new(db_path: String) -> Self {
        debug!("🔧 Creating SqliteManager for path: {}", db_path);
        Self { db_path }
    }
}

#[async_trait::async_trait]
impl Manager for SqliteManager {
    type Connection = Connection;
    type Error = rusqlite::Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        debug!(
            "🔌 SqliteManager::connect() - Opening database: {}",
            self.db_path
        );

        let conn = match Connection::open(&self.db_path) {
            Ok(c) => {
                debug!("✅ Database connection opened successfully");
                c
            }
            Err(e) => {
                log_rusqlite_error("Connection::open", &e);
                return Err(e);
            }
        };

        debug!("⚙️ Setting PRAGMA options...");

        // Helper function to execute PRAGMA statements safely
        let exec_pragma =
            |conn: &Connection, pragma: &str, name: &str| -> Result<(), rusqlite::Error> {
                debug!("🔧 Executing PRAGMA: {}", pragma);
                match conn.execute(pragma, []) {
                    Ok(_) => {
                        debug!("✅ {} (via execute)", name);
                        Ok(())
                    }
                    Err(rusqlite::Error::ExecuteReturnedResults) => {
                        // Some PRAGMA statements return results, try query_row
                        debug!("🔄 {} returned results, trying query_row", name);
                        match conn.query_row(pragma, [], |_| Ok(())) {
                            Ok(_) => {
                                debug!("✅ {} (via query_row)", name);
                                Ok(())
                            }
                            Err(e) => {
                                debug!("❌ {} failed with query_row: {}", name, e);
                                Err(e)
                            }
                        }
                    }
                    Err(e) => {
                        debug!("❌ {} failed with execute: {}", name, e);
                        Err(e)
                    }
                }
            };

        exec_pragma(&conn, "PRAGMA journal_mode=WAL", "PRAGMA journal_mode")?;
        exec_pragma(&conn, "PRAGMA synchronous=NORMAL", "PRAGMA synchronous")?;
        exec_pragma(&conn, "PRAGMA cache_size=1000000", "PRAGMA cache_size")?;
        exec_pragma(&conn, "PRAGMA temp_store=memory", "PRAGMA temp_store")?;
        exec_pragma(&conn, "PRAGMA mmap_size=268435456", "PRAGMA mmap_size")?;

        debug!("🏗️ Initializing database schema...");
        if let Err(e) = init_database(&conn) {
            log_rusqlite_error("init_database", &e);
            return Err(e);
        }
        debug!("✅ Database schema initialized");

        debug!("✅ SqliteManager::connect() completed successfully");
        Ok(conn)
    }

    async fn check(&self, conn: Self::Connection) -> Result<Self::Connection, Self::Error> {
        debug!("🔍 SqliteManager::check() - Testing connection...");

        match conn.query_row("SELECT 1", [], |_| Ok(())) {
            Ok(_) => {
                debug!("✅ Connection check passed");
                Ok(conn)
            }
            Err(e) => {
                log_rusqlite_error("connection check", &e);
                Err(e)
            }
        }
    }
}

fn init_database(conn: &Connection) -> SqliteResult<()> {
    debug!("🏗️ init_database() - Creating tables and indexes...");

    debug!("📋 Creating projects table...");
    if let Err(e) = create_projects_table(conn) {
        log_rusqlite_error("create_projects_table", &e);
        return Err(e);
    }

    debug!("👥 Creating contributors table...");
    if let Err(e) = create_contributors_table(conn) {
        log_rusqlite_error("create_contributors_table", &e);
        return Err(e);
    }

    debug!("🌐 Creating non_github_projects table...");
    if let Err(e) = create_non_github_projects_table(conn) {
        log_rusqlite_error("create_non_github_projects_table", &e);
        return Err(e);
    }

    debug!("📚 Creating sources table...");
    if let Err(e) = create_sources_table(conn) {
        log_rusqlite_error("create_sources_table", &e);
        return Err(e);
    }

    debug!("🔗 Creating indexes...");
    if let Err(e) = create_indexes(conn) {
        log_rusqlite_error("create_indexes", &e);
        return Err(e);
    }

    create_email_tracking_table(conn)?;
    create_email_tracking_indexes(conn)?;

    debug!("✅ init_database() completed successfully");
    Ok(())
}

pub type DbPool = Pool<SqliteManager>;

pub async fn create_db_pool(
    db_path: &str,
) -> Result<DbPool, Box<dyn std::error::Error + Send + Sync>> {
    debug!(
        "🏊 create_db_pool() - Creating connection pool for: {}",
        db_path
    );

    // Ensure directory exists
    if let Some(parent) = Path::new(db_path).parent() {
        debug!("📁 Creating directory: {:?}", parent);
        tokio::fs::create_dir_all(parent).await?;
    }

    let manager = SqliteManager::new(db_path.to_string());
    let pool = Pool::builder().max_open(10).max_idle(5).build(manager);

    info!("✓ SQLite connection pool created: {}", db_path);
    Ok(pool)
}

fn create_projects_table(conn: &Connection) -> SqliteResult<()> {
    debug!("📋 Creating projects table...");
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT UNIQUE NOT NULL,
            description TEXT,
            owner TEXT,
            repo_name TEXT,
            repository_created TEXT,
            first_commit_date TEXT,
            last_commit_date TEXT,
            email TEXT,
            email_source TEXT,
            top_contributor_email TEXT,
            top_contributor_commits INTEGER,
            total_commits INTEGER,
            source_repository TEXT NOT NULL,
            scraped_at TEXT NOT NULL,
            last_updated TEXT NOT NULL
        )
        "#,
        [],
    )?;
    debug!("✅ Projects table created");
    Ok(())
}

fn create_contributors_table(conn: &Connection) -> SqliteResult<()> {
    debug!("👥 Creating contributors table...");
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS contributors (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_url TEXT NOT NULL,
            email TEXT,
            name TEXT,
            commit_count INTEGER NOT NULL,
            first_commit_date TEXT,
            last_commit_date TEXT,
            created_at TEXT NOT NULL
        )
        "#,
        [],
    )?;
    debug!("✅ Contributors table created");
    Ok(())
}

fn create_non_github_projects_table(conn: &Connection) -> SqliteResult<()> {
    debug!("🌐 Creating non_github_projects table...");
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS non_github_projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            url TEXT UNIQUE NOT NULL,
            description TEXT,
            domain TEXT,
            project_type TEXT,
            source_repository TEXT NOT NULL,
            scraped_at TEXT NOT NULL,
            last_updated TEXT NOT NULL
        )
        "#,
        [],
    )?;
    debug!("✅ Non-GitHub projects table created");
    Ok(())
}

fn create_sources_table(conn: &Connection) -> SqliteResult<()> {
    debug!("📚 Creating sources table...");
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL,
            repository TEXT NOT NULL,
            last_scraped TEXT,
            total_github_projects INTEGER DEFAULT 0,
            total_non_github_projects INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )
        "#,
        [],
    )?;
    debug!("✅ Sources table created");
    Ok(())
}

fn create_indexes(conn: &Connection) -> SqliteResult<()> {
    debug!("🔗 Creating database indexes...");
    let indexes = [
        "CREATE INDEX IF NOT EXISTS idx_projects_url ON projects(url)",
        "CREATE INDEX IF NOT EXISTS idx_projects_owner_repo ON projects(owner, repo_name)",
        "CREATE INDEX IF NOT EXISTS idx_projects_source ON projects(source_repository)",
        "CREATE INDEX IF NOT EXISTS idx_projects_email ON projects(email)",
        "CREATE INDEX IF NOT EXISTS idx_projects_commits ON projects(total_commits DESC)",
        "CREATE INDEX IF NOT EXISTS idx_non_github_projects_url ON non_github_projects(url)",
        "CREATE INDEX IF NOT EXISTS idx_non_github_projects_domain ON non_github_projects(domain)",
        "CREATE INDEX IF NOT EXISTS idx_contributors_project_url ON contributors(project_url)",
        "CREATE INDEX IF NOT EXISTS idx_contributors_commit_count ON contributors(commit_count DESC)",
    ];

    for (i, index_sql) in indexes.iter().enumerate() {
        debug!(
            "🔗 Creating index {}/{}: {}",
            i + 1,
            indexes.len(),
            index_sql
        );
        if let Err(e) = conn.execute(index_sql, []) {
            log_rusqlite_error(&format!("create index {}", i + 1), &e);
            return Err(e);
        }
    }

    debug!("✅ All indexes created successfully");
    Ok(())
}

// HEAVILY INSTRUMENTED get_database_stats function
pub async fn get_database_stats(
    pool: &DbPool,
) -> Result<DatabaseStats, Box<dyn std::error::Error + Send + Sync>> {
    debug!("📊 get_database_stats() - Starting database statistics collection...");

    let conn = match pool.get().await {
        Ok(c) => {
            debug!("✅ Database connection acquired from pool");
            c
        }
        Err(e) => {
            error!("💥 Failed to get connection from pool: {}", e);
            return Err(Box::new(e));
        }
    };

    debug!("🔍 Checking if tables exist...");

    let table_exists = |table_name: &str| -> Result<bool, rusqlite::Error> {
        debug!("🔍 Checking if table '{}' exists...", table_name);
        let query = "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1";
        debug!("📝 Query: {} [table_name: {}]", query, table_name);

        match conn.query_row(query, [table_name], |row| row.get::<_, i64>(0)) {
            Ok(count) => {
                debug!(
                    "✅ Table '{}' check result: {} (exists: {})",
                    table_name,
                    count,
                    count > 0
                );
                Ok(count > 0)
            }
            Err(e) => {
                log_rusqlite_error(&format!("table_exists check for '{}'", table_name), &e);
                Err(e)
            }
        }
    };

    let projects_table_exists = match table_exists("projects") {
        Ok(exists) => {
            debug!("📋 Projects table exists: {}", exists);
            exists
        }
        Err(e) => {
            log_rusqlite_error("projects table existence check", &e);
            return Err(Box::new(e));
        }
    };

    let non_github_table_exists = match table_exists("non_github_projects") {
        Ok(exists) => {
            debug!("🌐 Non-GitHub projects table exists: {}", exists);
            exists
        }
        Err(e) => {
            log_rusqlite_error("non_github_projects table existence check", &e);
            return Err(Box::new(e));
        }
    };

    let sources_table_exists = match table_exists("sources") {
        Ok(exists) => {
            debug!("📚 Sources table exists: {}", exists);
            exists
        }
        Err(e) => {
            log_rusqlite_error("sources table existence check", &e);
            return Err(Box::new(e));
        }
    };

    debug!("📊 Collecting statistics...");

    // Get counts using query_row with detailed logging
    let total_github_projects = if projects_table_exists {
        debug!("📊 Counting total GitHub projects...");
        let query = "SELECT COUNT(*) FROM projects";
        debug!("📝 Query: {}", query);

        match conn.query_row(query, [], |row| row.get::<_, i64>(0)) {
            Ok(count) => {
                debug!("✅ Total GitHub projects: {}", count);
                count
            }
            Err(e) => {
                log_rusqlite_error("total_github_projects count", &e);
                return Err(Box::new(e));
            }
        }
    } else {
        debug!("⏭️ Projects table doesn't exist, returning 0");
        0
    };

    let total_non_github_projects = if non_github_table_exists {
        debug!("📊 Counting total non-GitHub projects...");
        let query = "SELECT COUNT(*) FROM non_github_projects";
        debug!("📝 Query: {}", query);

        match conn.query_row(query, [], |row| row.get::<_, i64>(0)) {
            Ok(count) => {
                debug!("✅ Total non-GitHub projects: {}", count);
                count
            }
            Err(e) => {
                log_rusqlite_error("total_non_github_projects count", &e);
                return Err(Box::new(e));
            }
        }
    } else {
        debug!("⏭️ Non-GitHub projects table doesn't exist, returning 0");
        0
    };

    let projects_with_email = if projects_table_exists {
        debug!("📧 Counting projects with email...");
        let query = "SELECT COUNT(*) FROM projects WHERE email IS NOT NULL AND email != ''";
        debug!("📝 Query: {}", query);

        match conn.query_row(query, [], |row| row.get::<_, i64>(0)) {
            Ok(count) => {
                debug!("✅ Projects with email: {}", count);
                count
            }
            Err(e) => {
                log_rusqlite_error("projects_with_email count", &e);
                return Err(Box::new(e));
            }
        }
    } else {
        debug!("⏭️ Projects table doesn't exist, returning 0");
        0
    };

    let projects_with_github_data = if projects_table_exists {
        debug!("📊 Counting projects with GitHub data...");
        let query = "SELECT COUNT(*) FROM projects WHERE (repository_created IS NOT NULL AND repository_created != '') AND (first_commit_date IS NOT NULL AND first_commit_date != '')";
        debug!("📝 Query: {}", query);

        match conn.query_row(query, [], |row| row.get::<_, i64>(0)) {
            Ok(count) => {
                debug!("✅ Projects with GitHub data: {}", count);
                count
            }
            Err(e) => {
                log_rusqlite_error("projects_with_github_data count", &e);
                return Err(Box::new(e));
            }
        }
    } else {
        debug!("⏭️ Projects table doesn't exist, returning 0");
        0
    };

    let projects_with_contributor_data = if projects_table_exists {
        debug!("👥 Counting projects with contributor data...");
        let query = "SELECT COUNT(*) FROM projects WHERE top_contributor_email IS NOT NULL AND top_contributor_email != ''";
        debug!("📝 Query: {}", query);

        match conn.query_row(query, [], |row| row.get::<_, i64>(0)) {
            Ok(count) => {
                debug!("✅ Projects with contributor data: {}", count);
                count
            }
            Err(e) => {
                log_rusqlite_error("projects_with_contributor_data count", &e);
                return Err(Box::new(e));
            }
        }
    } else {
        debug!("⏭️ Projects table doesn't exist, returning 0");
        0
    };

    let projects_with_commit_stats = if projects_table_exists {
        debug!("📈 Counting projects with commit stats...");
        let query =
            "SELECT COUNT(*) FROM projects WHERE total_commits IS NOT NULL AND total_commits > 0";
        debug!("📝 Query: {}", query);

        match conn.query_row(query, [], |row| row.get::<_, i64>(0)) {
            Ok(count) => {
                debug!("✅ Projects with commit stats: {}", count);
                count
            }
            Err(e) => {
                log_rusqlite_error("projects_with_commit_stats count", &e);
                return Err(Box::new(e));
            }
        }
    } else {
        debug!("⏭️ Projects table doesn't exist, returning 0");
        0
    };

    let avg_commits_per_project: f64 = if projects_table_exists {
        debug!("📊 Calculating average commits per project...");
        let query = "SELECT AVG(CAST(total_commits AS REAL)) FROM projects WHERE total_commits IS NOT NULL AND total_commits > 0";
        debug!("📝 Query: {}", query);

        match conn.query_row(query, [], |row| row.get::<_, Option<f64>>(0)) {
            Ok(Some(avg)) => {
                debug!("✅ Average commits per project: {}", avg);
                avg
            }
            Ok(None) => {
                debug!("ℹ️ No projects with commits found, average is 0");
                0.0
            }
            Err(e) => {
                log_rusqlite_error("avg_commits_per_project calculation", &e);
                return Err(Box::new(e));
            }
        }
    } else {
        debug!("⏭️ Projects table doesn't exist, returning 0.0");
        0.0
    };

    // Get source info
    debug!("📚 Collecting source information...");
    let mut sources = Vec::new();

    if sources_table_exists {
        debug!("📝 Preparing sources query...");
        let query = "SELECT name, last_scraped, total_github_projects, total_non_github_projects FROM sources ORDER BY last_scraped DESC";
        debug!("📝 Query: {}", query);

        let mut stmt = match conn.prepare(query) {
            Ok(s) => {
                debug!("✅ Sources query prepared successfully");
                s
            }
            Err(e) => {
                log_rusqlite_error("sources query preparation", &e);
                return Err(Box::new(e));
            }
        };

        debug!("🔄 Executing sources query...");
        let source_iter = match stmt.query_map([], |row| {
            let last_scraped_str: Option<String> = row.get(1)?;
            let last_scraped = match last_scraped_str {
                Some(s) => DateTime::parse_from_rfc3339(&s)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc)),
                None => None,
            };

            Ok(SourceInfo {
                name: row.get(0)?,
                last_scraped,
                total_github_projects: row.get(2)?,
                total_non_github_projects: row.get(3)?,
            })
        }) {
            Ok(iter) => {
                debug!("✅ Sources query executed successfully");
                iter
            }
            Err(e) => {
                log_rusqlite_error("sources query execution", &e);
                return Err(Box::new(e));
            }
        };

        debug!("🔄 Processing source results...");
        for (i, source_result) in source_iter.enumerate() {
            match source_result {
                Ok(source) => {
                    debug!("✅ Processed source {}: {}", i + 1, source.name);
                    sources.push(source);
                }
                Err(e) => {
                    log_rusqlite_error(&format!("processing source {}", i + 1), &e);
                    return Err(Box::new(e));
                }
            }
        }
        debug!("✅ All {} sources processed", sources.len());
    } else {
        debug!("⏭️ Sources table doesn't exist, returning empty sources list");
    }

    debug!("🎯 Creating DatabaseStats result...");
    let stats = DatabaseStats {
        total_github_projects,
        total_non_github_projects,
        projects_with_email,
        projects_with_github_data,
        projects_with_contributor_data,
        projects_with_commit_stats,
        avg_commits_per_project,
        sources,
    };

    debug!("✅ get_database_stats() completed successfully");
    Ok(stats)
}

// Add logging to other functions too
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

pub async fn upsert_project(
    pool: &DbPool,
    project: &StoredProject,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!("💾 upsert_project() - Upserting project: {}", project.url);

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

    match conn.execute(
        r#"
        INSERT INTO projects (
            url, description, owner, repo_name, repository_created,
            first_commit_date, last_commit_date, email, email_source, 
            top_contributor_email, top_contributor_commits, total_commits,
            source_repository, scraped_at, last_updated
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        ON CONFLICT (url) DO UPDATE SET
            description = COALESCE(NULLIF(excluded.description, ''), description),
            owner = COALESCE(NULLIF(excluded.owner, ''), owner),
            repo_name = COALESCE(NULLIF(excluded.repo_name, ''), repo_name),
            repository_created = COALESCE(NULLIF(excluded.repository_created, ''), repository_created),
            first_commit_date = COALESCE(NULLIF(excluded.first_commit_date, ''), first_commit_date),
            last_commit_date = COALESCE(NULLIF(excluded.last_commit_date, ''), last_commit_date),
            email = COALESCE(NULLIF(excluded.email, ''), email),
            email_source = COALESCE(NULLIF(excluded.email_source, ''), email_source),
            top_contributor_email = COALESCE(NULLIF(excluded.top_contributor_email, ''), top_contributor_email),
            top_contributor_commits = COALESCE(excluded.top_contributor_commits, top_contributor_commits),
            total_commits = COALESCE(excluded.total_commits, total_commits),
            last_updated = excluded.last_updated
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
    ) {
        Ok(_) => {
            debug!("✅ Project upserted successfully: {}", project.url);
            Ok(())
        }
        Err(e) => {
            log_rusqlite_error("upsert_project", &e);
            Err(Box::new(e))
        }
    }
}

pub async fn get_project_by_url(
    pool: &DbPool,
    url: &str,
) -> Result<Option<StoredProject>, Box<dyn std::error::Error + Send + Sync>> {
    debug!("🔍 get_project_by_url() - Looking for: {}", url);

    let conn = pool.get().await?;

    let mut stmt = conn.prepare(
        "SELECT url, description, owner, repo_name, repository_created, 
                first_commit_date, last_commit_date, email, email_source,
                top_contributor_email, top_contributor_commits, total_commits,
                source_repository, scraped_at, last_updated 
         FROM projects WHERE url = ?",
    )?;

    let mut project_iter = stmt.query_map([url], |row| {
        let get_optional_string = |idx: usize| -> Option<String> {
            match row.get::<_, Option<String>>(idx) {
                Ok(Some(s)) if !s.is_empty() => Some(s),
                _ => None,
            }
        };

        let get_optional_i32 =
            |idx: usize| -> Option<i32> { row.get::<_, Option<i32>>(idx).unwrap_or(None) };

        let scraped_at_str: String = row.get(13)?;
        let last_updated_str: String = row.get(14)?;

        let scraped_at = DateTime::parse_from_rfc3339(&scraped_at_str)
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    13,
                    scraped_at_str.clone(),
                    rusqlite::types::Type::Text,
                )
            })?
            .with_timezone(&Utc);
        let last_updated = DateTime::parse_from_rfc3339(&last_updated_str)
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    14,
                    last_updated_str.clone(),
                    rusqlite::types::Type::Text,
                )
            })?
            .with_timezone(&Utc);

        Ok(StoredProject {
            id: None,
            url: row.get(0)?,
            description: get_optional_string(1),
            owner: get_optional_string(2),
            repo_name: get_optional_string(3),
            repository_created: get_optional_string(4),
            first_commit_date: get_optional_string(5),
            last_commit_date: get_optional_string(6),
            email: get_optional_string(7),
            email_source: get_optional_string(8),
            top_contributor_email: get_optional_string(9),
            top_contributor_commits: get_optional_i32(10),
            total_commits: get_optional_i32(11),
            source_repository: row.get(12)?,
            scraped_at,
            last_updated,
        })
    })?;

    if let Some(project) = project_iter.next() {
        let project = project?;
        debug!("✅ Found project: {}", project.url);
        return Ok(Some(project));
    }

    debug!("❌ Project not found: {}", url);
    Ok(None)
}

pub async fn get_projects_needing_github_data(
    pool: &DbPool,
    max_age_hours: i64,
) -> Result<Vec<StoredProject>, Box<dyn std::error::Error + Send + Sync>> {
    debug!(
        "🔍 get_projects_needing_github_data() - max_age_hours: {}",
        max_age_hours
    );

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
        ORDER BY last_updated ASC
        "#
    )?;

    let project_iter = stmt.query_map([cutoff_time.to_rfc3339()], |row| {
        let get_optional_string = |idx: usize| -> Option<String> {
            match row.get::<_, Option<String>>(idx) {
                Ok(Some(s)) if !s.is_empty() => Some(s),
                _ => None,
            }
        };

        let get_optional_i32 =
            |idx: usize| -> Option<i32> { row.get::<_, Option<i32>>(idx).unwrap_or(None) };

        let scraped_at_str: String = row.get(13)?;
        let last_updated_str: String = row.get(14)?;

        let scraped_at = DateTime::parse_from_rfc3339(&scraped_at_str)
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    13,
                    scraped_at_str.clone(),
                    rusqlite::types::Type::Text,
                )
            })?
            .with_timezone(&Utc);
        let last_updated = DateTime::parse_from_rfc3339(&last_updated_str)
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    14,
                    last_updated_str.clone(),
                    rusqlite::types::Type::Text,
                )
            })?
            .with_timezone(&Utc);

        Ok(StoredProject {
            id: None,
            url: row.get(0)?,
            description: get_optional_string(1),
            owner: get_optional_string(2),
            repo_name: get_optional_string(3),
            repository_created: get_optional_string(4),
            first_commit_date: get_optional_string(5),
            last_commit_date: get_optional_string(6),
            email: get_optional_string(7),
            email_source: get_optional_string(8),
            top_contributor_email: get_optional_string(9),
            top_contributor_commits: get_optional_i32(10),
            total_commits: get_optional_i32(11),
            source_repository: row.get(12)?,
            scraped_at,
            last_updated,
        })
    })?;

    let mut projects = Vec::new();
    for project in project_iter {
        projects.push(project?);
    }

    debug!("✅ Found {} projects needing GitHub data", projects.len());
    Ok(projects)
}

pub async fn upsert_contributors(
    pool: &DbPool,
    project_url: &str,
    contributors: &[crate::models::ContributorInfo],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!(
        "👥 upsert_contributors() - project: {}, contributors: {}",
        project_url,
        contributors.len()
    );

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

    debug!(
        "✅ Upserted {} contributors for {}",
        contributors.len(),
        project_url
    );
    Ok(())
}

pub async fn update_source_last_scraped(
    pool: &DbPool,
    source_name: &str,
    repository: &str,
    github_project_count: i64,
    non_github_project_count: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!(
        "📚 update_source_last_scraped() - source: {}, repo: {}, github: {}, non-github: {}",
        source_name, repository, github_project_count, non_github_project_count
    );

    let conn = pool.get().await?;
    let now = Utc::now();

    conn.execute(
        r#"
        INSERT INTO sources (name, repository, last_scraped, total_github_projects, total_non_github_projects, created_at, updated_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT (name) DO UPDATE SET
            repository = excluded.repository,
            last_scraped = excluded.last_scraped,
            total_github_projects = excluded.total_github_projects,
            total_non_github_projects = excluded.total_non_github_projects,
            updated_at = excluded.updated_at
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

    debug!("✅ Source last scraped updated: {}", source_name);
    Ok(())
}

pub async fn upsert_non_github_project(
    pool: &DbPool,
    project: &StoredNonGithubProject,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!(
        "💾 upsert_non_github_project() - Upserting project: {}",
        project.url
    );

    let conn = pool.get().await?;
    let now = Utc::now();

    let description = project.description.as_deref().unwrap_or("");
    let domain = project.domain.as_deref().unwrap_or("");
    let project_type = project.project_type.as_deref().unwrap_or("");

    match conn.execute(
        r#"
        INSERT INTO non_github_projects (
            url, description, domain, project_type, source_repository,
            scraped_at, last_updated
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT (url) DO UPDATE SET
            description = COALESCE(NULLIF(excluded.description, ''), description),
            domain = COALESCE(NULLIF(excluded.domain, ''), domain),
            project_type = COALESCE(NULLIF(excluded.project_type, ''), project_type),
            last_updated = excluded.last_updated
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
    ) {
        Ok(_) => {
            debug!(
                "✅ Non-GitHub project upserted successfully: {}",
                project.url
            );
            Ok(())
        }
        Err(e) => {
            log_rusqlite_error("upsert_non_github_project", &e);
            Err(Box::new(e))
        }
    }
}

pub async fn get_non_github_project_by_url(
    pool: &DbPool,
    url: &str,
) -> Result<Option<StoredNonGithubProject>, Box<dyn std::error::Error + Send + Sync>> {
    debug!("🔍 get_non_github_project_by_url() - Looking for: {}", url);

    let conn = pool.get().await?;

    let mut stmt = conn.prepare(
        "SELECT url, description, domain, project_type, source_repository, 
                scraped_at, last_updated 
         FROM non_github_projects WHERE url = ?",
    )?;

    let project_iter = stmt.query_map([url], |row| {
        let get_optional_string = |idx: usize| -> Option<String> {
            match row.get::<_, Option<String>>(idx) {
                Ok(Some(s)) if !s.is_empty() => Some(s),
                _ => None,
            }
        };

        let scraped_at_str: String = row.get(5)?;
        let last_updated_str: String = row.get(6)?;

        let scraped_at = DateTime::parse_from_rfc3339(&scraped_at_str)
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    5,
                    scraped_at_str.clone(),
                    rusqlite::types::Type::Text,
                )
            })?
            .with_timezone(&Utc);
        let last_updated = DateTime::parse_from_rfc3339(&last_updated_str)
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    6,
                    last_updated_str.clone(),
                    rusqlite::types::Type::Text,
                )
            })?
            .with_timezone(&Utc);

        Ok(StoredNonGithubProject {
            id: None,
            url: row.get(0)?,
            description: get_optional_string(1),
            domain: get_optional_string(2),
            project_type: get_optional_string(3),
            source_repository: row.get(4)?,
            scraped_at,
            last_updated,
        })
    })?;

    for project in project_iter {
        let project = project?;
        debug!("✅ Found non-GitHub project: {}", project.url);
        return Ok(Some(project));
    }

    debug!("❌ Non-GitHub project not found: {}", url);
    Ok(None)
}

fn create_email_tracking_table(conn: &Connection) -> SqliteResult<()> {
    debug!("📧 Creating email_tracking table...");
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS email_tracking (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL,
            template_name TEXT NOT NULL,
            sent_at TEXT NOT NULL,
            campaign_type TEXT,
            mailgun_id TEXT,
            status TEXT DEFAULT 'sent',
            UNIQUE(email, template_name)
        )
        "#,
        [],
    )?;
    debug!("✅ Email tracking table created");
    Ok(())
}

// Add index for fast lookups
fn create_email_tracking_indexes(conn: &Connection) -> SqliteResult<()> {
    let indexes = [
        "CREATE INDEX IF NOT EXISTS idx_email_tracking_email ON email_tracking(email)",
        "CREATE INDEX IF NOT EXISTS idx_email_tracking_template ON email_tracking(template_name)",
        "CREATE INDEX IF NOT EXISTS idx_email_tracking_sent_at ON email_tracking(sent_at DESC)",
    ];

    for index_sql in indexes.iter() {
        conn.execute(index_sql, [])?;
    }
    Ok(())
}

// Update your init_database function to include this table
// Add these calls in init_database():
// create_email_tracking_table(conn)?;
// create_email_tracking_indexes(conn)?;
