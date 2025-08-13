// src/web_crawler/contact_extractor.rs
use crate::web_crawler::types::{ContactInfo, ContactType};
use regex::Regex;
use std::collections::HashSet;
use tracing::{debug, info};

pub struct ContactExtractor {
    email_regex: Regex,
    phone_regex: Regex,
    linkedin_regex: Regex,
    twitter_regex: Regex,
}

impl ContactExtractor {
    pub fn new() -> Self {
        Self {
            email_regex: Regex::new(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b").unwrap(),
            phone_regex: Regex::new(r"(?:\+?1[-.\s]?)?\(?([0-9]{3})\)?[-.\s]?([0-9]{3})[-.\s]?([0-9]{4})(?:\s?(?:ext|x|extension)\.?\s?(\d+))?").unwrap(),
            linkedin_regex: Regex::new(r"(?:https?://)?(?:www\.)?linkedin\.com/(?:in|company)/([a-zA-Z0-9\-_]+)").unwrap(),
            twitter_regex: Regex::new(r"(?:https?://)?(?:www\.)?(?:twitter\.com|x\.com)/([a-zA-Z0-9_]+)").unwrap(),
        }
    }

    pub fn extract_contacts(&self, html: &str, clean_text: &str, url: &str) -> Vec<ContactInfo> {
        let mut contacts = Vec::new();
        let mut seen_values = HashSet::new();

        // Extract emails with context
        contacts.extend(self.extract_emails(clean_text, html, url, &mut seen_values));
        
        // Extract phone numbers
        contacts.extend(self.extract_phones(clean_text, url, &mut seen_values));
        
        // Extract social media links
        contacts.extend(self.extract_social_media(html, url, &mut seen_values));
        
        // Extract contact forms
        contacts.extend(self.extract_contact_forms(html, url, &mut seen_values));

        info!("Found {} unique contacts on {}", contacts.len(), url);
        contacts
    }

    fn extract_emails(&self, text: &str, html: &str, url: &str, seen: &mut HashSet<String>) -> Vec<ContactInfo> {
        let mut emails = Vec::new();
        
        for captures in self.email_regex.captures_iter(text) {
            if let Some(email_match) = captures.get(0) {
                let email = email_match.as_str().to_lowercase();
                
                // Skip common non-contact emails
                if self.is_valid_contact_email(&email) && seen.insert(email.clone()) {
                    let context = self.extract_context(text, email_match.start(), email_match.end());
                    let confidence = self.calculate_email_confidence(&email, &context, html);
                    
                    emails.push(ContactInfo {
                        contact_type: ContactType::Email,
                        value: email,
                        context,
                        confidence,
                        source_url: url.to_string(),
                    });
                }
            }
        }

        debug!("Extracted {} emails from {}", emails.len(), url);
        emails
    }

    fn extract_phones(&self, text: &str, url: &str, seen: &mut HashSet<String>) -> Vec<ContactInfo> {
        let mut phones = Vec::new();
        
        for captures in self.phone_regex.captures_iter(text) {
            if let Some(phone_match) = captures.get(0) {
                let phone = self.normalize_phone(phone_match.as_str());
                
                if phone.len() >= 10 && seen.insert(phone.clone()) {
                    let context = self.extract_context(text, phone_match.start(), phone_match.end());
                    let confidence = self.calculate_phone_confidence(&phone, &context);
                    
                    phones.push(ContactInfo {
                        contact_type: ContactType::Phone,
                        value: phone,
                        context,
                        confidence,
                        source_url: url.to_string(),
                    });
                }
            }
        }

        debug!("Extracted {} phone numbers from {}", phones.len(), url);
        phones
    }

    fn extract_social_media(&self, html: &str, url: &str, seen: &mut HashSet<String>) -> Vec<ContactInfo> {
        let mut social = Vec::new();
        
        // LinkedIn
        for captures in self.linkedin_regex.captures_iter(html) {
            if let Some(profile) = captures.get(1) {
                let linkedin_url = format!("https://linkedin.com/in/{}", profile.as_str());
                if seen.insert(linkedin_url.clone()) {
                    social.push(ContactInfo {
                        contact_type: ContactType::LinkedIn,
                        value: linkedin_url,
                        context: "LinkedIn profile link".to_string(),
                        confidence: 0.8,
                        source_url: url.to_string(),
                    });
                }
            }
        }
        
        // Twitter/X
        for captures in self.twitter_regex.captures_iter(html) {
            if let Some(handle) = captures.get(1) {
                let twitter_url = format!("https://twitter.com/{}", handle.as_str());
                if seen.insert(twitter_url.clone()) {
                    social.push(ContactInfo {
                        contact_type: ContactType::Twitter,
                        value: twitter_url,
                        context: "Twitter/X profile link".to_string(),
                        confidence: 0.7,
                        source_url: url.to_string(),
                    });
                }
            }
        }

        debug!("Extracted {} social media links from {}", social.len(), url);
        social
    }

    fn extract_contact_forms(&self, html: &str, url: &str, seen: &mut HashSet<String>) -> Vec<ContactInfo> {
        let mut forms = Vec::new();
        
        // Look for contact forms
        let form_indicators = [
            r#"<form[^>]*action[^>]*contact"#,
            r#"<form[^>]*class[^>]*contact"#,
            r#"<input[^>]*type[^>]*email"#,
        ];
        
        for pattern in &form_indicators {
            if let Ok(regex) = Regex::new(pattern) {
                if regex.is_match(html) {
                    let form_url = format!("{}#contact-form", url);
                    if seen.insert(form_url.clone()) {
                        forms.push(ContactInfo {
                            contact_type: ContactType::ContactForm,
                            value: form_url,
                            context: "Contact form detected".to_string(),
                            confidence: 0.6,
                            source_url: url.to_string(),
                        });
                        break; // Only add one form per page
                    }
                }
            }
        }

        debug!("Found {} contact forms on {}", forms.len(), url);
        forms
    }

    fn is_valid_contact_email(&self, email: &str) -> bool {
        let invalid_patterns = [
            "noreply", "no-reply", "donotreply", "support@", "info@",
            "admin@", "webmaster@", "postmaster@", "example.com",
            "test@", "demo@", "sample@", "placeholder@",
        ];
        
        !invalid_patterns.iter().any(|&pattern| email.contains(pattern))
    }

    fn calculate_email_confidence(&self, email: &str, context: &str, html: &str) -> f32 {
        let mut confidence:f32 = 0.5;
        
        // Higher confidence for personal domains
        if email.contains("gmail") || email.contains("outlook") || email.contains("yahoo") {
            confidence += 0.2;
        }
        
        // Higher confidence if found in contact context
        let context_lower = context.to_lowercase();
        if context_lower.contains("contact") || context_lower.contains("reach out") || 
           context_lower.contains("email us") || context_lower.contains("get in touch") {
            confidence += 0.3;
        }
        
        // Higher confidence if on contact/about page
        if html.to_lowercase().contains("<title>") && 
           (html.to_lowercase().contains("contact") || html.to_lowercase().contains("about")) {
            confidence += 0.2;
        }
        
        confidence.min(1.0)
    }

    fn calculate_phone_confidence(&self, phone: &str, context: &str) -> f32 {
        let mut confidence:f32 = 0.6;
        
        // US phone numbers are more reliable
        if phone.len() == 10 || (phone.len() > 10 && phone.starts_with("+1")) {
            confidence += 0.2;
        }
        
        // Higher confidence in contact context
        let context_lower = context.to_lowercase();
        if context_lower.contains("phone") || context_lower.contains("call") || 
           context_lower.contains("tel") || context_lower.contains("contact") {
            confidence += 0.2;
        }
        
        confidence.min(1.0)
    }

    fn normalize_phone(&self, phone: &str) -> String {
        phone.chars()
            .filter(|c| c.is_ascii_digit() || *c == '+')
            .collect()
    }

    fn extract_context(&self, text: &str, start: usize, end: usize) -> String {
        let context_range = 50;
        let text_start = start.saturating_sub(context_range);
        let text_end = (end + context_range).min(text.len());
        
        text[text_start..text_end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn is_contact_page(&self, html: &str, url: &str) -> bool {
        let contact_indicators = [
            "contact", "about", "team", "staff", "leadership",
            "get in touch", "reach out", "contact us"
        ];
        
        let html_lower = html.to_lowercase();
        let url_lower = url.to_lowercase();
        
        contact_indicators.iter().any(|&indicator| {
            html_lower.contains(indicator) || url_lower.contains(indicator)
        })
    }

    pub fn has_contact_keywords(&self, text: &str) -> bool {
        let keywords = [
            "contact", "email", "phone", "call", "reach",
            "get in touch", "talk", "discuss", "inquiry"
        ];
        
        let text_lower = text.to_lowercase();
        keywords.iter().any(|&keyword| text_lower.contains(keyword))
    }
}
