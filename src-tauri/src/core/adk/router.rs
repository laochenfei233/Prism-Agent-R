use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RouteKind {
    Skill,
    McpTool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteItem {
    pub id: String,
    pub kind: RouteKind,
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResult {
    pub skills: Vec<RouteItem>,
    pub tools: Vec<RouteItem>,
    pub semantic_used: bool,
}

pub struct ToolRouter {
    items: Vec<RouteItem>,
}

impl ToolRouter {
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    pub fn refresh(&mut self, items: Vec<RouteItem>) {
        self.items = items;
    }

    pub fn route(&self, query: &str, top_k_skills: usize, top_k_tools: usize) -> RouteResult {
        let scores = self.bm25_score_all(query);
        let mut skills: Vec<(RouteItem, f32)> = scores.iter()
            .filter(|(item, _)| item.kind == RouteKind::Skill)
            .map(|(item, score)| (item.clone(), *score))
            .collect();
        let mut tools: Vec<(RouteItem, f32)> = scores.iter()
            .filter(|(item, _)| item.kind == RouteKind::McpTool)
            .map(|(item, score)| (item.clone(), *score))
            .collect();
        skills.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        tools.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        RouteResult {
            skills: skills.into_iter().take(top_k_skills).map(|(i, _)| i).collect(),
            tools: tools.into_iter().take(top_k_tools).map(|(i, _)| i).collect(),
            semantic_used: false,
        }
    }

    fn bm25_score_all(&self, query: &str) -> Vec<(RouteItem, f32)> {
        let query_tokens: Vec<String> = query.to_lowercase().split_whitespace().map(String::from).collect();
        self.items.iter().map(|item| {
            let text = format!("{} {} {}", item.name, item.description, item.keywords.join(" ")).to_lowercase();
            let score: f32 = query_tokens.iter().map(|t| text.matches(t.as_str()).count() as f32).sum();
            (item.clone(), score)
        }).collect()
    }
}
