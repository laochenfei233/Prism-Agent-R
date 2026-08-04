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
	list: () => invoke<AgentDto[]>('agent-list'),
	get: (id: string) => invoke<AgentDto>('agent-get', { id }),
	create: (name: string, description?: string, system_prompt?: string) =>
		invoke<AgentDto>('agent-create', { name, description, system_prompt }),
	update: (id: string, data: Partial<AgentDto>) =>
		invoke<AgentDto>('agent-update', { id, ...data }),
	delete: (id: string) => invoke<void>('agent-delete', { id }),
};

// ── Session API ───────────────────────────────────────────

export const sessionApi = {
	list: (agentId?: string) => invoke<SessionDto[]>('session-list', { agent_id: agentId }),
	create: (agentId: string, title?: string) =>
		invoke<SessionDto>('session-create', { agent_id: agentId, title }),
	rename: (id: string, title: string) =>
		invoke<SessionDto>('session-rename', { id, title }),
	delete: (id: string) => invoke<void>('session-delete', { id }),
};

// ── Chat API ──────────────────────────────────────────────

export const chatApi = {
	history: (sessionId: string, limit?: number) =>
		invoke<MessageDto[]>('chat-history', { session_id: sessionId, limit }),
	send: (sessionId: string, content: string) =>
		invoke<MessageDto>('chat-send', { session_id: sessionId, content }),
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
