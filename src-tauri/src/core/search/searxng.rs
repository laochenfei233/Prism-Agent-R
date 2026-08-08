use async_trait::async_trait;
use super::{SearchHit, SearchProvider};

pub struct SearxngProvider {
    base_url: String,
    client: reqwest::Client,
}

impl SearxngProvider {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SearchProvider for SearxngProvider {
    fn name(&self) -> &'static str {
        "searxng"
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
        let url = format!(
            "{}/search?q={}&format=json&engines=google,bing,duckduckgo",
            self.base_url,
            urlencoding::encode(query),
        );

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Searxng request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Searxng API error {status}: {body}"));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Searxng parse failed: {e}"))?;

        let mut hits = Vec::new();
        if let Some(results) = data["results"].as_array() {
            for r in results.iter().take(limit) {
                hits.push(SearchHit {
                    title: r["title"].as_str().unwrap_or("").to_string(),
                    url: r["url"].as_str().unwrap_or("").to_string(),
                    snippet: r["content"].as_str().unwrap_or("").to_string(),
                    source: "searxng".to_string(),
                    published_at: r["publishedDate"]
                        .as_str()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.timestamp_millis()),
                });
            }
        }
        Ok(hits)
    }
}
