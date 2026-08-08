use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecDocument {
    pub id: String,
    pub summary: String,
    pub tasks: Vec<SpecTask>,
    pub acceptance_criteria: std::collections::HashMap<String, Vec<String>>,
    pub dependencies: std::collections::HashMap<String, Vec<String>>,
    pub out_of_scope: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecTask {
    pub id: String,
    pub title: String,
    pub description: String,
    pub acceptance: Vec<String>,
    pub estimated_complexity: Complexity,
    pub required_tools: Vec<String>,
    pub suggested_model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Complexity {
    Low,
    Medium,
    High,
}

impl Complexity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}
