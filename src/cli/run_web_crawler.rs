// src/cli/run_web_crawler.rs
use crate::models::CliApp;
use crate::web_crawler::{CrawlConfig, WebCrawler};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::sync::Arc;
use tokio::sync::Mutex;
// use tracing::{info, warn};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl CliApp {

    async fn filter_uncrawled_urls(&self, urls: &[String], max_age_days: i64) -> Result<(Vec<String>, Vec<String>)> {
        let conn = self.db_pool.get().await?;
        let cutoff_date = (chrono::Utc::now() - chrono::Duration::days(max_age_days)).to_rfc3339();
        
        let mut uncrawled = Vec::new();
        let mut recently_crawled = Vec::new();
        
        for url in urls {
            let exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM crawl_results WHERE original_url = ? AND crawled_at > ? AND success = 1",
                [url, &cutoff_date],
                |row| row.get(0),
            ).unwrap_or(0);
            
            if exists > 0 {
                recently_crawled.push(url.clone());
            } else {
                uncrawled.push(url.clone());
            }
        }
        
        Ok((uncrawled, recently_crawled))
    }

    pub async fn run_web_crawler(&self) -> Result<()> {
        println!("\n🕷️  Web Crawler for Contact Discovery");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Show available URLs
        let available_urls = self.get_non_github_urls().await?;
        
        if available_urls.is_empty() {
            println!("❌ No non-GitHub URLs found in database");
            println!("💡 Run Phase 1 first to collect project URLs");
            return Ok(());
        }

        println!("📊 Found {} non-GitHub URLs to crawl", available_urls.len());

        // Show sample URLs
        println!("\n📋 Sample URLs:");
        for (i, url) in available_urls.iter().take(5).enumerate() {
            println!("  {}. {}", i + 1, url);
        }
        if available_urls.len() > 5 {
            println!("  ... and {} more", available_urls.len() - 5);
        }

        // Crawl configuration
        let config = self.configure_crawl().await?;
        
        // URL selection
        let urls_to_crawl = self.select_urls_to_crawl(&available_urls).await?;
        
        if urls_to_crawl.is_empty() {
            println!("❌ No URLs selected for crawling");
            return Ok(());
        }

        println!(
            "\n🎯 Ready to crawl {} URLs with {} max pages each",
            urls_to_crawl.len(),
            config.max_pages
        );

        // Confirm crawl
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Start crawling?")
            .interact()?
        {
            println!("❌ Crawl cancelled");
            return Ok(());
        }

        // Perform crawl
        self.execute_crawl(&urls_to_crawl, config).await?;

        Ok(())
    }

    async fn get_non_github_urls(&self) -> Result<Vec<String>> {
        let conn = self.db_pool.get().await?;
        
        let mut stmt = conn.prepare(
            "SELECT url FROM non_github_projects 
             WHERE project_type IN ('website', 'tool', 'api', 'other')
             AND url NOT LIKE '%github.%'
             ORDER BY last_updated DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(row.get::<_, String>(0)?)
        })?;

        let mut urls = Vec::new();
        for row in rows {
            urls.push(row?);
        }

        Ok(urls)
    }

    async fn configure_crawl(&self) -> Result<CrawlConfig> {
        println!("\n⚙️  Crawl Configuration");
        
        let preset_options = vec![
            "🏃 Quick Scan (1 page per site, contact pages only)",
            "🔍 Standard Crawl (3 pages per site, all pages)",
            "🕵️ Deep Crawl (5 pages per site, all pages)",
            "⚙️ Custom Configuration",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select crawl configuration")
            .items(&preset_options)
            .interact()?;

        let config = match selection {
            0 => CrawlConfig {
                max_pages: 1,
                delay_ms: 1000,
                timeout_seconds: 30,
                respect_robots: true,
                contact_pages_only: true,
                follow_external_links: false,
            },
            1 => CrawlConfig {
                max_pages: 3,
                delay_ms: 2000,
                timeout_seconds: 60,
                respect_robots: true,
                contact_pages_only: false,
                follow_external_links: false,
            },
            2 => CrawlConfig {
                max_pages: 5,
                delay_ms: 3000,
                timeout_seconds: 90,
                respect_robots: true,
                contact_pages_only: false,
                follow_external_links: false,
            },
            3 => {
                // Custom configuration
                let max_pages: u32 = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Maximum pages per site")
                    .default(3)
                    .interact_text()?;

                let delay_ms: u64 = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Delay between requests (ms)")
                    .default(2000)
                    .interact_text()?;

                let timeout_seconds: u64 = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Timeout per site (seconds)")
                    .default(60)
                    .interact_text()?;

                let contact_pages_only = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Only crawl contact/about pages?")
                    .default(false)
                    .interact()?;

                CrawlConfig {
                    max_pages,
                    delay_ms,
                    timeout_seconds,
                    respect_robots: true,
                    contact_pages_only,
                    follow_external_links: false,
                }
            }
            _ => CrawlConfig::default(),
        };

        println!("✅ Configuration: {} pages, {}ms delay, {}s timeout", 
                 config.max_pages, config.delay_ms, config.timeout_seconds);
        
        Ok(config)
    }

    async fn select_urls_to_crawl(&self, available_urls: &[String]) -> Result<Vec<String>> {
        let selection_options = vec![
            format!("🎯 All URLs ({})", available_urls.len()),
            "🔝 Top 50 most recent".to_string(),
            "🔥 Top 20 high-quality sites".to_string(), 
            "🧪 Test with 5 URLs".to_string(),
            "📝 Enter specific URLs".to_string(),
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select URLs to crawl")
            .items(&selection_options)
            .interact()?;

        let urls = match selection {
            0 => {
                // All URLs - but confirm if too many
                if available_urls.len() > 100 {
                    println!("⚠️  Crawling {} URLs will take significant time", available_urls.len());
                    if !Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt("Continue with all URLs?")
                        .default(false)
                        .interact()?
                    {
                        return Ok(Vec::new());
                    }
                }
                available_urls.to_vec()
            },
            1 => available_urls.iter().take(50).cloned().collect(),
            2 => {
                // Filter for high-quality sites
                self.filter_high_quality_urls(available_urls).await?
                    .into_iter().take(20).collect()
            },
            3 => available_urls.iter().take(5).cloned().collect(),
            4 => {
                // Manual URL entry
                let mut custom_urls = Vec::new();
                loop {
                    let url: String = Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Enter URL (empty to finish)")
                        .allow_empty(true)
                        .interact_text()?;
                    
                    if url.is_empty() {
                        break;
                    }
                    
                    if url.starts_with("http") {
                        custom_urls.push(url);
                    } else {
                        println!("⚠️  Invalid URL format, skipping");
                    }
                }
                custom_urls
            },
            _ => Vec::new(),
        };

        Ok(urls)
    }

    async fn filter_high_quality_urls(&self, urls: &[String]) -> Result<Vec<String>> {
        // Filter for likely high-quality sites based on domain patterns
        let high_quality_patterns = [
            ".com", ".org", ".io", ".co", 
            // Exclude common low-value domains
        ];
        
        let low_quality_patterns = [
            "github.io", "herokuapp.com", "netlify.app", 
            "wordpress.com", "blogspot.com", "medium.com"
        ];

        let filtered: Vec<String> = urls.iter()
            .filter(|url| {
                let url_lower = url.to_lowercase();
                
                // Must have high-quality domain
                let has_quality_domain = high_quality_patterns.iter()
                    .any(|&pattern| url_lower.contains(pattern));
                
                // Must not have low-quality patterns
                let has_low_quality = low_quality_patterns.iter()
                    .any(|&pattern| url_lower.contains(pattern));
                
                has_quality_domain && !has_low_quality
            })
            .cloned()
            .collect();

        println!("📊 Filtered to {} high-quality URLs", filtered.len());
        Ok(filtered)
    }

    async fn execute_crawl(&self, urls: &[String], config: CrawlConfig) -> Result<()> {
        println!("\n🚀 Starting crawl execution...");


       // NEW: Check for recently crawled URLs
        let cache_days = 7; // Don't re-crawl for 7 days
        let (mut uncrawled_urls, cached_urls) = self.filter_uncrawled_urls(urls, cache_days).await?;
        
        if !cached_urls.is_empty() {
            println!("📋 Found {} recently crawled URLs (within {} days):", cached_urls.len(), cache_days);
            for url in cached_urls.iter().take(5) {
                println!("  ✅ {}", url);
            }
            if cached_urls.len() > 5 {
                println!("  ... and {} more", cached_urls.len() - 5);
            }
            
            // Ask if user wants to re-crawl anyway
            if !uncrawled_urls.is_empty() {
                let re_crawl = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt(&format!("Re-crawl {} cached URLs anyway?", cached_urls.len()))
                    .default(false)
                    .interact()?;
                
                if re_crawl {
                    uncrawled_urls.extend(cached_urls.clone());
                    println!("🔄 Will re-crawl all URLs");
                }
            }
        }
        
        if uncrawled_urls.is_empty() {
            println!("✅ All URLs have been recently crawled! Use cached results or force re-crawl.");
            
            // Show cached results
            self.show_cached_crawl_results(&cached_urls).await?;
            return Ok(());
        }
        
        println!("🎯 Crawling {} new/stale URLs", uncrawled_urls.len());
        
        let crawler = WebCrawler::new();
        let start_time = std::time::Instant::now();
        
        // Progress tracking
        let total_contacts = Arc::new(Mutex::new(0usize));
        let successful_crawls = Arc::new(Mutex::new(0usize));
        
        let progress_callback = {
            let _total_contacts = total_contacts.clone();
            let _successful_crawls = successful_crawls.clone();
            
            Box::new(move |current: usize, total: usize, url: &str| {
                println!("[{}/{}] 🕷️  Crawling: {}", current, total, url);
            })
        };

        // Execute crawl
        let results = crawler.crawl_multiple_urls(urls, config, Some(progress_callback)).await;
        
        // Process results
        let mut all_contacts = Vec::new();
        let mut successful_sites = 0;
        
        for result in &results {
            if result.success {
                successful_sites += 1;
                all_contacts.extend(result.best_contacts.clone());
            }
        }

        // Update counters
        *total_contacts.lock().await = all_contacts.len();
        *successful_crawls.lock().await = successful_sites;

        let duration = start_time.elapsed();

        // Show results summary
        self.display_crawl_results(&results, duration).await?;
        
        // Save results to database
        self.save_crawl_results(&results).await?;
        
        // Export results
        self.export_crawl_results(&results).await?;

        Ok(())
    }

    async fn show_cached_crawl_results(&self, urls: &[String]) -> Result<()> {
        if urls.is_empty() {
            return Ok(());
        }
        
        let conn = self.db_pool.get().await?;
        
        println!("\n📊 Recent Crawl Results (Cached):");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        for url in urls.iter().take(10) {
            let result: Option<(i64, i64, String)> = conn.query_row(
                "SELECT pages_crawled, contacts_found, crawled_at FROM crawl_results WHERE original_url = ? ORDER BY crawled_at DESC LIMIT 1",
                [url],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            ).ok();
            
            if let Some((pages, contacts, crawled_at)) = result {
                let date = chrono::DateTime::parse_from_rfc3339(&crawled_at)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|_| "unknown".to_string());
                
                println!("  ✅ {} - {} contacts ({} pages) - {}", url, contacts, pages, date);
            }
        }
        
        if urls.len() > 10 {
            println!("  ... and {} more cached results", urls.len() - 10);
        }
        
        Ok(())
    }

    async fn display_crawl_results(
        &self, 
        results: &[crate::web_crawler::CrawlResult], 
        duration: std::time::Duration
    ) -> Result<()> {
        println!("\n🎉 Crawl Results Summary");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let successful = results.iter().filter(|r| r.success).count();
        let total_contacts: usize = results.iter().map(|r| r.contacts_found).sum();
        let total_pages: usize = results.iter().map(|r| r.pages_crawled).sum();

        println!("📊 Sites crawled: {}/{}", successful, results.len());
        println!("📄 Total pages: {}", total_pages);
        println!("📧 Total contacts found: {}", total_contacts);
        println!("⏱️  Total time: {:.2}s", duration.as_secs_f64());

        if successful > 0 {
            let avg_contacts = total_contacts as f64 / successful as f64;
            println!("📈 Average contacts per site: {:.1}", avg_contacts);
        }

        // Show top sites by contacts
        let mut top_sites: Vec<_> = results.iter()
            .filter(|r| r.success && r.contacts_found > 0)
            .collect();
        top_sites.sort_by(|a, b| b.contacts_found.cmp(&a.contacts_found));

        if !top_sites.is_empty() {
            println!("\n🏆 Top Sites by Contacts Found:");
            for (i, result) in top_sites.iter().take(10).enumerate() {
                println!("  {}. {} - {} contacts ({} pages)", 
                         i + 1, result.original_url, result.contacts_found, result.pages_crawled);
            }
        }

        // Show contact type breakdown
        let mut email_count = 0;
        let mut phone_count = 0;
        let mut linkedin_count = 0;
        let mut form_count = 0;

        for result in results {
            for contact in &result.best_contacts {
                match contact.contact_type {
                    crate::web_crawler::types::ContactType::Email => email_count += 1,
                    crate::web_crawler::types::ContactType::Phone => phone_count += 1,
                    crate::web_crawler::types::ContactType::LinkedIn => linkedin_count += 1,
                    crate::web_crawler::types::ContactType::ContactForm => form_count += 1,
                    _ => {}
                }
            }
        }

        if total_contacts > 0 {
            println!("\n📋 Contact Type Breakdown:");
            println!("  📧 Emails: {}", email_count);
            println!("  📞 Phone numbers: {}", phone_count);
            println!("  💼 LinkedIn profiles: {}", linkedin_count);
            println!("  📝 Contact forms: {}", form_count);
        }

        // Show failed sites
        let failed: Vec<_> = results.iter().filter(|r| !r.success).collect();
        if !failed.is_empty() {
            println!("\n❌ Failed Sites ({}):", failed.len());
            for result in failed.iter().take(5) {
                let error = result.error_message.as_deref().unwrap_or("Unknown error");
                println!("  • {}: {}", result.original_url, error);
            }
            if failed.len() > 5 {
                println!("  ... and {} more", failed.len() - 5);
            }
        }

        Ok(())
    }

        async fn save_crawl_results(&self, results: &[crate::web_crawler::CrawlResult]) -> Result<()> {
        println!("\n💾 Saving crawl results to database...");
 
        let conn = self.db_pool.get().await?;
        let mut saved_count = 0;

        for result in results {
            let best_contacts_json = serde_json::to_string(&result.best_contacts)?;
 
            // Use REPLACE to update existing entries
            conn.execute(
                r#"
                INSERT OR REPLACE INTO crawl_results (
                    original_url, pages_crawled, contacts_found, best_contacts,
                    crawl_duration_ms, success, error_message, crawled_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                "#,
                rusqlite::params![
                    result.original_url,
                    result.pages_crawled as i64,
                    result.contacts_found as i64,
                    best_contacts_json,
                    result.crawl_duration_ms as i64,
                    result.success,
                    result.error_message,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )?;
            saved_count += 1;
        }

        println!("✅ Saved {} crawl results to database", saved_count);
        Ok(())
    }

    async fn export_crawl_results(&self, results: &[crate::web_crawler::CrawlResult]) -> Result<()> {
        println!("\n📤 Exporting results...");

        // Create output directory
        tokio::fs::create_dir_all("out/crawl_results").await?;
        
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        
        // Export full results as JSON
        let json_filename = format!("out/crawl_results/crawl_results_{}.json", timestamp);
        let json_data = serde_json::to_string_pretty(results)?;
        tokio::fs::write(&json_filename, json_data).await?;
        
        // Export contacts as CSV
        let csv_filename = format!("out/crawl_results/contacts_{}.csv", timestamp);
        let mut csv_content = String::from("source_url,contact_type,value,context,confidence\n");
        
        for result in results {
            for contact in &result.best_contacts {
                let contact_type = match contact.contact_type {
                    crate::web_crawler::types::ContactType::Email => "email",
                    crate::web_crawler::types::ContactType::Phone => "phone", 
                    crate::web_crawler::types::ContactType::LinkedIn => "linkedin",
                    crate::web_crawler::types::ContactType::Twitter => "twitter",
                    crate::web_crawler::types::ContactType::ContactForm => "contact_form",
                    crate::web_crawler::types::ContactType::Address => "address",
                };
                
                csv_content.push_str(&format!(
                    "\"{}\",\"{}\",\"{}\",\"{}\",{}\n",
                    result.original_url,
                    contact_type,
                    contact.value.replace("\"", "\"\""),
                    contact.context.replace("\"", "\"\""),
                    contact.confidence
                ));
            }
        }
        
        tokio::fs::write(&csv_filename, csv_content).await?;
        
        println!("✅ Results exported:");
        println!("  📄 Full data: {}", json_filename);
        println!("  📧 Contacts: {}", csv_filename);

        Ok(())
    }
}
