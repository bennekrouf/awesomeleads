// src/email_rate_limiting.rs
use crate::database::DbPool;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmailLimitsConfig {
    // Daily limits based on account age
    pub new_account: usize,
    pub warming_up: usize,
    pub established: usize,
    pub mature: usize,

    // Rate limiting
    pub emails_per_hour: usize,
    pub emails_per_minute: usize,
    pub delay_between_emails_ms: u64,

    // Ramp up
    pub enable_auto_ramp: bool,
    pub ramp_percentage_increase: f64,
    pub max_ramp_daily_limit: usize,

    // Safety
    pub max_emails_per_campaign: usize,
    pub require_confirmation_above: usize,

    // Warm-up mode
    pub warm_up_mode: bool,
    pub warm_up_daily_limits: Vec<usize>,
}

impl Default for EmailLimitsConfig {
    fn default() -> Self {
        Self {
            new_account: 50,
            warming_up: 200,
            established: 500,
            mature: 1000,
            emails_per_hour: 100,
            emails_per_minute: 5,
            delay_between_emails_ms: 3000,
            enable_auto_ramp: true,
            ramp_percentage_increase: 20.0,
            max_ramp_daily_limit: 2000,
            max_emails_per_campaign: 100,
            require_confirmation_above: 50,
            warm_up_mode: false,
            warm_up_daily_limits: vec![10, 20, 50, 100, 200, 300, 500],
        }
    }
}

#[derive(Debug)]
pub struct EmailRateLimiter {
    config: EmailLimitsConfig,
    db_pool: DbPool,
}

#[derive(Debug)]
pub struct RateLimitStatus {
    pub can_send: bool,
    pub daily_limit: usize,
    pub daily_sent: usize,
    pub remaining_today: usize,
    pub hourly_sent: usize,
    pub minute_sent: usize,
    pub account_age_days: i64,
    pub next_allowed_send: Option<DateTime<Utc>>,
    pub recommended_batch_size: usize,
    pub reason: String,
}

impl EmailRateLimiter {
    pub fn new(config: EmailLimitsConfig, db_pool: DbPool) -> Self {
        Self { config, db_pool }
    }

    pub async fn check_rate_limits(
        &self,
        requested_batch_size: usize,
    ) -> Result<RateLimitStatus, Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now();
        let conn = self.db_pool.get().await?;

        // Get account age (days since first email sent)
        let account_age_days = self.get_account_age_days(&conn).await?;

        // Get current usage
        let daily_sent = self.get_daily_sent(&conn, now).await?;
        let hourly_sent = self.get_hourly_sent(&conn, now).await?;
        let minute_sent = self.get_minute_sent(&conn, now).await?;

        // Calculate current daily limit
        let base_daily_limit = self.get_base_daily_limit(account_age_days);
        let daily_limit = if self.config.enable_auto_ramp {
            self.apply_ramp_up(base_daily_limit, account_age_days)
                .min(self.config.max_ramp_daily_limit)
        } else {
            base_daily_limit
        };

        // Apply warm-up mode if enabled
        let daily_limit = if self.config.warm_up_mode {
            self.apply_warm_up_limit(daily_limit, account_age_days)
        } else {
            daily_limit
        };

        let remaining_today = daily_limit.saturating_sub(daily_sent);

        // Check limits
        let (can_send, reason) = self.evaluate_limits(
            requested_batch_size,
            remaining_today,
            hourly_sent,
            minute_sent,
        );

        // Calculate recommended batch size
        let recommended_batch_size =
            self.calculate_recommended_batch_size(remaining_today, hourly_sent, minute_sent);

        // Calculate next allowed send time if blocked
        let next_allowed_send = if !can_send {
            Some(self.calculate_next_allowed_send(hourly_sent, minute_sent))
        } else {
            None
        };

        Ok(RateLimitStatus {
            can_send,
            daily_limit,
            daily_sent,
            remaining_today,
            hourly_sent,
            minute_sent,
            account_age_days,
            next_allowed_send,
            recommended_batch_size,
            reason,
        })
    }

    pub async fn get_optimal_delay(&self) -> u64 {
        // Calculate delay based on current rate
        let base_delay = self.config.delay_between_emails_ms;

        // Add some jitter to avoid looking too robotic
        let jitter = fastrand::u64(0..=1000); // 0-1 second jitter
        base_delay + jitter
    }

    async fn get_account_age_days(
        &self,
        conn: &rusqlite::Connection,
    ) -> Result<i64, Box<dyn std::error::Error + Send + Sync>> {
        let first_email_date: Option<String> = conn
            .query_row("SELECT MIN(sent_at) FROM email_tracking", [], |row| {
                row.get(0)
            })
            .unwrap_or(None);

        match first_email_date {
            Some(date_str) => {
                if let Ok(first_date) = DateTime::parse_from_rfc3339(&date_str) {
                    let age = Utc::now().signed_duration_since(first_date.with_timezone(&Utc));
                    Ok(age.num_days())
                } else {
                    Ok(0) // If we can't parse the date, treat as new account
                }
            }
            None => Ok(0), // No emails sent yet
        }
    }

    async fn get_daily_sent(
        &self,
        conn: &rusqlite::Connection,
        now: DateTime<Utc>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let today_start = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .to_rfc3339();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE sent_at >= ? AND campaign_type NOT LIKE 'debug_%'",
            [today_start],
            |row| row.get(0)
        )?;

        Ok(count as usize)
    }

    async fn get_hourly_sent(
        &self,
        conn: &rusqlite::Connection,
        now: DateTime<Utc>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let hour_ago = (now - Duration::hours(1)).to_rfc3339();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE sent_at >= ? AND campaign_type NOT LIKE 'debug_%'",
            [hour_ago],
            |row| row.get(0)
        )?;

        Ok(count as usize)
    }

    async fn get_minute_sent(
        &self,
        conn: &rusqlite::Connection,
        now: DateTime<Utc>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let minute_ago = (now - Duration::minutes(1)).to_rfc3339();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM email_tracking WHERE sent_at >= ? AND campaign_type NOT LIKE 'debug_%'",
            [minute_ago],
            |row| row.get(0)
        )?;

        Ok(count as usize)
    }

    fn get_base_daily_limit(&self, account_age_days: i64) -> usize {
        match account_age_days {
            0..=7 => self.config.new_account,
            8..=30 => self.config.warming_up,
            31..=90 => self.config.established,
            _ => self.config.mature,
        }
    }

    fn apply_ramp_up(&self, base_limit: usize, account_age_days: i64) -> usize {
        if account_age_days <= 7 {
            return base_limit; // No ramp-up for very new accounts
        }

        let weeks_active = (account_age_days / 7) as f64;
        let ramp_multiplier =
            1.0 + (self.config.ramp_percentage_increase / 100.0 * (weeks_active - 1.0));

        (base_limit as f64 * ramp_multiplier) as usize
    }

    fn apply_warm_up_limit(&self, base_limit: usize, account_age_days: i64) -> usize {
        if account_age_days >= self.config.warm_up_daily_limits.len() as i64 {
            return base_limit;
        }

        let warm_up_limit = self
            .config
            .warm_up_daily_limits
            .get(account_age_days as usize)
            .copied()
            .unwrap_or(base_limit);

        warm_up_limit.min(base_limit)
    }

    fn evaluate_limits(
        &self,
        requested: usize,
        remaining_daily: usize,
        hourly_sent: usize,
        minute_sent: usize,
    ) -> (bool, String) {
        // Check daily limit
        if requested > remaining_daily {
            return (
                false,
                format!(
                    "Daily limit: requested {} but only {} remaining today",
                    requested, remaining_daily
                ),
            );
        }

        // Check campaign limit
        if requested > self.config.max_emails_per_campaign {
            return (
                false,
                format!(
                    "Campaign limit: requested {} but max per campaign is {}",
                    requested, self.config.max_emails_per_campaign
                ),
            );
        }

        // REMOVE the hourly and minute checks here - let the pacing handle it
        (true, "All limits OK".to_string())
    }

    fn calculate_recommended_batch_size(
        &self,
        remaining_daily: usize,
        hourly_sent: usize,
        minute_sent: usize,
    ) -> usize {
        let remaining_hourly = self.config.emails_per_hour.saturating_sub(hourly_sent);
        let remaining_minute = self.config.emails_per_minute.saturating_sub(minute_sent);

        // Take the most restrictive limit, but cap at campaign max
        let recommended = remaining_daily
            .min(remaining_hourly)
            .min(remaining_minute)
            .min(self.config.max_emails_per_campaign);

        // If we're hitting minute limits, suggest a smaller batch
        if remaining_minute < 5 {
            recommended.min(remaining_minute)
        } else {
            recommended
        }
    }

    fn calculate_next_allowed_send(&self, hourly_sent: usize, minute_sent: usize) -> DateTime<Utc> {
        let now = Utc::now();

        // If we're hitting minute limits, wait until next minute
        if minute_sent >= self.config.emails_per_minute {
            return now + Duration::minutes(1);
        }

        // If we're hitting hourly limits, wait until next hour
        if hourly_sent >= self.config.emails_per_hour {
            return now + Duration::hours(1);
        }

        // Otherwise, just wait the standard delay
        now + Duration::milliseconds(self.config.delay_between_emails_ms as i64)
    }

    pub fn display_status(&self, status: &RateLimitStatus) {
        println!("\n📊 Email Rate Limit Status");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Account status
        let account_status = match status.account_age_days {
            0..=7 => "🟡 New Account (Limited)",
            8..=30 => "🟠 Warming Up",
            31..=90 => "🟢 Established",
            _ => "🔵 Mature Account",
        };
        println!(
            "📅 Account Status: {} ({} days old)",
            account_status, status.account_age_days
        );

        // Daily limits
        let daily_percentage = if status.daily_limit > 0 {
            (status.daily_sent as f64 / status.daily_limit as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "📈 Daily Usage: {}/{} ({:.1}%) - {} remaining",
            status.daily_sent, status.daily_limit, daily_percentage, status.remaining_today
        );

        // Hourly limits
        let hourly_percentage = if self.config.emails_per_hour > 0 {
            (status.hourly_sent as f64 / self.config.emails_per_hour as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "🕐 Hourly Usage: {}/{} ({:.1}%)",
            status.hourly_sent, self.config.emails_per_hour, hourly_percentage
        );

        // Minute limits
        println!(
            "⏱️  Last Minute: {}/{}",
            status.minute_sent, self.config.emails_per_minute
        );

        // Status
        if status.can_send {
            println!("✅ Status: Ready to send");
            println!(
                "💡 Recommended batch size: {}",
                status.recommended_batch_size
            );
        } else {
            println!("❌ Status: Rate limited");
            println!("🚫 Reason: {}", status.reason);
            if let Some(next_time) = status.next_allowed_send {
                println!("⏰ Next allowed: {}", next_time.format("%H:%M:%S UTC"));
            }
        }

        // Safety reminders
        if status.daily_sent > 0 {
            let success_advice = if status.daily_sent < 50 {
                "Great! You're building sender reputation safely."
            } else if status.daily_sent < 200 {
                "Good pace. Monitor for bounces and complaints."
            } else {
                "High volume today. Watch your metrics carefully."
            };
            println!("💡 {}", success_advice);
        }
    }
}
