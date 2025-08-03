use regex::Regex;

use super::core::Result;

pub struct UrlUtils {
    github_url_regex: Regex,
}

impl UrlUtils {
    pub fn new(github_url_regex: Regex) -> Self {
        Self { github_url_regex }
    }

    pub fn parse_github_url(&self, url: &str) -> Option<(String, String)> {
        if let Some(caps) = self.github_url_regex.captures(url) {
            let owner = caps.get(1)?.as_str().to_string();
            let repo = caps.get(2)?.as_str().to_string();
            Some((owner, repo))
        } else {
            None
        }
    }

    pub fn parse_github_url_result(&self, url: &str) -> Result<(String, String)> {
        self.parse_github_url(url).ok_or_else(|| {
            "Invalid GitHub URL format. Expected: https://github.com/owner/repo".into()
        })
    }
}
