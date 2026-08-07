import { invoke, listen } from '$lib/api/client';

export interface TaskInput {
	key: string;
	label: string;
	kind: 'Text' | 'Textarea' | 'Select' | 'Number';
	options: any[] | null;
	default: any;
	required: boolean;
}

export interface TaskStageDef {
	id: string;
	name: string;
	role: string;
	agent_id: string | null;
	prompt_template: string;
	tools: string[];
	max_iterations: number;
	depends_on: string[];
	model_hint: string | null;
	output_spec: string | null;
}

export interface TaskDefinition {
	id: string;
	name: string;
	description: string;
	inputs: TaskInput[];
	stages: TaskStageDef[];
}

export interface TaskValidationResult {
	ok: boolean;
	errors: string[];
}

export interface TaskTemplateSummary {
	id: string;
	name: string;
	description: string;
	stage_count: number;
	input_count: number;
}

export interface TaskRunStatus {
	run_id: string;
	status: 'pending' | 'running' | 'completed' | 'failed';
	current_stage: string | null;
	stages_done: number;
	stages_total: number;
	outputs: Record<string, string> | null;
	error: string | null;
}

function createTaskStore() {
	let viewMode = $state<'template' | 'design' | 'run'>('template');
	let definition = $state<TaskDefinition | null>(null);
	let validation = $state<TaskValidationResult | null>(null);
	let runId = $state<string | null>(null);
	let templates = $state<TaskTemplateSummary[]>([]);
	let runStatus = $state<TaskRunStatus | null>(null);
	let templatesLoading = $state(false);
	let pollTimer: ReturnType<typeof setInterval> | null = null;

	// 监听 workflow:stage / workflow:done 事件，实时推进运行状态
	$effect(() => {
		const unsubStage = listen('workflow:stage', (event: { payload: any }) => {
			const payload = event.payload as { run_id?: string; stage_id?: string; status?: string };
			if (!payload || !runId || payload.run_id !== runId) return;
			if (runStatus) {
				runStatus = { ...runStatus, status: 'running', current_stage: payload.stage_id ?? runStatus.current_stage };
			}
		});

		const unsubDone = listen('workflow:done', (event: { payload: any }) => {
			const payload = event.payload as { run_id?: string; status?: string };
			if (!payload || !runId || payload.run_id !== runId) return;
			stopPolling();
			if (runStatus) {
				runStatus = { ...runStatus, status: payload.status === 'completed' ? 'completed' : 'failed' };
			}
			refreshRunStatus();
		});

		return () => {
			unsubStage.then((fn) => fn());
			unsubDone.then((fn) => fn());
			stopPolling();
		};
	});

	async function refreshRunStatus() {
		if (!runId) return;
		try {
			const result = await invoke<any>('workflow_result', { runId });
			const total = definition?.stages.length ?? 0;
			const done = result.status === 'completed' ? total : (runStatus?.stages_done ?? 0);
			runStatus = {
				run_id: result.run_id,
				status: result.status === 'running' || result.status === 'pending' ? 'running' : result.status,
				current_stage: runStatus?.current_stage ?? null,
				stages_done: done,
				stages_total: total,
				outputs: result.outputs ?? null,
				error: result.error ?? null,
			};
			if (result.status === 'completed' || result.status === 'failed' || result.status === 'cancelled') {
				stopPolling();
			}
		} catch {
			// 轮询失败忽略，下次重试
		}
	}

	function startPolling() {
		stopPolling();
		pollTimer = setInterval(refreshRunStatus, 2000);
	}

	function stopPolling() {
		if (pollTimer) {
			clearInterval(pollTimer);
			pollTimer = null;
		}
	}

	function newDefinition() {
		definition = {
			id: crypto.randomUUID(),
			name: '',
			description: '',
			inputs: [],
			stages: [
				{
					id: crypto.randomUUID(),
					name: '步骤 1',
					role: 'assistant',
					agent_id: null,
					prompt_template: '',
					tools: [],
					max_iterations: 5,
					depends_on: [],
					model_hint: null,
					output_spec: null,
				},
			],
		};
		validation = null;
		viewMode = 'design';
	}

	function loadTemplate(template: any) {
		definition = { ...template, id: crypto.randomUUID() };
		validation = null;
		viewMode = 'design';
	}

	async function validate(): Promise<boolean> {
		if (!definition) return false;
		try {
			validation = await invoke<TaskValidationResult>('task_validate', { definition });
			return validation.ok;
		} catch (e) {
			validation = { ok: false, errors: [e instanceof Error ? e.message : String(e)] };
			return false;
		}
	}

	async function saveTemplate() {
		if (!definition) return;
		await invoke('task_save_template', { definition });
		await loadTemplates();
	}

	async function loadTemplates() {
		templatesLoading = true;
		try {
			templates = await invoke<TaskTemplateSummary[]>('task_list_templates');
		} catch {
			templates = [];
		} finally {
			templatesLoading = false;
		}
	}

	async function startRun(inputs?: Record<string, any>) {
		if (!definition) return;
		try {
			const id = await invoke<string>('task_run', { definition, inputs: inputs || null });
			runId = id;
			runStatus = {
				run_id: id,
				status: 'running',
				current_stage: null,
				stages_done: 0,
				stages_total: definition.stages.length,
				outputs: null,
				error: null,
			};
			viewMode = 'run';
			refreshRunStatus();
			startPolling();
		} catch (e) {
			validation = { ok: false, errors: [e instanceof Error ? e.message : String(e)] };
		}
	}

	function addStage(atIndex?: number) {
		if (!definition) return;
		const n = definition.stages.length + 1;
		const newStage = {
			id: crypto.randomUUID(),
			name: `Stage ${n}`,
			role: 'Assistant',
			agent_id: null,
			prompt_template: '',
			tools: [],
			max_iterations: 5,
			depends_on: definition.stages.length > 0 ? [definition.stages[definition.stages.length - 1].id] : [],
			model_hint: null,
			output_spec: null,
		};
		if (atIndex !== undefined && atIndex >= 0 && atIndex < definition.stages.length) {
			const stages = [...definition.stages];
			stages.splice(atIndex + 1, 0, newStage);
			definition.stages = stages;
		} else {
			definition.stages = [...definition.stages, newStage];
		}
	}

	function removeStage(stageId: string) {
		if (!definition) return;
		definition.stages = definition.stages.filter((s) => s.id !== stageId);
		definition.stages.forEach((s) => {
			s.depends_on = s.depends_on.filter((d) => d !== stageId);
		});
	}

	function updateStage(stageId: string, patch: Partial<TaskStageDef>) {
		if (!definition) return;
		definition.stages = definition.stages.map((s) =>
			s.id === stageId ? { ...s, ...patch } : s
		);
	}

	function addInput() {
		if (!definition) return;
		definition.inputs = [
			...definition.inputs,
			{ key: `input_${definition.inputs.length + 1}`, label: '', kind: 'Text' as const, options: null, default: '', required: true },
		];
	}

	function removeInput(key: string) {
		if (!definition) return;
		definition.inputs = definition.inputs.filter((i) => i.key !== key);
	}

	function updateInput(key: string, patch: Partial<TaskInput>) {
		if (!definition) return;
		definition.inputs = definition.inputs.map((i) =>
			i.key === key ? { ...i, ...patch } : i
		);
	}

	function resetRun() {
		runId = null;
		runStatus = null;
		stopPolling();
	}

	return {
		get viewMode() { return viewMode; },
		set viewMode(v) { viewMode = v; },
		get definition() { return definition; },
		set definition(v) { definition = v; },
		get validation() { return validation; },
		get runId() { return runId; },
		get templates() { return templates; },
		get templatesLoading() { return templatesLoading; },
		get runStatus() { return runStatus; },
		newDefinition,
		loadTemplate,
		validate,
		saveTemplate,
		loadTemplates,
		startRun,
		addStage,
		removeStage,
		updateStage,
		addInput,
		removeInput,
		updateInput,
		resetRun,
	};
}

export const taskStore = createTaskStore();
