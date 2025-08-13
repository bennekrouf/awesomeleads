// src/cli/run_business_crawler.rs
use crate::database::{BusinessContact, Company};
use crate::models::CliApp;
use crate::web_crawler::{business_extractor::BusinessContactExtractor, CrawlConfig, WebCrawler};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use tracing::warn;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

impl CliApp {
    pub async fn run_business_crawler(&self) -> Result<()> {
        println!("\n🏢 Business Contact Discovery System");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🎯 Focus: Finding companies and decision-makers for investment opportunities");

        // Show current business data
        self.show_business_statistics().await?;

        let action_options = vec![
            "🕷️  Crawl for New Business Contacts",
            "📊 Show Business Contact Statistics", 
            "📧 Export Business Contacts for Outreach",
            "🏢 View Top Companies by Investment Potential",
            "🔍 Search Specific Company",
            "📈 Generate Investment Pipeline Report",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select business action")
            .items(&action_options)
            .interact()?;

        match selection {
            0 => self.run_business_contact_crawl().await?,
            1 => self.show_detailed_business_statistics().await?,
            2 => self.export_business_contacts().await?,
            3 => self.show_top_investment_targets().await?,
            4 => self.search_specific_company().await?,
            5 => self.generate_investment_pipeline_report().await?,
            _ => {}
        }

        Ok(())
    }

    async fn show_business_statistics(&self) -> Result<()> {
        let conn = self.db_pool.get().await?;

        let total_companies: i64 = conn.query_row(
            "SELECT COUNT(*) FROM companies", 
            [], 
            |row| row.get(0)
        ).unwrap_or(0);

        let business_contacts: i64 = conn.query_row(
            "SELECT COUNT(*) FROM business_contacts", 
            [], 
            |row| row.get(0)
        ).unwrap_or(0);

        let decision_makers: i64 = conn.query_row(
            "SELECT COUNT(*) FROM business_contacts WHERE is_decision_maker = 1", 
            [], 
            |row| row.get(0)
        ).unwrap_or(0);

        println!("\n📊 Current Business Database:");
        println!("  🏢 Companies discovered: {}", total_companies);
        println!("  👥 Business contacts: {}", business_contacts);
        println!("  🎯 Decision makers: {}", decision_makers);

        if total_companies == 0 {
            println!("\n💡 No business data yet. Run the business crawler to discover companies!");
        }

        Ok(())
    }

    async fn run_business_contact_crawl(&self) -> Result<()> {
        println!("\n🎯 Business-Focused Web Crawling");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Get business-focused URLs
        let business_urls = self.get_business_focused_urls().await?;
        
        if business_urls.is_empty() {
            println!("❌ No business URLs found.");
            println!("💡 Make sure you've run Phase 1 to collect non-GitHub project URLs.");
            return Ok(());
        }

        println!("🏢 Found {} potential business websites", business_urls.len());

        // Show sample URLs
        println!("\n📋 Sample Business URLs:");
        for (i, url) in business_urls.iter().take(5).enumerate() {
            println!("  {}. {}", i + 1, url);
        }
        if business_urls.len() > 5 {
            println!("  ... and {} more", business_urls.len() - 5);
        }

        // Configure business crawl
        let config = self.configure_business_crawl().await?;
        let urls_to_crawl = self.select_business_urls(&business_urls).await?;

        if urls_to_crawl.is_empty() {
            println!("❌ No URLs selected for crawling");
            return Ok(());
        }

        // Confirm crawl
        println!("\n🎯 Ready to crawl {} business websites", urls_to_crawl.len());
        println!("📊 Focus: Companies, decision-makers, investment signals");
        
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Start business crawling?")
            .interact()?
        {
            return Ok(());
        }

        // Execute business crawl
        self.execute_business_crawl(&urls_to_crawl, config).await?;

        Ok(())
    }

    async fn get_business_focused_urls(&self) -> Result<Vec<String>> {
        let conn = self.db_pool.get().await?;
        
        let mut stmt = conn.prepare(
            r#"
            SELECT url FROM non_github_projects 
            WHERE project_type IN ('website', 'tool', 'api', 'other')
            AND url NOT LIKE '%github.%'
            AND url NOT LIKE '%docs.%'
            AND url NOT LIKE '%blog.%'
            AND url NOT LIKE '%medium.com%'
            AND url NOT LIKE '%dev.to%'
            AND (
                LOWER(description) LIKE '%startup%' OR
                LOWER(description) LIKE '%company%' OR
                LOWER(description) LIKE '%platform%' OR
                LOWER(description) LIKE '%service%' OR
                LOWER(description) LIKE '%solution%' OR
                LOWER(description) LIKE '%business%' OR
                LOWER(url) LIKE '%.com%' OR
                LOWER(url) LIKE '%.io%'
            )
            ORDER BY last_updated DESC
            "#
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

    async fn configure_business_crawl(&self) -> Result<CrawlConfig> {
        println!("\n⚙️  Business Crawl Configuration");
        
        let preset_options = vec![
            "🎯 Executive Hunt (contact/about/team pages only)",
            "🏢 Company Deep Dive (5 pages, full company analysis)",
            "🚀 Startup Scout (3 pages, growth signals focus)",
            "⚙️ Custom Business Configuration",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select business crawl type")
            .items(&preset_options)
            .interact()?;

        let config = match selection {
            0 => CrawlConfig {
                max_pages: 3,
                delay_ms: 1500,
                timeout_seconds: 45,
                respect_robots: true,
                contact_pages_only: true,
                follow_external_links: false,
            },
            1 => CrawlConfig {
                max_pages: 5,
                delay_ms: 2000,
                timeout_seconds: 90,
                respect_robots: true,
                contact_pages_only: false,
                follow_external_links: false,
            },
            2 => CrawlConfig {
                max_pages: 3,
                delay_ms: 1500,
                timeout_seconds: 60,
                respect_robots: true,
                contact_pages_only: false,
                follow_external_links: false,
            },
            3 => {
                // Custom configuration
                let max_pages: u32 = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Maximum pages per company")
                    .default(3)
                    .interact_text()?;

                let contact_pages_only = Confirm::with_theme(&ColorfulTheme::default())
                    .with_prompt("Focus only on contact/team/about pages?")
                    .default(true)
                    .interact()?;

                CrawlConfig {
                    max_pages,
                    delay_ms: 2000,
                    timeout_seconds: 60,
                    respect_robots: true,
                    contact_pages_only,
                    follow_external_links: false,
                }
            }
            _ => CrawlConfig::default(),
        };

        println!("✅ Business crawl configured: {} pages per company", config.max_pages);
        Ok(config)
    }

    async fn select_business_urls(&self, available_urls: &[String]) -> Result<Vec<String>> {
        let selection_options = vec![
            "🎯 High-Value Prospects (Top 25 business sites)",
            "🚀 All Startup/Company URLs",
            "🧪 Test Sample (5 companies)",
            "📝 Enter Specific Company URLs",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select companies to analyze")
            .items(&selection_options)
            .interact()?;

        let urls = match selection {
            0 => {
                // Filter for high-value business prospects
                self.filter_high_value_prospects(available_urls).await?
                    .into_iter().take(25).collect()
            },
            1 => {
                if available_urls.len() > 50 {
                    println!("⚠️  This will crawl {} companies", available_urls.len());
                    if !Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt("Continue with all companies?")
                        .default(false)
                        .interact()?
                    {
                        return Ok(Vec::new());
                    }
                }
                available_urls.to_vec()
            },
            2 => available_urls.iter().take(5).cloned().collect(),
            3 => {
                let mut custom_urls = Vec::new();
                println!("💡 Enter company website URLs:");
                loop {
                    let url: String = Input::with_theme(&ColorfulTheme::default())
                        .with_prompt("Company URL (empty to finish)")
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

    async fn filter_high_value_prospects(&self, urls: &[String]) -> Result<Vec<String>> {
        // Filter for URLs that look like serious business prospects
        let high_value_indicators = [
            ".com", ".io", ".ai", ".co"
        ];
        
        let low_value_patterns = [
            "github.io", "herokuapp.com", "netlify.app", "vercel.app",
            "wordpress.com", "wix.com", "squarespace.com", "medium.com"
        ];

        let filtered: Vec<String> = urls.iter()
            .filter(|url| {
                let url_lower = url.to_lowercase();
                
                // Must have business-like domain
                let has_business_domain = high_value_indicators.iter()
                    .any(|&indicator| url_lower.contains(indicator));
                
                // Must not be low-value hosting
                let is_low_value = low_value_patterns.iter()
                    .any(|&pattern| url_lower.contains(pattern));
                
                has_business_domain && !is_low_value
            })
            .cloned()
            .collect();

        println!("📊 Filtered to {} high-value business prospects", filtered.len());
        Ok(filtered)
    }

    async fn execute_business_crawl(&self, urls: &[String], config: CrawlConfig) -> Result<()> {
        println!("\n🚀 Starting business contact discovery...");
        
        let crawler = WebCrawler::new();
        let business_extractor = BusinessContactExtractor::new();
        
        let mut companies_discovered = 0;
        let mut contacts_discovered = 0;
        let mut decision_makers_found = 0;
        
        for (i, url) in urls.iter().enumerate() {
            println!("[{}/{}] 🏢 Analyzing: {}", i + 1, urls.len(), url);
            
            match crawler.crawl_for_contacts(url, config.clone()).await {
                Ok(crawl_result) => {
                    if crawl_result.success && !crawl_result.pages.is_empty() {
                        // Extract company information
                        let first_page = &crawl_result.pages[0];
                        if let Some(company) = business_extractor
                            .extract_company_info(&first_page.title, url).await {
                            
                            // Save company to database
                            let company_id = self.save_company(&company).await?;
                            companies_discovered += 1;
                            
                            // Extract business contacts from all pages
                            let mut all_business_contacts = Vec::new();
                            for page in &crawl_result.pages {
                                let page_contacts = business_extractor.extract_business_contacts(
                                    "", // HTML would be here in real implementation
                                    &page.clean_text,
                                    &page.url,
                                    company_id,
                                );
                                all_business_contacts.extend(page_contacts);
                            }
                            
                            // Save business contacts
                            for contact in &all_business_contacts {
                                if let Err(e) = self.save_business_contact(contact).await {
                                    warn!("Failed to save contact: {}", e);
                                } else {
                                    contacts_discovered += 1;
                                    if contact.is_decision_maker {
                                        decision_makers_found += 1;
                                    }
                                }
                            }
                            
                            println!("  ✅ Company: {} - {} contacts ({} decision makers)", 
                                   company.name, all_business_contacts.len(),
                                   all_business_contacts.iter().filter(|c| c.is_decision_maker).count());
                        }
                    } else {
                        println!("  ⚠️  No useful data found");
                    }
                }
                Err(e) => {
                    println!("  ❌ Failed: {}", e);
                }
            }
            
            // Rate limiting
            if i < urls.len() - 1 {
                tokio::time::sleep(tokio::time::Duration::from_millis(config.delay_ms)).await;
            }
        }
        
        // Show results
        println!("\n🎉 Business Discovery Complete!");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("🏢 Companies discovered: {}", companies_discovered);
        println!("👥 Business contacts found: {}", contacts_discovered);
        println!("🎯 Decision makers identified: {}", decision_makers_found);
        
        if decision_makers_found > 0 {
            println!("\n💡 Next steps:");
            println!("  📧 Export business contacts for outreach");
            println!("  📊 Analyze investment pipeline");
            println!("  🎯 Prioritize by decision maker seniority");
        }

        Ok(())
    }

    async fn save_company(&self, company: &Company) -> Result<i64> {
        let conn = self.db_pool.get().await?;
        
        conn.execute(
            r#"
            INSERT INTO companies (
                name, domain, website_url, company_type, industry, description,
                discovered_from, confidence_score, created_at, last_updated
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(domain) DO UPDATE SET
                name = excluded.name,
                company_type = COALESCE(excluded.company_type, company_type),
                industry = COALESCE(excluded.industry, industry), 
                description = COALESCE(excluded.description, description),
                last_updated = excluded.last_updated
            "#,
            rusqlite::params![
                company.name,
                company.domain,
                company.website_url,
                company.company_type,
                company.industry,
                company.description,
                company.discovered_from,
                company.confidence_score,
                company.created_at.to_rfc3339(),
                company.last_updated.to_rfc3339(),
            ],
        )?;

        // Get the company ID
        let company_id: i64 = conn.query_row(
            "SELECT id FROM companies WHERE domain = ?",
            [&company.domain],
            |row| row.get(0),
        )?;

        Ok(company_id)
    }

    async fn save_business_contact(&self, contact: &BusinessContact) -> Result<()> {
        let conn = self.db_pool.get().await?;
        
        conn.execute(
            r#"
            INSERT INTO business_contacts (
                company_id, email, first_name, last_name, full_name, job_title,
                role_category, contact_type, contact_value, context, page_url,
                confidence, is_decision_maker, seniority_level, department,
                discovered_at, email_status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ON CONFLICT(email, company_id) DO UPDATE SET
                job_title = COALESCE(excluded.job_title, job_title),
                role_category = COALESCE(excluded.role_category, role_category),
                confidence = MAX(confidence, excluded.confidence),
                is_decision_maker = excluded.is_decision_maker OR is_decision_maker
            "#,
            rusqlite::params![
                contact.company_id,
                contact.email,
                contact.first_name,
                contact.last_name,
                contact.full_name,
                contact.job_title,
                contact.role_category,
                contact.contact_type,
                contact.contact_value,
                contact.context,
                contact.page_url,
                contact.confidence,
                contact.is_decision_maker,
                contact.seniority_level,
                contact.department,
                contact.discovered_at.to_rfc3339(),
                contact.email_status,
            ],
        )?;

        Ok(())
    }

    async fn export_business_contacts(&self) -> Result<()> {
        println!("\n📧 Business Contact Export");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let export_options = vec![
            "🎯 Decision Makers Only (CEOs, CTOs, Founders)",
            "👥 All Business Contacts",
            "🚀 Startups & Scale-ups Only", 
            "💰 Investment-Ready Companies",
            "🏢 By Industry (select specific)",
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select contacts to export")
            .items(&export_options)
            .interact()?;

        let (contacts, filename_suffix) = match selection {
            0 => (self.get_decision_makers().await?, &"decision_makers".to_string()),
            1 => (self.get_all_business_contacts().await?, &"all_business".to_string()),
            2 => (self.get_startup_contacts().await?, &"startups".to_string()),
            3 => (self.get_investment_ready_contacts().await?, &"investment_ready".to_string()),
            4 => {
                let industry = self.select_industry().await?;
                (self.get_contacts_by_industry(&industry).await?, &format!("industry_{}", industry.to_string()))
            },
            _ => return Ok(()),
        };

        if contacts.is_empty() {
            println!("❌ No contacts found for selected criteria");
            return Ok(());
        }

        // Export to CSV
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("out/business_contacts_{}_{}.csv", filename_suffix, timestamp);
        
        tokio::fs::create_dir_all("out").await?;
        
        let mut csv_content = String::from(
            "company_name,domain,industry,email,full_name,job_title,role_category,seniority_level,department,confidence,is_decision_maker,company_type\n"
        );
        
        for contact in &contacts {
            csv_content.push_str(&format!(
                "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},{},\"{}\"\n",
                contact.company_name.replace("\"", "\"\""),
                contact.domain,
                contact.industry.as_deref().unwrap_or(""),
                contact.email,
                contact.full_name.as_deref().unwrap_or(""),
                contact.job_title.as_deref().unwrap_or(""),
                contact.role_category.as_deref().unwrap_or(""),
                contact.seniority_level.as_deref().unwrap_or(""),
                contact.department.as_deref().unwrap_or(""),
                contact.confidence,
                contact.is_decision_maker,
                contact.company_type.as_deref().unwrap_or(""),
            ));
        }
        
        tokio::fs::write(&filename, csv_content).await?;
        
        println!("✅ Exported {} business contacts to: {}", contacts.len(), filename);
        println!("📊 Decision makers: {}", contacts.iter().filter(|c| c.is_decision_maker).count());
        
        Ok(())
    }

    // Additional helper methods for business contact management...
    async fn get_decision_makers(&self) -> Result<Vec<BusinessContactExport>> {
        let conn = self.db_pool.get().await?;
        
        let mut stmt = conn.prepare(
            r#"
            SELECT c.name, c.domain, c.industry, c.company_type,
                   bc.email, bc.full_name, bc.job_title, bc.role_category,
                   bc.seniority_level, bc.department, bc.confidence, bc.is_decision_maker
            FROM business_contacts bc
            JOIN companies c ON bc.company_id = c.id
            WHERE bc.is_decision_maker = 1
            ORDER BY bc.confidence DESC, c.confidence_score DESC
            "#
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(BusinessContactExport {
                company_name: row.get(0)?,
                domain: row.get(1)?,
                industry: row.get(2)?,
                company_type: row.get(3)?,
                email: row.get(4)?,
                full_name: row.get(5)?,
                job_title: row.get(6)?,
                role_category: row.get(7)?,
                seniority_level: row.get(8)?,
                department: row.get(9)?,
                confidence: row.get(10)?,
                is_decision_maker: row.get(11)?,
            })
        })?;

        let mut contacts = Vec::new();
        for row in rows {
            contacts.push(row?);
        }

        Ok(contacts)
    }

    // Continue with other helper methods...
    async fn show_detailed_business_statistics(&self) -> Result<()> {
        println!("\n📊 Detailed Business Statistics");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Implementation for detailed statistics...
        Ok(())
    }

    async fn show_top_investment_targets(&self) -> Result<()> {
        println!("\n🎯 Top Investment Targets");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        // Implementation for investment targets...
        Ok(())
    }

    async fn search_specific_company(&self) -> Result<()> {
        let company_name: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter company name or domain to search")
            .interact_text()?;

        // Implementation for company search...
        println!("🔍 Searching for: {}", company_name);
        Ok(())
    }

    async fn generate_investment_pipeline_report(&self) -> Result<()> {
        println!("\n📈 Generating Investment Pipeline Report...");
        
        // Implementation for pipeline report...
        Ok(())
    }

    // Placeholder implementations for other methods...
    async fn get_all_business_contacts(&self) -> Result<Vec<BusinessContactExport>> { Ok(Vec::new()) }
    async fn get_startup_contacts(&self) -> Result<Vec<BusinessContactExport>> { Ok(Vec::new()) }
    async fn get_investment_ready_contacts(&self) -> Result<Vec<BusinessContactExport>> { Ok(Vec::new()) }
    async fn select_industry(&self) -> Result<String> { Ok("technology".to_string()) }
    async fn get_contacts_by_industry(&self, _industry: &str) -> Result<Vec<BusinessContactExport>> { Ok(Vec::new()) }
}

#[derive(Debug)]
struct BusinessContactExport {
    company_name: String,
    domain: String,
    industry: Option<String>,
    company_type: Option<String>,
    email: String,
    full_name: Option<String>,
    job_title: Option<String>,
    role_category: Option<String>,
    seniority_level: Option<String>,
    department: Option<String>,
    confidence: f64,
    is_decision_maker: bool,
}
