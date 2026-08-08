use async_trait::async_trait;
use super::{SearchHit, SearchProvider};

pub struct SerperProvider {
    api_key: String,
    client: reqwest::Client,
}

impl SerperProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SearchProvider for SerperProvider {
    fn name(&self) -> &'static str {
        "serper"
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
        let resp = self
            .client
            .post("https://google.serper.dev/search")
            .header("X-API-KEY", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "q": query,
                "num": limit,
            }))
            .send()
            .await
            .map_err(|e| format!("Serper request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Serper API error {status}: {body}"));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Serper parse failed: {e}"))?;

        let mut hits = Vec::new();
        if let Some(organic) = data["organic"].as_array() {
            for r in organic {
                hits.push(SearchHit {
                    title: r["title"].as_str().unwrap_or("").to_string(),
                    url: r["link"].as_str().unwrap_or("").to_string(),
                    snippet: r["snippet"].as_str().unwrap_or("").to_string(),
                    source: "serper".to_string(),
                    published_at: None,
                });
            }
        }
        Ok(hits)
    }
}
