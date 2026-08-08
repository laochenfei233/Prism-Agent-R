import { invoke, listen } from './client';

// ── Types ─────────────────────────────────────────────────

export interface AgentDto {
	id: string;
	name: string;
	description: string | null;
	avatar: string | null;
	system_prompt: string | null;
	model_id: string | null;
	temperature: number | null;
	max_tokens: number | null;
	disabled_tools: string[];
	order_key: number;
}

export interface SessionDto {
	id: string;
	agent_id: string;
	title: string | null;
	pinned: boolean;
	created_at: number;
	updated_at: number;
}

export interface MessageDto {
	id: string;
	session_id: string;
	role: string;
	content: string;
	tool_calls: unknown[] | null;
	tool_call_id: string | null;
	model_id: string | null;
	usage: unknown | null;
	created_at: number;
}

// ── Agent API ─────────────────────────────────────────────

export const agentApi = {
	list: () => invoke<AgentDto[]>('agent_list'),
	get: (id: string) => invoke<AgentDto>('agent_get', { id }),
	create: (name: string, description?: string, system_prompt?: string) =>
		invoke<AgentDto>('agent_create', { name, description, system_prompt }),
	update: (id: string, data: Partial<AgentDto>) =>
		invoke<AgentDto>('agent_update', { id, ...data }),
	delete: (id: string) => invoke<void>('agent_delete', { id }),
};

// ── Session API ───────────────────────────────────────────

export const sessionApi = {
	list: (agentId?: string) => invoke<SessionDto[]>('session_list', { agentId }),
	create: (agentId: string, title?: string) =>
		invoke<SessionDto>('session_create', { agentId, title }),
	rename: (id: string, title: string) =>
		invoke<SessionDto>('session_rename', { id, title }),
	delete: (id: string) => invoke<void>('session_delete', { id }),
	search: (query: string, limit?: number) =>
		invoke<SessionDto[]>('session_search', { query, limit }),
	injectFile: (sessionId: string, path: string) =>
		invoke<void>('session_inject_file', { sessionId, path }),
};

// ── Chat API ──────────────────────────────────────────────

export const chatApi = {
	history: (sessionId: string, limit?: number) =>
		invoke<MessageDto[]>('chat_history', { sessionId, limit }),
	send: (sessionId: string, content: string, attachments?: string[]) =>
		invoke<MessageDto>('chat_send', { sessionId, content, attachments }),
};

// ── Stream Events ─────────────────────────────────────────

export interface StreamDelta {
	session_id: string;
	message_id: string;
	delta: string;
}

export interface StreamToolCall {
	session_id: string;
	message_id: string;
	call: { id: string; name: string; arguments: unknown };
}

export interface StreamDone {
	session_id: string;
	message_id: string;
	usage: unknown;
}

export interface StreamError {
	session_id: string;
	message_id: string;
	message: string;
}

export const streamEvents = {
	onDelta: (sessionId: string, handler: (delta: string) => void) =>
		listen<StreamDelta>('chat:stream:delta', (e) => {
			if (e.session_id === sessionId) handler(e.delta);
		}),
	onToolCall: (sessionId: string, handler: (call: StreamToolCall['call']) => void) =>
		listen<StreamToolCall>('chat:stream:tool_call', (e) => {
			if (e.session_id === sessionId) handler(e.call);
		}),
	onDone: (sessionId: string, handler: () => void) =>
		listen<StreamDone>('chat:stream:done', (e) => {
			if (e.session_id === sessionId) handler();
		}),
	onError: (sessionId: string, handler: (message: string) => void) =>
		listen<StreamError>('chat:stream:error', (e) => {
			if (e.session_id === sessionId) handler(e.message);
		}),
};

// ── MCP API ─────────────────────────────────────────────

export interface McpServerDto {
	id: string;
	name: string;
	type: string;
	command: string | null;
	args: string[];
	base_url: string | null;
	is_active: boolean;
	timeout_ms: number | null;
}

export interface McpTool {
	name: string;
	description: string;
	input_schema: unknown;
}

export interface McpTestResult {
	ok: boolean;
	tools_count: number;
	latency_ms: number | null;
	error: string | null;
}

export interface ServerStatusInfo {
	id: string;
	name: string;
	type: string;
	status: string;
	tools_count: number;
	tools: string[];
}

export const mcpApi = {
	list: () => invoke<McpServerDto[]>('mcp_list'),
	add: (data: {
		name: string;
		type: string;
		command?: string;
		args?: string[];
		env?: Record<string, string>;
		base_url?: string;
		headers?: Record<string, string>;
		timeout_ms?: number;
	}) => invoke<McpServerDto>('mcp_add', data),
	update: (id: string, data: Partial<McpServerDto>) =>
		invoke<McpServerDto>('mcp_update', { id, ...data }),
	remove: (id: string) => invoke<void>('mcp_remove', { id }),
	test: (id: string) => invoke<McpTestResult>('mcp_test', { id }),
	tools: (serverId?: string) => invoke<McpTool[]>('mcp_tools', { serverId }),
	statusAll: () => invoke<ServerStatusInfo[]>('mcp_status_all'),
	callTool: (serverId: string, toolName: string, args: unknown) =>
		invoke<unknown>('mcp_call_tool', { serverId, toolName, arguments: args }),
};

// ── Skill API ───────────────────────────────────────────

export interface SkillDto {
	id: string;
	name: string;
	description: string | null;
	folder_name: string;
	source: string;
	is_enabled: boolean;
}

export interface InstalledSkill {
	id: string;
	name: string;
	folder_name: string;
	source: string;
	is_enabled: boolean;
}

export interface LocalSkill {
	name: string;
	description: string | null;
	path: string;
}

export const skillApi = {
	list: () => invoke<SkillDto[]>('skill_list'),
	install: (source: string, sourceUrl?: string) =>
		invoke<InstalledSkill>('skill_install', { source, sourceUrl }),
	uninstall: (id: string) => invoke<void>('skill_uninstall', { id }),
	toggle: (agentId: string, skillId: string, enabled: boolean) =>
		invoke<void>('skill_toggle', { agentId, skillId, enabled }),
	searchMarket: (query: string) => invoke<unknown[]>('skill_search_market', { query }),
	listLocal: (workdir: string) => invoke<LocalSkill[]>('skill_list_local', { workdir }),
};

// ── Workflow API ────────────────────────────────────────

export interface WorkflowDto {
	id: string;
	name: string;
	description: string | null;
	definition: unknown;
}

export const workflowApi = {
	list: () => invoke<WorkflowDto[]>('workflow_list'),
	run: (workflowId: string, inputs: Record<string, unknown>) =>
		invoke<{ run_id: string }>('workflow_run', { workflowId, inputs }),
	rerun: (runId: string, inputs?: Record<string, unknown>) =>
		invoke<{ run_id: string; status: string }>('task_rerun', { runId, inputs: inputs || null }),
	stop: (runId: string) => invoke<void>('workflow_stop', { runId }),
	result: (runId: string) =>
		invoke<{ run_id: string; status: string; outputs: Record<string, string>; error: string | null }>('workflow_result', { runId }),
};

// ── Memory API ──────────────────────────────────────────

export interface MemorySearchHit {
	path: string;
	scope: string;
	snippet: string;
	score: number;
}

export interface MemoryDump {
	path: string;
	scope: string;
	size: number;
	preview: string;
}

export const memoryApi = {
	search: (query: string, scope?: string, limit?: number) =>
		invoke<MemorySearchHit[]>('memory_search', { query, scope, limit }),
	read: (path: string) => invoke<string>('memory_read', { path }),
	write: (path: string, content: string) =>
		invoke<void>('memory_write', { path, content }),
	contextDump: () => invoke<MemoryDump[]>('memory_context_dump'),
	reconcile: () => invoke<number>('memory_reconcile'),
};

// ── File API ────────────────────────────────────────────

export interface FileEntry {
	path: string;
	name: string;
	is_dir: boolean;
	size: number;
}

export const fileApi = {
	pick: (path?: string) => invoke<string>('file_pick', { path }),
	readText: (path: string) => invoke<string>('file_read_text', { path }),
	write: (path: string, content: string) => invoke<void>('file_write', { path, content }),
	list: (path: string, depth?: number) => invoke<FileEntry[]>('file_list', { path, depth }),
	parse: (path: string) => invoke<any>('file_parse', { path }),
};

// ── Settings API ────────────────────────────────────────

export const settingsApi = {
	saveProviderKey: (providerId: string, apiKey: string) =>
		invoke<void>('settings_save_provider_key', { providerId, apiKey }),
};

// ── Wiki API ─────────────────────────────────────────────

export interface WikiDto {
	id: string;
	name: string;
	description: string | null;
	created_at: number;
	updated_at: number;
}

export interface WikiPageDto {
	path: string;
	title: string;
	size: number;
}

export interface WikiPageHitDto {
	path: string;
	title: string;
	snippet: string;
	score: number;
}

export const wikiApi = {
	create: (name: string, description?: string) =>
		invoke<WikiDto>('wiki_create', { name, description }),
	list: () => invoke<WikiDto[]>('wiki_list'),
	get: (id: string) => invoke<WikiDto>('wiki_get', { id }),
	delete: (id: string) => invoke<void>('wiki_delete', { id }),
	readPage: (wikiId: string, path: string) =>
		invoke<string>('wiki_read_page', { wikiId, path }),
	writePage: (wikiId: string, path: string, content: string) =>
		invoke<void>('wiki_write_page', { wikiId, path, content }),
	listPages: (wikiId: string) => invoke<WikiPageDto[]>('wiki_list_pages', { wikiId }),
	search: (wikiId: string, query: string) =>
		invoke<WikiPageHitDto[]>('wiki_search', { wikiId, query }),
	writeAi: (wikiId: string, info: string, preview = true) =>
		invoke<{ plan: WikiWritePlan }>('wiki_write_ai', { wikiId, info, preview }),
	applyPlan: (wikiId: string, plan: WikiWritePlan) =>
		invoke<WikiWriteResult>('wiki_apply_plan', { wikiId, plan }),
};

export interface WikiOp {
	op: 'create_page' | 'update_page' | 'delete_page' | 'update_index' | 'noop';
	path?: string;
	title?: string;
	content?: string;
	summary?: string;
	reason?: string;
	entries?: string[];
}

export interface WikiWritePlan {
	operations: WikiOp[];
}

export interface WikiWriteResult {
	applied: number;
	noop: number;
	summary: string;
	log_appended: boolean;
}

// ── RAG API ─────────────────────────────────────────────

export interface RagDocumentDto {
	id: string;
	name: string;
	mime_type: string;
	size: number;
	chunk_count: number;
	status: string;
}

export interface RagHitDto {
	chunk_id: string;
	document_title: string;
	page_start: number | null;
	page_end: number | null;
	section: string | null;
	quote: string;
	score: number;
}

export interface IngestResultDto {
	document_id: string;
	chunk_count: number;
	status: string;
}

export const ragApi = {
	ingest: (wikiId: string, filePath: string) =>
		invoke<IngestResultDto>('rag_ingest', { wikiId, filePath }),
	search: (wikiId: string, query: string, topK?: number) =>
		invoke<RagHitDto[]>('rag_search', { wikiId, query, topK }),
	listDocuments: (wikiId: string) =>
		invoke<RagDocumentDto[]>('rag_list_documents', { wikiId }),
	deleteDocument: (docId: string) =>
		invoke<void>('rag_delete_document', { docId }),
	embeddingConfig: (mode: 'local' | 'api', providerId?: string, model?: string, dim?: number) =>
		invoke<EmbeddingStatusDto>('rag_embedding_config', { mode, providerId, model, dim }),
	embeddingStatus: () =>
		invoke<EmbeddingStatusDto>('rag_embedding_status'),
	contextualConfig: (enabled: boolean) =>
		invoke<{ enabled: boolean }>('rag_contextual_config', { enabled }),
	contextualStatus: () =>
		invoke<{ enabled: boolean }>('rag_contextual_status'),
	rerankConfig: (enabled: boolean) =>
		invoke<{ enabled: boolean }>('rag_rerank_config', { enabled }),
	rerankStatus: () =>
		invoke<{ enabled: boolean }>('rag_rerank_status'),
	eval: (wikiId?: string, suite?: string, topK?: number) =>
		invoke<EvalReportDto>('rag_eval', { wikiId, suite, topK }),
	evalAdd: (case_: Record<string, unknown>) =>
		invoke<string>('rag_eval_add', { case: case_ }),
	evalReport: () =>
		invoke<EvalReportDto[]>('rag_eval_report'),
};

export interface EvalMetricsDto {
	recall_at_k: number;
	page_acc: number;
	table_acc: number;
	ocr_completeness: number;
	chart_acc: number;
}

export interface EvalReportDto {
	suite: string;
	case_count: number;
	metrics: EvalMetricsDto;
	cases: Array<{ id: string; question: string; passed: boolean; hit_count: number; detail: string }>;
	created_at: number;
}

export interface EmbeddingStatusDto {
	mode: string;
	kind: string;
	provider_id: string | null;
	model: string | null;
	dim: number;
	is_local: boolean;
	base_dir: string;
}

// ── Meeting API ──────────────────────────────────────────

export interface MeetingDto {
	id: string;
	title: string;
	date: string;
	transcript: string;
	summary: string;
	participants: string[];
	recording_duration: number;
	created_at: number;
	updated_at: number;
}

export interface TranscriptSegmentDto {
	index: number;
	text: string;
	is_final: boolean;
	translated: string | null;
	speaker_id: number | null;
}

export interface AsrConfigDto {
	id: string;
	name: string;
	kind: string;
	base_url: string | null;
	model: string | null;
	lang: string | null;
	is_default: boolean;
	model_path: string | null;
	extra: Record<string, unknown> | null;
}

export interface AsrBackendInfoDto {
	kind: string;
	name: string;
	description: string;
	languages: string[];
}

export interface AsrModelInfoDto {
	id: string;
	name: string;
	backend: string;
	size_mb: number;
	lang: string[];
	url: string;
	requires_vad: boolean;
	user_placed: boolean;
}

export interface InstalledAsrModelDto {
	id: string;
	path: string;
	size_mb: number;
	backend: string;
	lang: string[];
}

export const meetingApi = {
	create: (title: string, participants?: string[]) =>
		invoke<MeetingDto>('meeting_create', { title, participants }),
	list: () => invoke<MeetingDto[]>('meeting_list'),
	get: (id: string) => invoke<MeetingDto>('meeting_get', { id }),
	delete: (id: string) => invoke<void>('meeting_delete', { id }),
	updateTranscript: (id: string, segments: TranscriptSegmentDto[]) =>
		invoke<void>('meeting_update_transcript', { id, segments }),
	getTranscript: (id: string) =>
		invoke<TranscriptSegmentDto[]>('meeting_get_transcript', { id }),
	summary: (id: string) => invoke<string>('meeting_summary', { id }),
	clean: (id: string) => invoke<string>('meeting_clean', { id }),
	qa: (id: string, question: string) =>
		invoke<string>('meeting_qa', { id, question }),
	pushToAgent: (meetingId: string, agentId: string, sessionId?: string) =>
		invoke<string>('meeting_push_to_agent', { meetingId, agentId, sessionId }),
	export: (id: string, format: string, includeSummary = true, includeTranslation = false) =>
		invoke<string>('meeting_export', { id, format, includeSummary, includeTranslation }),
	exportTranslation: (id: string, targetLang: string) =>
		invoke<string>('meeting_export_translation', { id, targetLang }),
};

export const asrApi = {
	listConfigs: () => invoke<AsrConfigDto[]>('asr_list_configs'),
	saveConfig: (config: AsrConfigInputDto) =>
		invoke<AsrConfigDto>('asr_save_config', { config }),
	deleteConfig: (id: string) => invoke<void>('asr_delete_config', { id }),
	backends: () => invoke<AsrBackendInfoDto[]>('asr_backends'),
	modelCatalog: () => invoke<AsrModelInfoDto[]>('asr_model_catalog'),
	modelInstalled: () => invoke<InstalledAsrModelDto[]>('asr_model_installed'),
	modelDownload: (modelId: string) =>
		invoke<{ model_id: string; path: string; status: string }>('asr_model_download', { modelId }),
	modelRemove: (modelId: string) => invoke<void>('asr_model_remove', { modelId }),
	test: (config: AsrConfigInputDto) =>
		invoke<{ ok: boolean; latency_ms: number; error: string | null }>('asr_test', { config }),
	startRecording: (id: string, asrConfig?: AsrConfigInputDto) =>
		invoke<void>('meeting_start_recording', { id, asrConfig }),
	audioChunk: (meetingId: string, pcmBase64: string) =>
		invoke<void>('meeting_audio_chunk', { meetingId, pcmBase64 }),
	stopRecording: (id: string) =>
		invoke<{ transcript: string }>('meeting_stop_recording', { id }),
};

export interface AsrConfigInputDto {
	name: string;
	kind: string;
	base_url?: string;
	api_key?: string;
	model?: string;
	lang?: string;
	is_default: boolean;
	model_path?: string;
	extra?: Record<string, unknown>;
}

// ── Translate API ────────────────────────────────────────

export interface TranslateResultDto {
	translated: string;
	source_lang: string;
	from_cache: boolean;
}

export interface TranslateHistoryDto {
	id: string;
	source_text: string;
	source_lang: string;
	target_lang: string;
	translated: string;
	created_at: number;
}

export interface GlossaryTermDto {
	id: string;
	source_lang: string;
	target_lang: string;
	source_term: string;
	target_term: string;
	category: string | null;
	enabled: boolean;
}

export interface OcrBlockDto {
	text: string;
	bbox: [number, number, number, number];
	confidence: number;
	kind: string;
}

export interface OcrResultDto {
	text: string;
	lang: string;
	provider: string;
	blocks: OcrBlockDto[];
}

export const translateApi = {
	translate: (text: string, target: string, source?: string, modelId?: string) =>
		invoke<TranslateResultDto>('translate_translate', { text, target, source, modelId }),
	batch: (texts: string[], target: string, source?: string) =>
		invoke<TranslateResultDto[]>('translate_batch', { texts, target, source }),
	file: (path: string, target: string, source?: string) =>
		invoke<string>('translate_file', { path, target, source }),
	history: (query?: string, limit?: number, offset?: number) =>
		invoke<{ items: TranslateHistoryDto[]; total: number }>('translate_history', { query, limit, offset }),
	detect: (text: string) =>
		invoke<{ lang: string; confidence: number }>('translate_detect', { text }),
	modelConfig: (modelId?: string) =>
		invoke<{ model_id: string | null }>('translate_model_config', { modelId }),
	modelStatus: () =>
		invoke<{ model_id: string | null }>('translate_model_status'),
};

export const glossaryApi = {
	list: (langPair?: string) =>
		invoke<GlossaryTermDto[]>('glossary_list', { langPair }),
	add: (term: Omit<GlossaryTermDto, 'id' | 'enabled'>) =>
		invoke<void>('glossary_add', { term }),
	remove: (id: string) => invoke<void>('glossary_remove', { id }),
	importCsv: (path: string) =>
		invoke<{ imported: number; failed: number }>('glossary_import_csv', { path }),
};

export const ocrApi = {
	recognize: (imagePath: string, lang?: string) =>
		invoke<OcrResultDto>('ocr_recognize', { imagePath, lang }),
	providers: () =>
		invoke<{ name: string; kind: string; available: boolean }[]>('ocr_providers'),
};

// ── Trace API（Agent 执行轨迹） ────────────────────────────

export interface TraceStepDto {
	step_index: number;
	kind: string;
	input_summary: string;
	output_summary: string;
	latency_ms: number;
	tool_name: string | null;
	error: string | null;
}

export interface AgentTraceDto {
	id: string;
	session_id: string;
	agent_id: string;
	trace_id: string;
	started_at: number;
	finished_at: number | null;
	steps: TraceStepDto[];
	total_prompt_tokens: number;
	total_completion_tokens: number;
	total_cost: number;
	outcome: string;
}

// ── Router API（Skill/MCP 路由调试） ──────────────────────

export interface RouteItemDto {
	id: string;
	kind: 'Skill' | 'McpTool';
	name: string;
	description: string;
	keywords: string[];
	server_id: string | null;
}

export interface RouteResultDto {
	skills: RouteItemDto[];
	tools: RouteItemDto[];
	semantic_used: boolean;
}

export const routerApi = {
	route: (query: string, topK?: number) =>
		invoke<RouteResultDto>('router_route', { query, topK }),
	indexStatus: () =>
		invoke<{ skills: number; mcp_tools: number; updated_at: number }>('router_index_status'),
};

// ── 项目级自动索引 API（§10.2.1） ─────────────────────────

export interface ProjectIndexStatusDto {
	enabled: boolean;
	workdir: string | null;
	indexed_files: number;
	in_progress: boolean;
	last_indexed_at: number | null;
}

export const projectIndexApi = {
	status: () => invoke<ProjectIndexStatusDto>('project_index_status'),
	toggle: (enabled: boolean) =>
		invoke<ProjectIndexStatusDto>('project_index_toggle', { enabled }),
	reindex: () => invoke<ProjectIndexStatusDto>('project_index_reindex'),
};

// ── TTS 播报 API（§10.3.9） ───────────────────────────────

export interface TtsSpeakResultDto {
	backend: string;
	segments: string[];
	lang: string;
	rate: number;
}

export interface TtsVoiceInfoDto {
	backend: string;
	available: boolean;
	lang: string | null;
	rate: number;
}

export const ttsApi = {
	speak: (text: string, lang?: string, rate?: number) =>
		invoke<TtsSpeakResultDto>('tts_speak', { text, lang, rate }),
	stop: () => invoke<void>('tts_stop'),
	voices: () => invoke<TtsVoiceInfoDto>('tts_voices'),
};

// ── Search API (§15) ─────────────────────────────────────

export interface SearchConfigResult {
	provider: string;
	api_key_set: boolean;
	searxng_url: string | null;
	fallback_provider: string | null;
}

export interface SearchTestResult {
	success: boolean;
	provider: string;
	first_result_title: string | null;
	first_result_url: string | null;
	elapsed_ms: number;
	error: string | null;
}

export const searchApi = {
	config: () => invoke<SearchConfigResult>('search_config'),
	saveConfig: (data: { provider?: string; api_key?: string; searxng_url?: string; fallback_provider?: string }) =>
		invoke<void>('search_config_save', data),
	test: () => invoke<SearchTestResult>('search_test'),
};

// ── Session Lifecycle API (§17.1) ────────────────────────

export type SessionLifecycle = 'Created' | 'Init' | 'Ready' | 'Running' | 'Paused' | 'Verifying' | 'Done' | 'InitFailed';

export interface SessionInitReport {
	provider_ok: boolean;
	provider_error: string | null;
	memory_ok: boolean;
	memory_error: string | null;
	mcp_ok: boolean;
	mcp_error: string | null;
}

export const sessionLifecycleApi = {
	init: (sessionId: string) =>
		invoke<SessionInitReport>('session_init', { sessionId }),
	state: (sessionId: string) =>
		invoke<SessionLifecycle>('session_state_query', { sessionId }),
	cleanup: (sessionId: string) =>
		invoke<void>('session_cleanup', { sessionId }),
	fork: (sessionId: string, turnId: string) =>
		invoke<SessionDto>('session_fork', { sessionId, turnId }),
	approve: (callId: string, decision: string, alwaysAllow?: boolean) =>
		invoke<boolean>('session_approve', { callId, decision, alwaysAllow }),
};

// ── Loop API (§17.2) ────────────────────────────────────

export type LoopKind = 'Goal' | 'Timer' | 'MakerChecker';
export type LoopStatus = 'Idle' | 'Running' | 'Paused' | 'Completed' | 'Failed';

export interface AgentLoop {
	id: string;
	kind: LoopKind;
	interval_secs: number | null;
	max_rounds: number;
	goal: unknown | null;
	maker_workflow_id: string | null;
	checker_workflow_id: string | null;
	status: LoopStatus;
	current_round: number;
}

export interface LoopCreateRequest {
	kind: LoopKind;
	interval_secs?: number;
	max_rounds?: number;
	goal?: unknown;
	maker_workflow_id?: string;
	checker_workflow_id?: string;
}

export const loopApi = {
	start: (request: LoopCreateRequest) =>
		invoke<AgentLoop>('loop_start', { request }),
	stop: (loopId: string) =>
		invoke<boolean>('loop_stop', { loopId }),
	list: () => invoke<AgentLoop[]>('loop_list'),
};

// ── Trace API (§17.3) ───────────────────────────────────

export interface TraceStep {
	step_index: number;
	kind: string;
	input_summary: string;
	output_summary: string;
	latency_ms: number;
	tool_name: string | null;
	error: string | null;
}

export interface AgentTrace {
	id: string;
	session_id: string;
	agent_id: string;
	trace_id: string;
	started_at: number;
	finished_at: number | null;
	steps: TraceStep[];
	total_prompt_tokens: number;
	total_completion_tokens: number;
	total_cost: number;
	outcome: string;
	grade_score: number | null;
	grade_reason: string | null;
	graded_at: number | null;
}

export const traceApi = {
	list: (sessionId: string, limit?: number, minGrade?: number, toolFailed?: boolean) =>
		invoke<AgentTrace[]>('trace_list', { sessionId, limit, minGrade, toolFailed }),
	grade: (traceId: string, score: number, reason: string) =>
		invoke<void>('trace_grade', { traceId, score, reason }),
};
