use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCandidate {
    pub provider_id: String,
    pub model_id: String,
    pub display_name: String,
    pub cost_per_1k_tokens: f64,
    pub max_tokens: u64,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFallbackChain {
    pub models: Vec<ModelCandidate>,
    pub current_index: usize,
}

impl ModelFallbackChain {
    pub fn new(mut models: Vec<ModelCandidate>) -> Self {
        models.sort_by(|a, b| a.cost_per_1k_tokens.partial_cmp(&b.cost_per_1k_tokens).unwrap());
        Self { models, current_index: 0 }
    }

    pub fn current(&self) -> Option<&ModelCandidate> {
        self.models.get(self.current_index)
    }

    /// 超预算时切换到下一个更便宜的模型
    pub fn downgrade(&mut self) -> Option<&ModelCandidate> {
        if self.current_index + 1 < self.models.len() {
            self.current_index += 1;
            Some(&self.models[self.current_index])
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        self.current_index = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_chain_ordering() {
        let models = vec![
            ModelCandidate { provider_id: "a".into(), model_id: "expensive".into(), display_name: "Expensive".into(), cost_per_1k_tokens: 0.06, max_tokens: 128000, capabilities: vec![] },
            ModelCandidate { provider_id: "b".into(), model_id: "cheap".into(), display_name: "Cheap".into(), cost_per_1k_tokens: 0.001, max_tokens: 8000, capabilities: vec![] },
        ];
        let chain = ModelFallbackChain::new(models);
        assert_eq!(chain.current().unwrap().model_id, "cheap");
    }

    #[test]
    fn downgrade_returns_next() {
        let models = vec![
            ModelCandidate { provider_id: "a".into(), model_id: "a".into(), display_name: "A".into(), cost_per_1k_tokens: 0.001, max_tokens: 8000, capabilities: vec![] },
            ModelCandidate { provider_id: "b".into(), model_id: "b".into(), display_name: "B".into(), cost_per_1k_tokens: 0.01, max_tokens: 32000, capabilities: vec![] },
        ];
        let mut chain = ModelFallbackChain::new(models);
        assert!(chain.downgrade().is_some());
        assert_eq!(chain.current().unwrap().model_id, "b");
        assert!(chain.downgrade().is_none());
    }
}
