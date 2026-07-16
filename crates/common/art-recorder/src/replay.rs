use thiserror::Error;

use crate::schema::RecordingEntry;

#[derive(Debug, Clone)]
pub struct ReplayClient {
    base_url: String,
}

impl ReplayClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    pub fn lookup_url(
        &self,
        session_id: &str,
        counter: usize,
    ) -> Result<reqwest::Url, ReplayError> {
        let endpoint = format!("{}/mockRedis", self.base_url.trim_end_matches('/'));
        let mut url = reqwest::Url::parse(&endpoint).map_err(|error| ReplayError::InvalidUrl {
            message: error.to_string(),
        })?;
        url.query_pairs_mut()
            .append_pair("guuid", session_id)
            .append_pair("counter", &counter.to_string());
        Ok(url)
    }

    pub async fn fetch_entry(
        &self,
        session_id: &str,
        counter: usize,
    ) -> Result<RecordingEntry, ReplayError> {
        let url = self.lookup_url(session_id, counter)?;
        let client = reqwest::Client::builder().no_proxy().build()?;
        let response = client.get(url).send().await?.error_for_status()?;
        Ok(response.json::<RecordingEntry>().await?)
    }
}

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("invalid ART replay lookup URL: {message}")]
    InvalidUrl { message: String },
    #[error("failed to fetch ART replay entry")]
    Fetch(#[from] reqwest::Error),
}
