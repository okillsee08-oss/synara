use anyhow::Result;
#[derive(Clone, Debug)]
pub struct BrowserSession {
    pub id: String,
    pub url: Option<String>,
}
impl BrowserSession {
    pub fn validate_url(url: &str) -> Result<()> {
        let trimmed = url.trim();
        if trimmed.is_empty() { anyhow::bail!("browser URL cannot be empty"); }
        let parsed = url::Url::parse(trimmed)?;
        match parsed.scheme() { "http" | "https" => Ok(()), _ => anyhow::bail!("unsupported browser URL scheme") }
    }
}

pub trait BrowserDriver: Send + Sync {
    fn navigate(&self, url: &str) -> Result<()>;
    fn screenshot(&self) -> Result<Vec<u8>>;
}


#[cfg(test)]
mod tests { use super::*; #[test] fn url_policy_rejects_non_web_schemes() { assert!(BrowserSession::validate_url(" ").is_err()); assert!(BrowserSession::validate_url("file:///tmp/a").is_err()); assert!(BrowserSession::validate_url("https://example.com").is_ok()); } }
