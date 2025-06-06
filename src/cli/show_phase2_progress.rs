use crate::{models::CliApp, Result};

impl CliApp {
    pub async fn show_phase2_progress(&self) -> Result<()> {
        println!("\n📈 Phase 2 Detailed Progress Analysis");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        let progress = self.get_phase2_progress_summary().await?;
        let conn = self.db_pool.get().await?;

        println!("📊 Overall Progress:");
        println!("  📦 Total GitHub projects: {}", progress.total);
        println!(
            "  ✅ Complete projects: {} ({:.1}%)",
            progress.complete, progress.completion_rate
        );
        println!("  🔄 Partial projects: {}", progress.partial);
        println!("  ⏳ Untouched projects: {}", progress.untouched);

        println!("\n📈 Data Breakdown:");
        let emails: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE email IS NOT NULL AND email != ''",
            [],
            |row| row.get(0),
        )?;
        let creation_dates: i64 = conn.query_row("SELECT COUNT(*) FROM projects WHERE repository_created IS NOT NULL AND repository_created != ''", [], |row| row.get(0))?;
        let first_commits: i64 = conn.query_row("SELECT COUNT(*) FROM projects WHERE first_commit_date IS NOT NULL AND first_commit_date != ''", [], |row| row.get(0))?;
        let contributors: i64 = conn.query_row("SELECT COUNT(*) FROM projects WHERE top_contributor_email IS NOT NULL AND top_contributor_email != ''", [], |row| row.get(0))?;

        println!(
            "  📧 Projects with emails: {} ({:.1}%)",
            emails,
            emails as f64 / progress.total as f64 * 100.0
        );
        println!(
            "  📅 Projects with creation date: {} ({:.1}%)",
            creation_dates,
            creation_dates as f64 / progress.total as f64 * 100.0
        );
        println!(
            "  🏁 Projects with first commit: {} ({:.1}%)",
            first_commits,
            first_commits as f64 / progress.total as f64 * 100.0
        );
        println!(
            "  🏆 Projects with contributors: {} ({:.1}%)",
            contributors,
            contributors as f64 / progress.total as f64 * 100.0
        );

        // Language breakdown for high-value projects
        println!("\n🎯 High-Value Project Analysis:");
        let rust_projects: i64 = conn.query_row("SELECT COUNT(*) FROM projects WHERE LOWER(url) LIKE '%rust%' OR LOWER(description) LIKE '%rust%'", [], |row| row.get(0))?;
        let js_projects: i64 = conn.query_row("SELECT COUNT(*) FROM projects WHERE LOWER(url) LIKE '%javascript%' OR LOWER(url) LIKE '%node%' OR LOWER(url) LIKE '%react%'", [], |row| row.get(0))?;
        let python_projects: i64 = conn.query_row("SELECT COUNT(*) FROM projects WHERE LOWER(url) LIKE '%python%' OR LOWER(description) LIKE '%python%'", [], |row| row.get(0))?;
        let go_projects: i64 = conn.query_row("SELECT COUNT(*) FROM projects WHERE LOWER(url) LIKE '%golang%' OR LOWER(description) LIKE '%golang%'", [], |row| row.get(0))?;
        let recent_projects: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE repository_created > '2022-01-01'",
            [],
            |row| row.get(0),
        )?;

        println!("  🦀 Rust projects: {}", rust_projects);
        println!("  🟨 JavaScript/Node.js projects: {}", js_projects);
        println!("  🐍 Python projects: {}", python_projects);
        println!("  ⚡ Go projects: {}", go_projects);
        println!("  🔥 Recent projects (2022+): {}", recent_projects);

        // Completion rates for high-value projects
        let rust_complete: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE (LOWER(url) LIKE '%rust%' OR LOWER(description) LIKE '%rust%') AND email IS NOT NULL AND email != '' AND first_commit_date IS NOT NULL", 
            [], |row| row.get(0)
        )?;
        let js_complete: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE (LOWER(url) LIKE '%javascript%' OR LOWER(url) LIKE '%node%' OR LOWER(url) LIKE '%react%') AND email IS NOT NULL AND email != '' AND first_commit_date IS NOT NULL", 
            [], |row| row.get(0)
        )?;

        if rust_projects > 0 {
            println!(
                "    • Rust completion rate: {:.1}%",
                rust_complete as f64 / rust_projects as f64 * 100.0
            );
        }
        if js_projects > 0 {
            println!(
                "    • JavaScript completion rate: {:.1}%",
                js_complete as f64 / js_projects as f64 * 100.0
            );
        }

        // Recent activity analysis
        println!("\n⏰ Recent Activity:");
        let updated_today: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE last_updated > ?",
            [&(chrono::Utc::now() - chrono::Duration::hours(24)).to_rfc3339()],
            |row| row.get(0),
        )?;
        let updated_week: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE last_updated > ?",
            [&(chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339()],
            |row| row.get(0),
        )?;

        println!("  📅 Updated in last 24 hours: {}", updated_today);
        println!("  📅 Updated in last 7 days: {}", updated_week);

        // Recommendations
        println!("\n💡 Recommendations:");
        if progress.untouched > 5000 {
            println!("  🎯 Use Smart Batch Processing with 'Mixed high-value batch'");
            println!("  📦 Process in batches of 200-500 projects");
            println!(
                "  🕐 Estimated time: {} hours at current rate",
                (progress.untouched / 100)
            );
        } else if progress.untouched > 1000 {
            println!("  🚀 Use Smart Batch Processing targeting specific languages");
            println!("  📦 Process in batches of 500-1000 projects");
        } else if progress.untouched > 0 {
            println!(
                "  ✨ Almost done! Process remaining {} projects",
                progress.untouched
            );
        }

        if progress.partial > 500 {
            println!("  🧹 Use 'Cleanup partial projects' to complete failed attempts");
        }

        if progress.completion_rate < 50.0 {
            println!("  ⚡ Focus on high-value projects first (Rust, JS, Python)");
        } else if progress.completion_rate > 80.0 {
            println!("  🎉 Great progress! Consider processing remaining projects");
        }

        // Performance tips
        if updated_today < 100 && progress.untouched > 1000 {
            println!("  🔧 Consider increasing batch size for faster processing");
            println!("  🕐 Run during off-peak hours for better GitHub API reliability");
        }

        Ok(())
    }
}
