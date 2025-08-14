// src/web_crawler/crawler.rs - Updated to use reqwest instead of spider
use crate::web_crawler::contact_extractor::ContactExtractor;
use crate::web_crawler::types::{ContactInfo, CrawlConfig, CrawlResult, CrawledPage, PageMetadata};
use reqwest::Client;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};
use url::Url;
use uuid::Uuid;

pub struct WebCrawler {
    client: Client,
    contact_extractor: ContactExtractor,
}

impl WebCrawler {
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (compatible; ContactCrawler/1.0)")
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            contact_extractor: ContactExtractor::new(),
        }
    }

    pub async fn crawl_for_contacts(
        &self,
        url: &str,
        config: CrawlConfig,
    ) -> Result<CrawlResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        info!("🕷️  Starting crawl of {} with config: {:?}", url, config);

        let base_url = self.parse_base_url(url)?;
        let mut visited_urls = HashSet::new();
        let mut crawled_pages = Vec::new();
        let mut all_contacts = Vec::new();

        // Start with the main URL
        let urls_to_crawl = self.discover_urls(&base_url, &config).await?;
        
        for (i, page_url) in urls_to_crawl.iter().take(config.max_pages as usize).enumerate() {
            if visited_urls.contains(page_url) {
                continue;
            }
            
            debug!("Crawling page {}/{}: {}", i + 1, config.max_pages, page_url);
            visited_urls.insert(page_url.clone());

            match self.crawl_single_page(page_url, &config).await {
                Ok(page) => {
                    all_contacts.extend(page.contacts.clone());
                    crawled_pages.push(page);
                }
                Err(e) => {
                    warn!("Failed to crawl {}: {}", page_url, e);
                }
            }

            // Rate limiting
            if i < urls_to_crawl.len() - 1 {
                tokio::time::sleep(Duration::from_millis(config.delay_ms)).await;
            }
        }

        // Filter and rank best contacts
        let best_contacts = self.select_best_contacts(&all_contacts);
        let duration = start_time.elapsed();

        let result = CrawlResult {
            original_url: url.to_string(),
            pages_crawled: crawled_pages.len(),
            contacts_found: all_contacts.len(),
            pages: crawled_pages,
            best_contacts,
            crawl_duration_ms: duration.as_millis() as u64,
            success: true,
            error_message: None,
        };

        info!(
            "🎯 Crawl complete for {}: {} pages, {} contacts in {}ms",
            url, result.pages_crawled, result.contacts_found, result.crawl_duration_ms
        );

        Ok(result)
    }

    async fn discover_urls(
        &self,
        base_url: &str,
        config: &CrawlConfig,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut urls = vec![base_url.to_string()];
        
        // Try to get the main page first to discover other pages
        match self.fetch_page_content(base_url).await {
            Ok(html) => {
                let additional_urls = self.extract_contact_related_urls(&html, base_url, config);
                urls.extend(additional_urls);
            }
            Err(e) => {
                warn!("Failed to fetch main page {}: {}", base_url, e);
            }
        }

        // If contact_pages_only, prioritize contact/about/team pages
        if config.contact_pages_only {
            urls = self.prioritize_contact_pages(urls);
        }

        Ok(urls)
    }

    async fn crawl_single_page(
        &self,
        url: &str,
        config: &CrawlConfig,
    ) -> Result<CrawledPage, Box<dyn std::error::Error + Send + Sync>> {
        let html = self.fetch_page_content(url).await?;
        Ok(self.extract_page_content(&html, url, config))
    }

    async fn fetch_page_content(
        &self,
        url: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        debug!("Fetching: {}", url);
        
        let response = self.client.get(url).send().await?;
        
        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()).into());
        }

        let html = response.text().await?;
        debug!("Fetched {} bytes from {}", html.len(), url);
        
        Ok(html)
    }

    fn extract_contact_related_urls(
        &self,
        html: &str,
        base_url: &str,
        _config: &CrawlConfig,
    ) -> Vec<String> {
        let document = Html::parse_document(html);
        let link_selector = Selector::parse("a[href]").unwrap();
        let mut urls = Vec::new();

        for element in document.select(&link_selector) {
            if let Some(href) = element.value().attr("href") {
                if let Some(full_url) = self.resolve_url(href, base_url) {
                    // Look for contact-related pages
                    let href_lower = href.to_lowercase();
                    if self.is_contact_related_url(&href_lower) {
                        urls.push(full_url);
                    }
                }
            }
        }

        // Remove duplicates and limit
        urls.sort();
        urls.dedup();
        urls.truncate(10);
        
        urls
    }

    fn is_contact_related_url(&self, url_path: &str) -> bool {
        let contact_indicators = [
            "contact", "about", "team", "people", "leadership",
            "staff", "founders", "management", "executives"
        ];
        
        contact_indicators.iter().any(|&indicator| url_path.contains(indicator))
    }

    fn prioritize_contact_pages(&self, mut urls: Vec<String>) -> Vec<String> {
        // Sort URLs with contact pages first
        urls.sort_by(|a, b| {
            let a_is_contact = self.is_contact_related_url(&a.to_lowercase());
            let b_is_contact = self.is_contact_related_url(&b.to_lowercase());
            
            match (a_is_contact, b_is_contact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });
        
        urls
    }

    fn resolve_url(&self, href: &str, base_url: &str) -> Option<String> {
        match Url::parse(href) {
            Ok(url) => Some(url.to_string()),
            Err(_) => {
                // Try to resolve as relative URL
                if let Ok(base) = Url::parse(base_url) {
                    base.join(href).ok().map(|u| u.to_string())
                } else {
                    None
                }
            }
        }
    }

    fn parse_base_url(&self, url: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let parsed = Url::parse(url)?;
        let base = format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""));
        Ok(base)
    }

    fn extract_page_content(&self, html: &str, url: &str, config: &CrawlConfig) -> CrawledPage {
        let document = Html::parse_document(html);

        // Extract title
        let title_selector = Selector::parse("title").unwrap();
        let title = document
            .select(&title_selector)
            .next()
            .map(|t| t.text().collect::<String>())
            .unwrap_or_default();

        // Extract clean text
        let clean_text = self.extract_clean_text(&document);
        
        // Extract domain
        let domain = Url::parse(url)
            .map(|u| u.host_str().unwrap_or("unknown").to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // Check page characteristics
        let is_contact_page = self.contact_extractor.is_contact_page(html, url);
        let is_about_page = url.to_lowercase().contains("about") || 
                           title.to_lowercase().contains("about");
        let has_contact_keywords = self.contact_extractor.has_contact_keywords(&clean_text);

        // Count links
        let link_selector = Selector::parse("a[href]").unwrap();
        let links_count = document.select(&link_selector).count();

        // Determine page type
        let page_type = self.determine_page_type(url, &title, &clean_text);

        // Skip non-contact pages if configured
        let contacts = if config.contact_pages_only && !is_contact_page && !is_about_page {
            Vec::new()
        } else {
            self.contact_extractor.extract_contacts(html, &clean_text, url)
        };

        CrawledPage {
            id: Uuid::new_v4().to_string(),
            url: url.to_string(),
            title,
            clean_text: clean_text.chars().take(5000).collect(), // Limit stored text
            metadata: PageMetadata {
                word_count: clean_text.split_whitespace().count(),
                has_contact_keywords,
                page_type,
                links_count,
                domain,
                is_contact_page,
                is_about_page,
            },
            contacts,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn extract_clean_text(&self, document: &Html) -> String {
        // Remove scripts, styles, navigation, etc.
        let body_selector = Selector::parse("body").unwrap();
        
        document
            .select(&body_selector)
            .next()
            .map(|body| {
                body.text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default()
    }

    fn determine_page_type(&self, url: &str, title: &str, _text: &str) -> String {
        let url_lower = url.to_lowercase();
        let title_lower = title.to_lowercase();

        if url_lower.contains("/contact") || title_lower.contains("contact") {
            "contact"
        } else if url_lower.contains("/about") || title_lower.contains("about") {
            "about"
        } else if url_lower.contains("/team") || title_lower.contains("team") {
            "team"
        } else if url_lower.contains("/blog") || url_lower.contains("/news") {
            "blog"
        } else if url_lower.contains("/product") || url_lower.contains("/service") {
            "product"
        } else {
            "general"
        }.to_string()
    }

    fn select_best_contacts(&self, all_contacts: &[ContactInfo]) -> Vec<ContactInfo> {
        use std::collections::HashMap;

        let mut best_contacts = HashMap::new();
        
        // Group by contact type and value, keeping highest confidence
        for contact in all_contacts {
            let key = (contact.contact_type.clone(), contact.value.clone());
            
            best_contacts
                .entry(key)
                .and_modify(|existing: &mut ContactInfo| {
                    if contact.confidence > existing.confidence {
                        *existing = contact.clone();
                    }
                })
                .or_insert(contact.clone());
        }

        // Sort by confidence and type priority
        let mut contacts: Vec<ContactInfo> = best_contacts.into_values().collect();
        contacts.sort_by(|a, b| {
            // First by type priority
            let type_priority = |ct: &crate::web_crawler::types::ContactType| match ct {
                crate::web_crawler::types::ContactType::Email => 0,
                crate::web_crawler::types::ContactType::Phone => 1,
                crate::web_crawler::types::ContactType::LinkedIn => 2,
                crate::web_crawler::types::ContactType::Twitter => 3,
                crate::web_crawler::types::ContactType::ContactForm => 4,
                crate::web_crawler::types::ContactType::Address => 5,
            };
            
            let a_priority = type_priority(&a.contact_type);
            let b_priority = type_priority(&b.contact_type);
            
            a_priority.cmp(&b_priority)
                .then_with(|| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal))
        });

        // Return top 10 contacts
        contacts.into_iter().take(10).collect()
    }

    pub async fn crawl_multiple_urls(
        &self,
        urls: &[String],
        config: CrawlConfig,
        progress_callback: Option<Box<dyn Fn(usize, usize, &str) + Send + Sync>>,
    ) -> Vec<CrawlResult> {
        let mut results = Vec::new();
        
        info!("🚀 Starting batch crawl of {} URLs", urls.len());
        
        for (i, url) in urls.iter().enumerate() {
            if let Some(ref callback) = progress_callback {
                callback(i + 1, urls.len(), url);
            }
            
            match self.crawl_for_contacts(url, config.clone()).await {
                Ok(result) => {
                    info!("✅ Successfully crawled {}: {} contacts", url, result.contacts_found);
                    results.push(result);
                }
                Err(e) => {
                    error!("❌ Failed to crawl {}: {}", url, e);
                    results.push(CrawlResult {
                        original_url: url.clone(),
                        pages_crawled: 0,
                        contacts_found: 0,
                        pages: Vec::new(),
                        best_contacts: Vec::new(),
                        crawl_duration_ms: 0,
                        success: false,
                        error_message: Some(e.to_string()),
                    });
                }
            }
            
            // Rate limiting between URLs
            if i < urls.len() - 1 {
                tokio::time::sleep(Duration::from_millis(config.delay_ms)).await;
            }
        }
        
        info!("🏁 Batch crawl complete: {}/{} successful", 
              results.iter().filter(|r| r.success).count(), 
              urls.len());
        
        results
    }
}
