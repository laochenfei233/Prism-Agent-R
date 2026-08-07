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
};

// ── Chat API ──────────────────────────────────────────────

export const chatApi = {
	history: (sessionId: string, limit?: number) =>
		invoke<MessageDto[]>('chat_history', { sessionId, limit }),
	send: (sessionId: string, content: string) =>
		invoke<MessageDto>('chat_send', { sessionId, content }),
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

export const skillApi = {
	list: () => invoke<SkillDto[]>('skill_list'),
	install: (source: string, sourceUrl?: string) =>
		invoke<InstalledSkill>('skill_install', { source, sourceUrl }),
	uninstall: (id: string) => invoke<void>('skill_uninstall', { id }),
	toggle: (agentId: string, skillId: string, enabled: boolean) =>
		invoke<void>('skill_toggle', { agentId, skillId, enabled }),
	searchMarket: (query: string) => invoke<unknown[]>('skill_search_market', { query }),
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
};
