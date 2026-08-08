import { invoke, listen } from '$lib/api/client';

// ── Types ──────────────────────────────────────────────────

export interface BudgetStatus {
	daily_tokens_used: number;
	daily_tokens_limit: number;
	daily_cost_used: number;
	daily_cost_limit: number;
	monthly_cost_used: number;
	monthly_cost_limit: number;
	active_workflows: number;
}

export interface AgentException {
	id: string;
	session_id: string;
	agent_id: string;
	workflow_id: string | null;
	run_id: string | null;
	stage_id: string | null;
	exception_type: string;
	severity: string;
	message: string;
	context: string | null;
	tool_name: string | null;
	model_id: string | null;
	tokens_used: number | null;
	cost_used: number | null;
	created_at: number;
	resolved_at: number | null;
	resolved_by: string | null;
	resolution: string | null;
}

export interface GuardrailDecision {
	type: 'Allow' | 'Deny' | 'NeedApproval';
	reason?: string;
	tool?: string;
}

// ── Store ──────────────────────────────────────────────────

function createMonitorStore() {
	let budget = $state<BudgetStatus | null>(null);
	let exceptions = $state<AgentException[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let listenerAttached = false;

	async function loadBudget() {
		try {
			budget = await invoke<BudgetStatus>('monitor_get_budget');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function loadExceptions(limit?: number) {
		try {
			exceptions = await invoke<AgentException[]>('monitor_get_exceptions', { limit: limit ?? 20 });
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function refresh() {
		loading = true;
		await Promise.all([loadBudget(), loadExceptions()]);
		loading = false;
	}

	function attachListeners() {
		if (listenerAttached) return;
		listenerAttached = true;

		listen('budget:warning', (event: any) => {
			if (budget) {
				const payload = event.payload;
				if (payload.entity_type === 'global') {
					if (payload.level === 'daily_tokens') {
						budget.daily_tokens_used = payload.current;
					}
				}
			}
		}).catch(() => {});

		listen('budget:exceeded', () => {
			void loadBudget();
		}).catch(() => {});

		listen('monitor:exception', (event: any) => {
			exceptions = [event.payload, ...exceptions].slice(0, 100);
		}).catch(() => {});
	}

	attachListeners();

	// §26.3 轮询备用（事件系统不可用时保底刷新）
	let polling = false;
	function startPolling(intervalMs = 10000) {
		if (polling) return;
		polling = true;
		setInterval(() => refresh(), intervalMs);
	}

	async function clearResolvedExceptions() {
		try {
			await invoke('exception_clear');
			await loadExceptions(50);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		}
	}

	async function exportLog(): Promise<string | null> {
		try {
			return await invoke<string>('log_export');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			return null;
		}
	}

	return {
		get budget() { return budget; },
		get exceptions() { return exceptions; },
		get loading() { return loading; },
		get error() { return error; },
		loadBudget,
		loadExceptions,
		refresh,
		startPolling,
		clearResolvedExceptions,
		exportLog,
	};
}

export const monitorStore = createMonitorStore();
