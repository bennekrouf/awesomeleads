// src/web_crawler/business_extractor.rs
use crate::database::{BusinessContact, Company};
use crate::web_crawler::types::ContactInfo;
use regex::Regex;
use scraper::{Html, Selector};
// use std::collections::HashMap;
use tracing::info;
use url::Url;

pub struct BusinessContactExtractor {
    email_regex: Regex,
    phone_regex: Regex,
    linkedin_regex: Regex,
    name_title_regex: Regex,
}

impl BusinessContactExtractor {
    pub fn new() -> Self {
        Self {
            email_regex: Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap(),
            phone_regex: Regex::new(r"(?:\+?1[-.\s]?)?\(?([0-9]{3})\)?[-.\s]?([0-9]{3})[-.\s]?([0-9]{4})").unwrap(),
            linkedin_regex: Regex::new(r"(?:https?://)?(?:www\.)?linkedin\.com/(?:in|company)/([a-zA-Z0-9\-_]+)").unwrap(),
            name_title_regex: Regex::new(r"(?i)(CEO|CTO|CFO|COO|VP|Vice President|President|Founder|Co-founder|Director|Head of|Chief)").unwrap(),
        }
    }

    pub async fn extract_company_info(&self, html: &str, url: &str) -> Option<Company> {
        let document = Html::parse_document(html);
        let parsed_url = Url::parse(url).ok()?;
        let domain = parsed_url.host_str()?.to_string();

        // Extract company name from multiple sources
        let company_name = self.extract_company_name(&document, &domain);

        // Extract company description
        let description = self.extract_company_description(&document);

        // Determine company type and industry
        let (company_type, industry) = self.classify_company(&document, &description);

        // Extract other company signals
        let employee_count = self.estimate_employee_count();
        let funding_stage = self.detect_funding_stage(&document);
        let location = self.extract_location();
        let founded_year = self.extract_founded_year();

        Some(Company {
            id: None,
            name: company_name,
            domain: domain.clone(),
            website_url: url.to_string(),
            company_type: Some(company_type),
            industry: Some(industry),
            description,
            employee_count_estimate: employee_count,
            funding_stage,
            location,
            founded_year,
            discovered_from: "web_crawler".to_string(),
            confidence_score: 0.7, // Base confidence
            verified: false,
            created_at: chrono::Utc::now(),
            last_updated: chrono::Utc::now(),
        })
    }

    pub fn extract_business_contacts(
        &self,
        html: &str,
        clean_text: &str,
        url: &str,
        company_id: i64,
    ) -> Vec<BusinessContact> {
        let mut contacts = Vec::new();
        let _document = Html::parse_document(html);
        
        // Look for team/about/leadership pages specifically
        let is_team_page = self.is_team_or_leadership_page(url, html);
        
        // Extract emails with business context
        let emails = self.extract_business_emails(clean_text, html, url);
        
        for email_info in emails {
            // Try to extract name and title for this email
            let (name, title, role_category, seniority) = 
                self.extract_person_details(&email_info.value, html, clean_text);
            
            let is_decision_maker = self.is_decision_maker(&title, &role_category);
            let context = Some(email_info.context.clone());
            let contact_value = email_info.value.clone();
           
            let contact = BusinessContact {
                id: None,
                company_id,
                email: contact_value.clone(),
                first_name: name.as_ref().and_then(|n| n.split_whitespace().next().map(String::from)),
                last_name: name.as_ref().and_then(|n| n.split_whitespace().last().map(String::from)),
                full_name: name,
                job_title: title.clone(),
                role_category: Some(role_category),
                contact_type: "email".to_string(),
                contact_value,
                context,
                page_url: Some(url.to_string()),
                confidence: self.calculate_business_confidence(&email_info, is_team_page),
                is_decision_maker,
                linkedin_profile: None, // Will be populated separately
                twitter_profile: None,
                phone_number: None,
                seniority_level: Some(seniority),
                department: self.guess_department(&title.as_deref().unwrap_or("")),
                discovered_at: chrono::Utc::now(),
                last_contacted: None,
                email_status: "never_contacted".to_string(),
                notes: None,
            };
            
            contacts.push(contact);
        }
        
        info!("Extracted {} business contacts from {}", contacts.len(), url);
        contacts
    }

    fn extract_company_name(&self, document: &Html, domain: &str) -> String {
        // Try multiple selectors for company name
        let selectors = [
            "h1.company-name",
            ".company-name", 
            "h1",
            "title",
            ".site-title",
            ".brand",
            ".logo",
        ];
        
        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() && text.len() < 100 {
                        return self.clean_company_name(&text);
                    }
                }
            }
        }
        
        // Fallback: clean up domain name
        self.domain_to_company_name(domain)
    }

    fn extract_company_description(&self, document: &Html) -> Option<String> {
        let selectors = [
            "meta[name='description']",
            ".company-description",
            ".about",
            ".hero-text",
            ".tagline",
        ];
        
        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    let text = if selector_str.starts_with("meta") {
                        element.value().attr("content").unwrap_or("").to_string()
                    } else {
                        element.text().collect::<String>()
                    };
                    
                    let cleaned = text.trim().to_string();
                    if !cleaned.is_empty() && cleaned.len() > 20 && cleaned.len() < 500 {
                        return Some(cleaned);
                    }
                }
            }
        }
        
        None
    }

    fn classify_company(&self, document: &Html, description: &Option<String>) -> (String, String) {
        let text = document.root_element().text().collect::<String>().to_lowercase();
        let desc_text = description.as_deref().unwrap_or("").to_lowercase();
        let combined = format!("{} {}", text, desc_text);
        
        // Industry classification
        let industry = if combined.contains("blockchain") || combined.contains("crypto") || 
                         combined.contains("web3") || combined.contains("defi") {
            "web3"
        } else if combined.contains("ai") || combined.contains("machine learning") || 
                  combined.contains("artificial intelligence") {
            "ai"
        } else if combined.contains("fintech") || combined.contains("financial") {
            "fintech"
        } else if combined.contains("saas") || combined.contains("software as a service") {
            "saas"
        } else if combined.contains("ecommerce") || combined.contains("e-commerce") {
            "ecommerce"
        } else if combined.contains("healthtech") || combined.contains("health") {
            "healthtech"
        } else {
            "technology"
        }.to_string();
        
        // Company type classification
        let company_type = if combined.contains("startup") || combined.contains("founded") {
            "startup"
        } else if combined.contains("enterprise") || combined.contains("corporation") {
            "enterprise"
        } else if combined.contains("agency") || combined.contains("consultancy") {
            "agency"
        } else {
            "scale-up"
        }.to_string();
        
        (company_type, industry)
    }

    fn extract_business_emails(&self, text: &str, _html: &str, url: &str) -> Vec<ContactInfo> {
        let mut emails = Vec::new();
        let mut seen = std::collections::HashSet::new();
        
        for captures in self.email_regex.captures_iter(text) {
            if let Some(email_match) = captures.get(0) {
                let email = email_match.as_str().to_lowercase();
                
                // Filter for business emails only
                if self.is_business_email(&email) && seen.insert(email.clone()) {
                    let context = self.extract_context_around_email(text, email_match.start(), email_match.end());
                    
                    emails.push(ContactInfo {
                        contact_type: crate::web_crawler::types::ContactType::Email,
                        value: email,
                        context,
                        confidence: 0.7, // Will be recalculated
                        source_url: url.to_string(),
                    });
                }
            }
        }
        
        emails
    }

    fn is_business_email(&self, email: &str) -> bool {
        // Exclude developer/support emails, focus on business contacts
        let exclude_patterns = [
            "noreply", "no-reply", "support", "help", "info", "contact",
            "admin", "webmaster", "dev", "developer", "engineering",
            "github", "gitlab", "bitbucket", "hello", "hi"
        ];
        
        // Include business-focused patterns
        let business_patterns = [
            "ceo", "cto", "cfo", "coo", "founder", "president", "vp",
            "director", "head", "chief", "sales", "business", "partnerships"
        ];
        
        let email_lower = email.to_lowercase();
        
        // Exclude if matches exclude patterns
        if exclude_patterns.iter().any(|&pattern| email_lower.contains(pattern)) {
            return false;
        }
        
        // Include if matches business patterns
        if business_patterns.iter().any(|&pattern| email_lower.contains(pattern)) {
            return true;
        }
        
        // Include personal emails (likely founders/decision makers)
        if email_lower.contains("gmail") || email_lower.contains("outlook") {
            return true;
        }
        
        // Default to true for custom domain emails
        true
    }

    fn extract_person_details(&self, email: &str, html: &str, text: &str) -> (Option<String>, Option<String>, String, String) {
        // Look for name and title near the email
        let email_context = self.find_email_context(email, html, text);
        
        let name = self.extract_name_from_context(&email_context);
        let title = self.extract_title_from_context(&email_context);
        
        let role_category = self.categorize_role(&title.as_deref().unwrap_or(""));
        let seniority = self.determine_seniority(&title.as_deref().unwrap_or(""));
        
        (name, title, role_category, seniority)
    }

    fn is_decision_maker(&self, title: &Option<String>, role_category: &str) -> bool {
        if let Some(title_str) = title {
            let title_lower = title_str.to_lowercase();
            if title_lower.contains("ceo") || title_lower.contains("founder") || 
               title_lower.contains("president") || title_lower.contains("owner") {
                return true;
            }
        }
        
        matches!(role_category, "founder" | "c-level" | "vp")
    }

    fn categorize_role(&self, title: &str) -> String {
        let title_lower = title.to_lowercase();
        
        if title_lower.contains("founder") || title_lower.contains("co-founder") {
            "founder"
        } else if title_lower.contains("ceo") || title_lower.contains("cto") || 
                  title_lower.contains("cfo") || title_lower.contains("coo") ||
                  title_lower.contains("chief") {
            "c-level"
        } else if title_lower.contains("vp") || title_lower.contains("vice president") {
            "vp"
        } else if title_lower.contains("director") {
            "director"
        } else if title_lower.contains("head of") || title_lower.contains("lead") {
            "head"
        } else if title_lower.contains("manager") {
            "manager"
        } else {
            "individual"
        }.to_string()
    }

    fn determine_seniority(&self, title: &str) -> String {
        let title_lower = title.to_lowercase();
        
        if title_lower.contains("ceo") || title_lower.contains("founder") || 
           title_lower.contains("president") || title_lower.contains("chief") {
            "c-level"
        } else if title_lower.contains("vp") || title_lower.contains("vice president") {
            "vp"
        } else if title_lower.contains("director") || title_lower.contains("head of") {
            "director"
        } else if title_lower.contains("manager") || title_lower.contains("lead") {
            "manager"
        } else {
            "individual"
        }.to_string()
    }

    fn guess_department(&self, title: &str) -> Option<String> {
        let title_lower = title.to_lowercase();
        
        if title_lower.contains("engineer") || title_lower.contains("developer") || 
           title_lower.contains("cto") || title_lower.contains("technical") {
            Some("engineering".to_string())
        } else if title_lower.contains("marketing") || title_lower.contains("growth") {
            Some("marketing".to_string())
        } else if title_lower.contains("sales") || title_lower.contains("business development") {
            Some("sales".to_string())
        } else if title_lower.contains("product") {
            Some("product".to_string())
        } else if title_lower.contains("finance") || title_lower.contains("cfo") {
            Some("finance".to_string())
        } else if title_lower.contains("hr") || title_lower.contains("people") {
            Some("people".to_string())
        } else {
            None
        }
    }

    // Helper methods...
    fn is_team_or_leadership_page(&self, url: &str, html: &str) -> bool {
        let url_lower = url.to_lowercase();
        let html_lower = html.to_lowercase();
        
        (url_lower.contains("/team") || url_lower.contains("/about") || 
         url_lower.contains("/leadership") || url_lower.contains("/people")) ||
        (html_lower.contains("our team") || html_lower.contains("leadership") || 
         html_lower.contains("meet the team"))
    }

    fn calculate_business_confidence(&self, contact: &ContactInfo, is_team_page: bool) -> f64 {
        let mut confidence:f32 = 0.5;
        
        // Higher confidence for team pages
        if is_team_page {
            confidence += 0.3;
        }
        
        // Higher confidence for business email patterns
        if contact.value.contains("ceo") || contact.value.contains("founder") {
            confidence += 0.4;
        }
        
        // Higher confidence for custom domains vs generic
        if !contact.value.contains("gmail") && !contact.value.contains("outlook") {
            confidence += 0.1;
        }
        
        confidence.min(1.0).into()
    }

    // Additional helper methods for name/title extraction...
    fn find_email_context(&self, email: &str, _html: &str, text: &str) -> String {
        // Implementation to find surrounding context of email
        // This would look for nearby text that might contain name/title
        let email_pos = text.find(email).unwrap_or(0);
        let start = email_pos.saturating_sub(200);
        let end = (email_pos + 200).min(text.len());
        text[start..end].to_string()
    }

    fn extract_name_from_context(&self, _context: &str) -> Option<String> {
        // Implementation to extract person's name from context
        // Would use patterns to find likely names near email
        None // Simplified for now
    }

    fn extract_title_from_context(&self, context: &str) -> Option<String> {
        // Implementation to extract job title from context
        if let Some(captures) = self.name_title_regex.captures(context) {
            captures.get(0).map(|m| m.as_str().to_string())
        } else {
            None
        }
    }

    fn extract_context_around_email(&self, text: &str, start: usize, end: usize) -> String {
        let context_range = 100;
        let text_start = start.saturating_sub(context_range);
        let text_end = (end + context_range).min(text.len());
        
        text[text_start..text_end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn clean_company_name(&self, name: &str) -> String {
        name.replace(" | ", " ")
            .replace(" - ", " ")
            .trim()
            .to_string()
    }

    fn domain_to_company_name(&self, domain: &str) -> String {
        domain.replace("www.", "")
              .split('.')
              .next()
              .unwrap_or(domain)
              .replace("-", " ")
              .replace("_", " ")
              .to_string()
    }

    // Additional extraction methods for company details...
    fn estimate_employee_count(&self) -> Option<String> {
        // Look for employee count indicators in text
        None // Simplified for now
    }

    fn detect_funding_stage(&self, document: &Html) -> Option<String> {
        let text = document.root_element().text().collect::<String>().to_lowercase();
        
        if text.contains("series a") {
            Some("series-a".to_string())
        } else if text.contains("series b") {
            Some("series-b".to_string())
        } else if text.contains("seed") {
            Some("seed".to_string())
        } else if text.contains("pre-seed") {
            Some("pre-seed".to_string())
        } else {
            None
        }
    }

    fn extract_location(&self) -> Option<String> {
        // Look for location information
        None // Simplified for now
    }

    fn extract_founded_year(&self) -> Option<i32> {
        // Look for founding year
        None // Simplified for now
    }
}
