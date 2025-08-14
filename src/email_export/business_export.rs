// src/email_export/business_export.rs
// Separate business contact export system

use crate::database::DbPool;
use super::types::ExportConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct BusinessEmailExport {
    pub email: String,
    pub company_name: String,
    pub domain: String,
    pub industry: String,
    pub company_type: String,
    pub full_name: String,
    pub first_name: String,
    pub job_title: String,
    pub role_category: String,
    pub seniority_level: String,
    pub department: String,
    pub is_decision_maker: bool,
    pub confidence: f64,
    pub investment_score: i32,
    pub funding_stage: String,
    pub employee_count_estimate: String,
    pub contact_source: String,
}

pub struct BusinessExportConfig {
    pub title: String,
    pub sql_filter: String,
    pub focus_area: BusinessFocus,
}

#[derive(Debug, Clone)]
pub enum BusinessFocus {
    DecisionMakers,     // CEOs, CTOs, Founders
    InvestmentTargets,  // Companies seeking funding
    TechLeaders,        // CTOs, Tech Directors  
    Founders,           // Founders and Co-founders
    ByIndustry(String), // Specific industry focus
    HighPotential,      // High investment score companies
}

pub struct BusinessEmailExporter;

impl BusinessEmailExporter {
    pub fn new() -> Self {
        Self
    }

    pub async fn export_business_contacts(
        &self,
        db_pool: &DbPool,
        config: &BusinessExportConfig,
    ) -> Result<Vec<BusinessEmailExport>, Box<dyn std::error::Error + Send + Sync>> {
        let conn = db_pool.get().await?;

        let sql = self.build_business_query(&config);
        
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(BusinessEmailExport {
                email: row.get(0)?,
                company_name: row.get(1)?,
                domain: row.get(2)?,
                industry: row.get::<_, Option<String>>(3)?.unwrap_or_else(|| "technology".to_string()),
                company_type: row.get::<_, Option<String>>(4)?.unwrap_or_else(|| "startup".to_string()),
                full_name: row.get::<_, Option<String>>(5)?.unwrap_or_else(|| "".to_string()),
                first_name: row.get::<_, Option<String>>(6)?.unwrap_or_else(|| "".to_string()),
                job_title: row.get::<_, Option<String>>(7)?.unwrap_or_else(|| "".to_string()),
                role_category: row.get::<_, Option<String>>(8)?.unwrap_or_else(|| "individual".to_string()),
                seniority_level: row.get::<_, Option<String>>(9)?.unwrap_or_else(|| "individual".to_string()),
                department: row.get::<_, Option<String>>(10)?.unwrap_or_else(|| "".to_string()),
                is_decision_maker: row.get::<_, i32>(11)? == 1,
                confidence: row.get(12)?,
                investment_score: row.get::<_, Option<i32>>(13)?.unwrap_or(0),
                funding_stage: row.get::<_, Option<String>>(14)?.unwrap_or_else(|| "unknown".to_string()),
                employee_count_estimate: row.get::<_, Option<String>>(15)?.unwrap_or_else(|| "unknown".to_string()),
                contact_source: "business_crawler".to_string(),
            })
        })?;

        let mut contacts = Vec::new();
        for row in rows {
            contacts.push(row?);
        }

        Ok(contacts)
    }

    fn build_business_query(&self, config: &BusinessExportConfig) -> String {
        let base_query = r#"
            SELECT DISTINCT
                bc.email,
                c.name as company_name,
                c.domain,
                c.industry,
                c.company_type,
                bc.full_name,
                bc.first_name,
                bc.job_title,
                bc.role_category,
                bc.seniority_level,
                bc.department,
                bc.is_decision_maker,
                bc.confidence,
                COALESCE(i.total_score, 0) as investment_score,
                c.funding_stage,
                c.employee_count_estimate
            FROM business_contacts bc
            JOIN companies c ON bc.company_id = c.id
            LEFT JOIN investment_scores i ON c.id = i.company_id
            WHERE bc.email IS NOT NULL 
            AND bc.email != ''
            AND bc.email NOT LIKE '%noreply%'
        "#;

        let filter = match &config.focus_area {
            BusinessFocus::DecisionMakers => {
                "AND bc.is_decision_maker = 1"
            },
            BusinessFocus::InvestmentTargets => {
                "AND (c.funding_stage IS NOT NULL OR COALESCE(i.total_score, 0) > 70)"
            },
            BusinessFocus::TechLeaders => {
                "AND (bc.role_category IN ('c-level', 'vp', 'director') AND bc.department = 'engineering')"
            },
            BusinessFocus::Founders => {
                "AND bc.role_category = 'founder'"
            },
            BusinessFocus::ByIndustry(industry) => {
                return format!("{} AND c.industry = '{}' ORDER BY bc.confidence DESC, i.total_score DESC", base_query, industry);
            },
            BusinessFocus::HighPotential => {
                "AND COALESCE(i.total_score, 0) > 60"
            },
        };

        format!("{} {} ORDER BY bc.confidence DESC, i.total_score DESC", base_query, filter)
    }

    pub async fn export_to_csv(
        &self,
        contacts: &[BusinessEmailExport],
        filename: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut file = std::fs::File::create(filename)?;
        
        // Enhanced CSV header for business contacts
        writeln!(
            file,
            "email,company_name,domain,industry,company_type,full_name,first_name,job_title,role_category,seniority_level,department,is_decision_maker,confidence,investment_score,funding_stage,employee_count,contact_source"
        )?;

        for contact in contacts {
            writeln!(
                file,
                "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},{},{},\"{}\",\"{}\",\"{}\"",
                contact.email,
                contact.company_name.replace("\"", "\"\""),
                contact.domain,
                contact.industry,
                contact.company_type,
                contact.full_name.replace("\"", "\"\""),
                contact.first_name.replace("\"", "\"\""),
                contact.job_title.replace("\"", "\"\""),
                contact.role_category,
                contact.seniority_level,
                contact.department,
                contact.is_decision_maker,
                contact.confidence,
                contact.investment_score,
                contact.funding_stage,
                contact.employee_count_estimate,
                contact.contact_source
            )?;
        }

        Ok(())
    }

    pub fn generate_business_filename(&self, focus: &BusinessFocus) -> String {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let suffix = match focus {
            BusinessFocus::DecisionMakers => "decision_makers",
            BusinessFocus::InvestmentTargets => "investment_targets", 
            BusinessFocus::TechLeaders => "tech_leaders",
            BusinessFocus::Founders => "founders",
            BusinessFocus::ByIndustry(industry) => &format!("industry_{}", industry),
            BusinessFocus::HighPotential => "high_potential",
        };
        
        format!("out/business_contacts_{}_{}.csv", suffix, timestamp)
    }

    pub fn print_business_stats(&self, contacts: &[BusinessEmailExport]) {
        println!("\n📊 Business Contact Export Statistics:");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let total = contacts.len();
        let decision_makers = contacts.iter().filter(|c| c.is_decision_maker).count();
        let c_level = contacts.iter().filter(|c| c.seniority_level == "c-level").count();
        let founders = contacts.iter().filter(|c| c.role_category == "founder").count();

        println!("👥 Total business contacts: {}", total);
        println!("🎯 Decision makers: {} ({:.1}%)", decision_makers, decision_makers as f64 / total as f64 * 100.0);
        println!("👑 C-level executives: {} ({:.1}%)", c_level, c_level as f64 / total as f64 * 100.0);
        println!("🚀 Founders: {} ({:.1}%)", founders, founders as f64 / total as f64 * 100.0);

        // Industry breakdown
        let mut industry_counts = std::collections::HashMap::new();
        for contact in contacts {
            *industry_counts.entry(contact.industry.clone()).or_insert(0) += 1;
        }

        println!("\n🏭 By Industry:");
        let mut sorted_industries: Vec<_> = industry_counts.iter().collect();
        sorted_industries.sort_by(|a, b| b.1.cmp(a.1));
        
        for (industry, count) in sorted_industries.iter().take(5) {
            println!("   • {}: {}", industry, count);
        }

        // Company type breakdown
        let mut type_counts = std::collections::HashMap::new();
        for contact in contacts {
            *type_counts.entry(contact.company_type.clone()).or_insert(0) += 1;
        }

        println!("\n🏢 By Company Type:");
        for (company_type, count) in &type_counts {
            println!("   • {}: {}", company_type, count);
        }

        // Investment potential
        let high_score = contacts.iter().filter(|c| c.investment_score > 70).count();
        let avg_score = if total > 0 {
            contacts.iter().map(|c| c.investment_score).sum::<i32>() as f64 / total as f64
        } else {
            0.0
        };

        println!("\n💰 Investment Potential:");
        println!("   📈 High-potential companies: {} ({:.1}%)", high_score, high_score as f64 / total as f64 * 100.0);
        println!("   📊 Average investment score: {:.1}", avg_score);

        // Confidence levels
        let high_confidence = contacts.iter().filter(|c| c.confidence > 0.8).count();
        let avg_confidence = if total > 0 {
            contacts.iter().map(|c| c.confidence).sum::<f64>() / total as f64
        } else {
            0.0
        };

        println!("\n🎯 Contact Quality:");
        println!("   ✨ High confidence contacts: {} ({:.1}%)", high_confidence, high_confidence as f64 / total as f64 * 100.0);
        println!("   📊 Average confidence: {:.2}", avg_confidence);
    }
}

// Update src/cli/mod.rs to include business crawler
pub mod run_business_crawler;
