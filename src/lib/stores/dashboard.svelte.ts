import { invoke, listen } from '$lib/api/client';

// ── Types (mirror backend DashboardOverview) ───────────────

export interface UsageStats {
	today_tokens: number;
	week_tokens: number;
	month_tokens: number;
	month_cost: number;
	today_calls: number;
}

export interface UsagePoint {
	date: string;
	tokens: number;
	cost: number;
}

export interface AgentSummary {
	id: string;
	name: string;
	description: string;
	avatar: string | null;
	model_name: string | null;
	skill_count: number;
	mcp_count: number;
	last_used: string | null;
	order_key: number;
}

export interface SkillOverview {
	enabled: number;
	total: number;
	popular: string[];
}

export interface McpServerStatus {
	id: string;
	name: string;
	status: string;
	tools_count: number;
	last_error: string | null;
}

export interface SessionSummary {
	id: string;
	title: string;
	agent_name: string;
	updated_at: string;
	message_count: number;
}

export interface ModelStatus {
	provider_name: string;
	model_id: string;
	display_name: string;
	status: string;
}

export interface DashboardOverview {
	agents: AgentSummary[];
	usage: UsageStats;
	usage_trend: UsagePoint[];
	skills: SkillOverview;
	mcp_servers: McpServerStatus[];
	recent_sessions: SessionSummary[];
	models: ModelStatus[];
}

// ── Store ──────────────────────────────────────────────────

function createDashboardStore() {
	let overview = $state<DashboardOverview | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let listenerAttached = false;
	let lastUsageRefresh = 0;
	const USAGE_REFRESH_THROTTLE_MS = 5000;

	async function loadOverview() {
		loading = true;
		error = null;
		try {
			overview = await invoke<DashboardOverview>('dashboard_overview');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			loading = false;
		}
	}

	// 消息完成时后端 emit usage:updated，节流刷新用量卡/趋势图
	function attachUsageListener() {
		if (listenerAttached) return;
		listenerAttached = true;
		listen('usage:updated', () => {
			const now = Date.now();
			if (now - lastUsageRefresh < USAGE_REFRESH_THROTTLE_MS) return;
			lastUsageRefresh = now;
			void loadOverview();
		}).catch(() => {
			// 非 Tauri 环境（如纯 web dev）下无事件系统，静默降级
			listenerAttached = false;
		});
	}

	attachUsageListener();

	return {
		get overview() { return overview; },
		get loading() { return loading; },
		get error() { return error; },
		loadOverview,
	};
}

export const dashboardStore = createDashboardStore();
