pub mod anthropic;
pub mod ollama;
pub mod openai;
pub mod responses_api;

use std::sync::Arc;

use crate::core::adk::model::ModelProvider;
use crate::data::models::{ModelRow, ProviderRow};

pub use anthropic::AnthropicProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAiProvider;
pub use responses_api::ResponsesApiProvider;

/// 按 provider.kind 分发创建模型 Provider（chat / compose 共用）。
///
/// - `responses-api` → ResponsesApiProvider
/// - `anthropic` → AnthropicProvider
/// - 其他（openai / ollama / 兼容网关）→ OpenAiProvider
pub fn build_provider(
    provider_row: &ProviderRow,
    model_row: &ModelRow,
    api_key: String,
    base_url: String,
) -> Arc<dyn ModelProvider> {
    let display_name = model_row
        .display_name
        .clone()
        .unwrap_or_else(|| model_row.model_id.clone());
    match provider_row.kind.as_str() {
        "responses-api" => Arc::new(ResponsesApiProvider::new(
            model_row.provider_id.clone(),
            display_name,
            api_key,
            base_url,
            model_row.model_id.clone(),
        )),
        "anthropic" => Arc::new(AnthropicProvider::new(
            model_row.provider_id.clone(),
            display_name,
            api_key,
            base_url,
            model_row.model_id.clone(),
        )),
        _ => Arc::new(OpenAiProvider::new(
            model_row.provider_id.clone(),
            display_name,
            api_key,
            base_url,
            model_row.model_id.clone(),
        )),
    }
}
