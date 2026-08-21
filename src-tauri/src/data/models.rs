use serde::{Deserialize, Serialize};

// ── Provider ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub api_key_enc: Option<String>,
    pub is_enabled: i32,
    pub created_at: i64,
    pub updated_at: i64,
    /// 自定义图标（预置 Logo 或用户上传路径）；旧查询未 select 时默认 None
    #[sqlx(default)]
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub is_enabled: bool,
    /// 是否已配置 API Key（仅暴露布尔值，不回传密文）
    pub has_key: bool,
    /// 自定义图标（预置 Logo 或用户上传路径）
    pub avatar: Option<String>,
}

// ── Model ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ModelRow {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub kind: String,
    pub max_tokens: Option<i32>,
    pub is_default: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDto {
    pub id: String,
    pub provider_id: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub kind: String,
    pub max_tokens: Option<i32>,
    pub is_default: bool,
}

// ── Agent ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar: Option<String>,
    pub system_prompt: Option<String>,
    pub model_id: Option<String>,
    pub plan_model_id: Option<String>,
    pub small_model_id: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub disabled_tools: String,
    pub configuration: String,
    pub order_key: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar: Option<String>,
    pub system_prompt: Option<String>,
    pub model_id: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub disabled_tools: Vec<String>,
    pub order_key: i32,
}

// ── Session ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub agent_id: String,
    pub title: Option<String>,
    pub pinned: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionDto {
    pub id: String,
    pub agent_id: String,
    pub title: Option<String>,
    pub pinned: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

// ── Message ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MessageRow {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<String>,
    pub tool_call_id: Option<String>,
    pub model_id: Option<String>,
    pub usage: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDto {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub tool_calls: Option<serde_json::Value>,
    pub tool_call_id: Option<String>,
    pub model_id: Option<String>,
    pub usage: Option<serde_json::Value>,
    pub created_at: i64,
}

// ── MCP Server ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct McpServerRow {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub command: Option<String>,
    pub args: String,
    pub env: String,
    pub base_url: Option<String>,
    pub headers: String,
    pub is_active: i32,
    pub timeout_ms: Option<i32>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerDto {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub base_url: Option<String>,
    pub is_active: bool,
    pub timeout_ms: Option<i32>,
}

// ── Skill ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SkillRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub folder_name: String,
    pub source: String,
    pub source_url: Option<String>,
    pub namespace: Option<String>,
    pub author: Option<String>,
    pub tags: String,
    pub content_hash: String,
    pub is_enabled: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub folder_name: String,
    pub source: String,
    pub is_enabled: bool,
}

// ── Sidebar types ──

#[derive(Serialize, Deserialize, Clone)]
pub struct AgentContext {
    pub agent: AgentDto,
    pub session_usage: SessionUsage,
    pub workspace: WorkspaceInfo,
    pub instructions: Vec<InstructionFile>,
    pub mcp: Vec<McpServerStatus>,
    pub lsp: Vec<LspServerInfo>,
    pub tree: DirTree,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub context_used: u64,
    pub context_limit: u64,
    pub tool_calls: u64,
    pub cost_est: f64,
    pub today_calls: u64,
    pub today_tokens: u64,
    pub today_cost: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WorkspaceInfo {
    pub current_dir: String,
    pub recent_dirs: Vec<String>,
    pub bound_agent_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct InstructionFile {
    pub path: String,
    pub name: String,
    pub lines: usize,
    pub injected: bool,
    pub priority: u8,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LspServerInfo {
    pub id: String,
    pub cmd: String,
    pub status: String,
    pub langs: Vec<String>,
    pub index_file_count: Option<u64>,
    pub last_error: Option<String>,
    pub install_hint: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DirTree {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<DirTree>>,
    pub language: Option<String>,
    pub line_count: Option<u64>,
}

// ── File types ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFile {
    pub path: String,
    pub kind: String,
    pub content: Option<String>,
    pub json: Option<serde_json::Value>,
    pub size: u64,
    pub mime: Option<String>,
}

// ── Dashboard types ──

#[derive(Serialize, Deserialize, Clone)]
pub struct UsageStats {
    pub today_tokens: u64,
    pub week_tokens: u64,
    pub month_tokens: u64,
    pub month_cost: f64,
    pub today_calls: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct UsagePoint {
    pub date: String,
    pub tokens: u64,
    pub cost: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SkillOverview {
    pub enabled: usize,
    pub total: usize,
    pub popular: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct McpServerStatus {
    pub id: String,
    pub name: String,
    pub status: String,
    pub tools_count: usize,
    pub last_error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub agent_name: String,
    pub updated_at: String,
    pub message_count: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ModelStatus {
    pub provider_name: String,
    pub model_id: String,
    pub display_name: String,
    pub status: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub avatar: Option<String>,
    pub model_name: Option<String>,
    pub skill_count: usize,
    pub mcp_count: usize,
    pub last_used: Option<String>,
    pub order_key: i32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DashboardOverview {
    pub agents: Vec<AgentSummary>,
    pub usage: UsageStats,
    pub usage_trend: Vec<UsagePoint>,
    pub skills: SkillOverview,
    pub mcp_servers: Vec<McpServerStatus>,
    pub recent_sessions: Vec<SessionSummary>,
    pub models: Vec<ModelStatus>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct KanbanCard {
    pub agent_id: String,
    pub agent_name: String,
    pub agent_avatar: Option<String>,
    pub model_name: Option<String>,
    pub session_id: Option<String>,
    pub session_title: Option<String>,
    pub session_updated_at: Option<i64>,
    pub lifecycle: String,
    pub message_count: i64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct KanbanData {
    pub idle: Vec<KanbanCard>,
    pub running: Vec<KanbanCard>,
    pub done: Vec<KanbanCard>,
    pub failed: Vec<KanbanCard>,
}

// ── RAG ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RagDocumentRow {
    pub id: String,
    pub wiki_id: String,
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub chunk_count: i32,
    pub status: String,
    pub error_msg: Option<String>,
    pub file_path: Option<String>,
    pub fingerprint: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectIndexStatus {
    pub enabled: bool,
    pub workdir: Option<String>,
    pub indexed_files: i64,
    pub in_progress: bool,
    pub last_indexed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagDocumentDto {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub size: i64,
    pub chunk_count: i32,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RagChunkRow {
    pub id: String,
    pub document_id: String,
    pub wiki_id: String,
    #[sqlx(rename = "index")]
    pub index: i32,
    pub content: String,
    pub embedding: Option<Vec<u8>>,
    pub context: Option<String>,
    pub page_start: Option<i32>,
    pub page_end: Option<i32>,
    pub section: Option<String>,
    pub block_type: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagHit {
    pub chunk_id: String,
    pub document_title: String,
    pub page_start: Option<i32>,
    pub page_end: Option<i32>,
    pub section: Option<String>,
    pub quote: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagHitDto {
    pub chunk_id: String,
    pub document_title: String,
    pub page_start: Option<i32>,
    pub page_end: Option<i32>,
    pub section: Option<String>,
    pub quote: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResult {
    pub document_id: String,
    pub chunk_count: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResultDto {
    pub document_id: String,
    pub chunk_count: usize,
    pub status: String,
}

// ── 会议 ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateResult {
    pub translated: String,
    pub source_lang: String,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateResultDto {
    pub translated: String,
    pub source_lang: String,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TranslateHistoryRow {
    pub id: String,
    pub source_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub translated: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateHistoryDto {
    pub id: String,
    pub source_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub translated: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateHistoryResult {
    pub items: Vec<TranslateHistoryDto>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateHistoryResultDto {
    pub items: Vec<TranslateHistoryDto>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectResult {
    pub lang: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectResultDto {
    pub lang: String,
    pub confidence: f32,
}

// ── 术语表 ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GlossaryTermRow {
    pub id: String,
    pub source_lang: String,
    pub target_lang: String,
    pub source_term: String,
    pub target_term: String,
    pub category: Option<String>,
    pub enabled: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryTerm {
    pub id: String,
    pub source_lang: String,
    pub target_lang: String,
    pub source_term: String,
    pub target_term: String,
    pub category: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryTermDto {
    pub id: String,
    pub source_lang: String,
    pub target_lang: String,
    pub source_term: String,
    pub target_term: String,
    pub category: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryTermInput {
    pub source_lang: String,
    pub target_lang: String,
    pub source_term: String,
    pub target_term: String,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub imported: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResultDto {
    pub imported: usize,
    pub failed: usize,
}

/// 内置词库条目（一键导入列表展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinGlossaryDto {
    pub file: String,
    pub label: String,
    pub description: String,
}

// ── OCR ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrBlock {
    pub text: String,
    pub bbox: (f32, f32, f32, f32),
    pub confidence: f32,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    pub text: String,
    pub lang: String,
    pub provider: String,
    #[serde(default)]
    pub blocks: Vec<OcrBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResultDto {
    pub text: String,
    pub lang: String,
    pub provider: String,
    #[serde(default)]
    pub blocks: Vec<OcrBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrProviderInfo {
    pub name: String,
    pub kind: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrProviderInfoDto {
    pub name: String,
    pub kind: String,
    pub available: bool,
}

// ── Wiki ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WikiRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub schema: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPage {
    pub path: String,
    pub title: String,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPageDto {
    pub path: String,
    pub title: String,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPageHit {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPageHitDto {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: f64,
}

// ── Wiki AI 写入（§10.1.1） ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiWritePlan {
    pub operations: Vec<WikiOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WikiOp {
    /// 新建页面（path 相对 wiki/ 根）
    CreatePage {
        path: String,
        title: String,
        content: String,
    },
    /// 更新现有页面（content 为全文替换）
    UpdatePage {
        path: String,
        content: String,
        summary: String,
    },
    /// 删除页面（软删除到 .trash/）
    DeletePage { path: String, reason: String },
    /// 追加 index.md 条目
    UpdateIndex { entries: Vec<String> },
    /// 跳过（信息重复，无变更）
    Noop { reason: String },
}

/// 一次 write_ai 的执行结果
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WikiWriteResult {
    pub applied: usize,
    pub noop: usize,
    pub summary: String,
    pub log_appended: bool,
}

// ── 会议 ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MeetingRow {
    pub id: String,
    pub title: String,
    pub date: String,
    pub transcript: String,
    pub summary: String,
    pub participants: String,
    pub recording_duration: i32,
    pub audio_path: Option<String>,
    pub folder_path: Option<String>,
    pub source_lang: Option<String>,
    pub target_lang: Option<String>,
    pub asr_kind: Option<String>,
    pub asr_model: Option<String>,
    pub retranscribed_at: Option<i64>,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub date: String,
    pub transcript: String,
    pub summary: String,
    pub participants: Vec<String>,
    pub recording_duration: i32,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingDto {
    pub id: String,
    pub title: String,
    pub date: String,
    pub transcript: String,
    pub summary: String,
    pub participants: Vec<String>,
    pub recording_duration: i32,
    pub status: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TranscriptSegmentRow {
    pub id: String,
    pub meeting_id: String,
    #[sqlx(rename = "index")]
    pub index: i32,
    pub text: String,
    pub is_final: i32,
    pub translated: Option<String>,
    pub speaker_id: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub index: i32,
    pub text: String,
    pub is_final: bool,
    pub translated: Option<String>,
    pub speaker_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegmentDto {
    pub index: i32,
    pub text: String,
    pub is_final: bool,
    pub translated: Option<String>,
    pub speaker_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AsrConfigRow {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub api_key_enc: Option<String>,
    pub model: Option<String>,
    pub lang: Option<String>,
    pub is_default: i32,
    pub model_path: Option<String>,
    pub extra: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfig {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    /// 明文 API key（服务层内部传递；落库前由 service 加密）
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub lang: Option<String>,
    pub is_default: bool,
    pub model_path: Option<String>,
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfigDto {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub lang: Option<String>,
    pub is_default: bool,
    pub model_path: Option<String>,
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrConfigInput {
    pub name: String,
    pub kind: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub lang: Option<String>,
    pub is_default: bool,
    /// 本地模型路径（sherpa/vosk 等本地后端，任意目录，不限定内置清单）
    pub model_path: Option<String>,
    /// 额外参数（任意 JSON，自定义后端扩展用）
    pub extra: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrBackendInfo {
    pub kind: String,
    pub name: String,
    pub description: String,
    pub languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrBackendInfoDto {
    pub kind: String,
    pub name: String,
    pub description: String,
    pub languages: Vec<String>,
}
