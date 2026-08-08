import { invoke, listen } from '$lib/api/client';

// ── Types ──────────────────────────────────────────────────

export interface OrchestratorSession {
	id: string;
	user_request: string;
	spec: SpecDocument | null;
	plan: ExecutionPlan | null;
	status: string;
	cycle_count: number;
	max_cycles: number;
	history: OrchestratorEvent[];
	created_at: number;
	updated_at: number;
}

export interface SpecDocument {
	id: string;
	summary: string;
	tasks: SpecTask[];
	acceptance_criteria: Record<string, string[]>;
	dependencies: Record<string, string[]>;
	out_of_scope: string[];
}

export interface SpecTask {
	id: string;
	title: string;
	description: string;
	acceptance: string[];
	estimated_complexity: string;
	required_tools: string[];
	suggested_model: string | null;
}

export interface ExecutionPlan {
	groups: ExecutionGroup[];
	total_tasks: number;
	estimated_tokens: number | null;
}

export interface ExecutionGroup {
	id: string;
	kind: string;
	tasks: PlannedTask[];
}

export interface PlannedTask {
	spec_task_id: string;
	agent_config: AgentConfig;
	prompt: string;
	tools: string[];
	timeout_secs: number | null;
}

export interface AgentConfig {
	role: string;
	model_provider: string;
	model_id: string;
	system_prompt: string | null;
	temperature: number | null;
	max_tokens: number | null;
}

export interface OrchestratorEvent {
	event_type: string;
	message: string;
	timestamp: number;
	data: any;
}

// ── Store ──────────────────────────────────────────────────

function createOrchestratorStore() {
	let session = $state<OrchestratorSession | null>(null);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let events = $state<OrchestratorEvent[]>([]);
	let listenerAttached = false;

	async function startSession(userRequest: string): Promise<OrchestratorSession | null> {
		loading = true;
		error = null;
		try {
			session = await invoke<OrchestratorSession>('orchestrator_start', { userRequest });
			events = [];
			attachListeners();
			return session;
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
			return null;
		} finally {
			loading = false;
		}
	}

	function attachListeners() {
		if (listenerAttached) return;
		listenerAttached = true;

		listen('orchestrator:event', (event: any) => {
			const payload = event.payload as OrchestratorEvent;
			events = [payload, ...events].slice(0, 200);

			// Update session status based on events
			if (session && payload.event_type) {
				switch (payload.event_type) {
					case 'spec_generated':
						session.status = 'plan_generating';
						break;
					case 'plan_generated':
						session.status = 'executing';
						break;
					case 'execution_completed':
						session.status = 'reviewing';
						break;
					case 'review_passed':
						session.status = 'completed';
						break;
					case 'review_failed':
						session.status = 'repairing';
						break;
					case 'budget_exhausted':
						session.status = 'budget_exhausted';
						break;
				}
				session.updated_at = Date.now();
			}
		}).catch(() => {});
	}

	function reset() {
		session = null;
		events = [];
		error = null;
	}

	return {
		get session() { return session; },
		get events() { return events; },
		get loading() { return loading; },
		get error() { return error; },
		startSession,
		reset,
	};
}

export const orchestratorStore = createOrchestratorStore();
