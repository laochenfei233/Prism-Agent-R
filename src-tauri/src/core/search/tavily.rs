use async_trait::async_trait;
use super::{SearchHit, SearchProvider};

pub struct TavilyProvider {
    api_key: String,
    client: reqwest::Client,
}

impl TavilyProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SearchProvider for TavilyProvider {
    fn name(&self) -> &'static str {
        "tavily"
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
        let resp = self
            .client
            .post("https://api.tavily.com/search")
            .json(&serde_json::json!({
                "api_key": self.api_key,
                "query": query,
                "max_results": limit,
                "include_answer": false,
                "include_raw_content": false,
            }))
            .send()
            .await
            .map_err(|e| format!("Tavily request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Tavily API error {status}: {body}"));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Tavily parse failed: {e}"))?;

        let mut hits = Vec::new();
        if let Some(results) = data["results"].as_array() {
            for r in results {
                hits.push(SearchHit {
                    title: r["title"].as_str().unwrap_or("").to_string(),
                    url: r["url"].as_str().unwrap_or("").to_string(),
                    snippet: r["content"].as_str().unwrap_or("").to_string(),
                    source: "tavily".to_string(),
                    published_at: None,
                });
            }
        }
        Ok(hits)
    }
}
