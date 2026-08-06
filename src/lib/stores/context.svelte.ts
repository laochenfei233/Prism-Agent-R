import { invoke } from '$lib/api/client';

export interface AgentContext {
	agent: any;
	session_usage: SessionUsage;
	workspace: WorkspaceInfo;
	instructions: InstructionFile[];
	mcp: McpServerStatus[];
	lsp: LspServerInfo[];
	tree: DirTree;
}

export interface SessionUsage {
	input_tokens: number;
	output_tokens: number;
	context_used: number;
	context_limit: number;
	tool_calls: number;
	cost_est: number;
	today_calls: number;
	today_tokens: number;
	today_cost: number;
}

export interface WorkspaceInfo {
	current_dir: string;
	recent_dirs: string[];
	bound_agent_id: string | null;
}

export interface InstructionFile {
	path: string;
	name: string;
	lines: number;
	injected: boolean;
	priority: number;
}

export interface McpServerStatus {
	id: string;
	name: string;
	status: string;
	tools_count: number;
	last_error: string | null;
}

export interface LspServerInfo {
	id: string;
	cmd: string;
	status: string;
	langs: string[];
	index_file_count: number | null;
	last_error: string | null;
	install_hint: string | null;
}

export interface DirTree {
	name: string;
	path: string;
	is_dir: boolean;
	children: DirTree[] | null;
	language: string | null;
	line_count: number | null;
}

function createContextStore() {
	let context = $state<AgentContext | null>(null);
	let loading = $state(false);
	let activeTab = $state('usage');
	let sidebarWidth = $state(320);
	let collapsed = $state(false);

	async function loadContext(agentId: string, sessionId?: string) {
		loading = true;
		try {
			context = await invoke<AgentContext>('context_agent', {
				agentId,
				sessionId: sessionId || null
			});
		} catch (e) {
			console.error('Failed to load context:', e);
		} finally {
			loading = false;
		}
	}

	function toggleCollapse() {
		collapsed = !collapsed;
	}

	return {
		get context() {
			return context;
		},
		get loading() {
			return loading;
		},
		get activeTab() {
			return activeTab;
		},
		set activeTab(v: string) {
			activeTab = v;
		},
		get sidebarWidth() {
			return sidebarWidth;
		},
		set sidebarWidth(v: number) {
			sidebarWidth = Math.max(280, Math.min(480, v));
		},
		get collapsed() {
			return collapsed;
		},
		loadContext,
		toggleCollapse
	};
}

export const contextStore = createContextStore();
