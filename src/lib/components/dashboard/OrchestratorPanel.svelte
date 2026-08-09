<script lang="ts">
	import { invoke } from '$lib/api/client';
	import { orchestratorStore } from '$lib/stores/orchestrator.svelte';

	const session = $derived(orchestratorStore.session);
	const events = $derived(orchestratorStore.events);
	const loading = $derived(orchestratorStore.loading);
	const error = $derived(orchestratorStore.error);

	let userInput = $state('');
	let activeTab = $state<'input' | 'spec' | 'execution' | 'review'>('input');

	const tabs = [
		{ key: 'input', label: '输入' },
		{ key: 'spec', label: 'SPEC' },
		{ key: 'execution', label: '执行' },
		{ key: 'review', label: '审查' },
	] as const;

	interface TaskState {
		status: 'pending' | 'running' | 'completed' | 'failed';
		role: string;
		model_id: string;
		group_id: string | null;
		duration_ms: number | null;
		tokens_used: number | null;
		output_summary: string | null;
		error: string | null;
	}

	// 以 data.task_id 为键，task_finished 覆盖 task_started（事件流为最新在前，倒序遍历让最新事件胜出）
	const taskStates = $derived.by(() => {
		const map = new Map<string, TaskState>();
		for (let i = events.length - 1; i >= 0; i--) {
			const ev = events[i];
			const data = ev.data ?? {};
			if (!data.task_id) continue;
			const prev = map.get(data.task_id);
			const base: TaskState = prev ?? {
				status: 'running',
				role: '',
				model_id: '',
				group_id: null,
				duration_ms: null,
				tokens_used: null,
				output_summary: null,
				error: null,
			};
			if (ev.event_type === 'task_started') {
				map.set(data.task_id, {
					...base,
					status: 'running',
					role: data.role ?? base.role,
					model_id: data.model_id ?? base.model_id,
					group_id: data.group_id ?? base.group_id,
				});
			} else if (ev.event_type === 'task_finished') {
				map.set(data.task_id, {
					...base,
					status: data.status === 'completed' ? 'completed' : 'failed',
					duration_ms: data.duration_ms ?? base.duration_ms,
					tokens_used: data.tokens_used ?? base.tokens_used,
					output_summary: data.output_summary ?? base.output_summary,
					error: data.error ?? base.error,
				});
			}
		}
		return map;
	});

	const completedCount = $derived([...taskStates.values()].filter((t) => t.status === 'completed').length);
	const totalTasks = $derived(session?.plan?.total_tasks ?? 0);
	const progressPct = $derived(totalTasks > 0 ? Math.min(100, Math.round((completedCount / totalTasks) * 100)) : 0);

	async function startOrchestration() {
		if (!userInput.trim()) return;
		const s = await orchestratorStore.startSession(userInput.trim());
		if (s) {
			activeTab = 'spec';
		}
	}

	async function pauseSession() {
		if (!session) return;
		await invoke('orchestrator_pause', { sessionId: session.id });
		session.status = 'paused';
	}

	async function resumeSession() {
		if (!session) return;
		const s = await invoke('orchestrator_resume', { sessionId: session.id });
		if (s) {
			session.status = 'executing';
			orchestratorStore.attachListeners?.();
		}
	}

	async function stopSession() {
		if (!session) return;
		await invoke('orchestrator_stop', { sessionId: session.id });
		session.status = 'failed';
	}

	function statusLabel(status: string): string {
		switch (status) {
			case 'spec_generating': return '正在分析需求...';
			case 'spec_reviewing': return '等待确认 SPEC';
			case 'plan_generating': return '正在生成执行计划...';
			case 'executing': return '正在执行任务';
			case 'reviewing': return '正在审查结果';
			case 'repairing': return '正在修复失败任务';
			case 'completed': return '全部完成';
			case 'paused': return '已暂停';
			case 'budget_exhausted': return '预算耗尽';
			case 'failed': return '执行失败';
			default: return status;
		}
	}

	function statusColor(status: string): string {
		switch (status) {
			case 'completed': return 'badge-green';
			case 'executing': return 'badge-accent';
			case 'reviewing': return 'badge-yellow';
			case 'failed':
			case 'budget_exhausted': return 'badge-red';
			default: return 'badge-neutral';
		}
	}

	function complexityColor(c: string): string {
		switch (c) {
			case 'high': return 'complexity-high';
			case 'medium': return 'complexity-medium';
			default: return 'complexity-low';
		}
	}

	function taskStatusLabel(status: string): string {
		switch (status) {
			case 'completed': return '已完成';
			case 'failed': return '失败';
			case 'running': return '执行中';
			default: return '待运行';
		}
	}

	function taskStatusBadge(status: string): string {
		switch (status) {
			case 'completed': return 'badge-green';
			case 'failed': return 'badge-red';
			case 'running': return 'badge-accent';
			default: return 'badge-neutral';
		}
	}

	function eventBadgeClass(eventType: string): string {
		if (eventType.includes('failed') || eventType.includes('exhausted')) return 'badge-red';
		if (eventType.includes('completed') || eventType.includes('passed')) return 'badge-green';
		if (eventType.includes('executing') || eventType.includes('reviewing')) return 'badge-yellow';
		return 'badge-accent';
	}

	function formatTime(ts: number): string {
		const d = new Date(ts);
		return d.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', second: '2-digit' });
	}

	function formatDuration(ms: number): string {
		if (ms < 1000) return `${ms}ms`;
		const secs = ms / 1000;
		if (secs < 60) return `${secs.toFixed(1)}s`;
		const mins = Math.floor(secs / 60);
		const s = Math.round(secs % 60);
		return `${mins}m ${s}s`;
	}
</script>

<div class="orchestrator-panel">
	<!-- Header -->
	<header class="panel-header">
		<h2 class="panel-title">自主编排</h2>
		{#if session}
			<div class="header-actions">
				<span class="badge {statusColor(session.status)}">{statusLabel(session.status)}</span>
				{#if session.status === 'executing' || session.status === 'spec_generating' || session.status === 'plan_generating' || session.status === 'reviewing' || session.status === 'repairing'}
					<button class="btn btn-secondary" onclick={pauseSession}>暂停</button>
				{/if}
				{#if session.status === 'paused'}
					<button class="btn btn-primary" onclick={resumeSession}>继续</button>
				{/if}
				{#if session.status !== 'completed'}
					<button class="btn btn-danger" onclick={stopSession}>终止</button>
				{/if}
				<button class="btn-link" onclick={() => { orchestratorStore.reset(); activeTab = 'input'; }}>新建</button>
			</div>
		{/if}
	</header>

	<!-- Content -->
	<div class="panel-content">
		{#if !session}
			<!-- Input View -->
			<div class="input-view">
				<div class="input-card">
					<div class="input-heading">
						<h3>描述你的需求</h3>
						<p>输入模糊需求，AI 将自动生成计划、分配 Agent 并执行</p>
					</div>

					<textarea
						placeholder="例如：帮我实现一个用户认证系统，包含登录、注册、JWT token 刷新、权限中间件"
						bind:value={userInput}
						onkeydown={(e) => {
							if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
								startOrchestration();
							}
						}}
					></textarea>

					{#if error}
						<div class="error-banner">{error}</div>
					{/if}

					<button
						class="btn btn-primary btn-block"
						onclick={startOrchestration}
						disabled={loading || !userInput.trim()}
					>
						{loading ? '分析中...' : '开始编排'}
					</button>

					<p class="hint">按 Ctrl+Enter 快速开始</p>
				</div>
			</div>
		{:else}
			<!-- Tabs -->
			<nav class="tabs">
				{#each tabs as tab (tab.key)}
					<button
						class="tab"
						class:active={activeTab === tab.key}
						onclick={() => activeTab = tab.key}
					>
						{tab.label}
					</button>
				{/each}
			</nav>

			<!-- Tab Content -->
			<div class="tab-content">
				{#if activeTab === 'input'}
					<div class="stack">
						<div class="card">
							<h4 class="card-title">需求</h4>
							<p class="body-strong request-text">{session.user_request}</p>
						</div>
						<div class="card">
							<h4 class="card-title">进度</h4>
							<div class="meta-list">
								<div class="weak">循环次数: {session.cycle_count} / {session.max_cycles}</div>
								<div class="weak">状态: {statusLabel(session.status)}</div>
								<div class="weak">事件数: {events.length}</div>
							</div>
						</div>
					</div>

				{:else if activeTab === 'spec'}
					{#if session.spec}
						<div class="stack">
							<div class="card">
								<h4 class="card-title">需求摘要</h4>
								<p class="body-text">{session.spec.summary}</p>
							</div>

							<div class="card">
								<h4 class="card-title">任务清单</h4>
								<div class="spec-task-list">
									{#each session.spec.tasks as task (task.id)}
										<div class="spec-task">
											<span class="spec-task-id mono">{task.id}</span>
											<div class="spec-task-body">
												<div class="spec-task-title-row">
													<span class="body-strong">{task.title}</span>
													<span class="complexity {complexityColor(task.estimated_complexity)}">{task.estimated_complexity}</span>
												</div>
												<p class="body-text spec-task-desc">{task.description}</p>
												{#if task.acceptance.length > 0}
													<div class="weak">验收: {task.acceptance.join(' | ')}</div>
												{/if}
											</div>
										</div>
									{/each}
								</div>
							</div>

							{#if Object.keys(session.spec.dependencies).length > 0}
								<div class="card">
									<h4 class="card-title">依赖关系</h4>
									<div class="meta-list">
										{#each Object.entries(session.spec.dependencies) as [taskId, deps]}
											<div class="weak dep-row">{taskId} → {deps.join(', ')}</div>
										{/each}
									</div>
								</div>
							{/if}
						</div>
					{:else}
						<div class="empty-state">
							{session.status === 'spec_generating' ? '正在生成 SPEC...' : 'SPEC 尚未生成'}
						</div>
					{/if}

				{:else if activeTab === 'execution'}
					{#if session.plan}
						<div class="stack">
							<!-- Progress -->
							<div class="card">
								<div class="progress-header">
									<h4 class="card-title progress-title">执行进度</h4>
									<span class="progress-label">{completedCount} / {totalTasks} · {progressPct}%</span>
								</div>
								<div class="progress-track">
									<div class="progress-fill" style:width="{progressPct}%"></div>
								</div>
							</div>

							<!-- Task Cards -->
							{#each session.plan.groups as group, gi (group.id)}
								<div class="card">
									<div class="group-header">
										<span class="group-label">第 {gi + 1} 组</span>
										<span class="badge {group.kind === 'parallel' ? 'badge-accent' : 'badge-neutral'}">
											{group.kind === 'parallel' ? '并行' : '顺序'}
										</span>
									</div>
									<div class="task-list">
										{#each group.tasks as task (task.spec_task_id)}
											{@const state = taskStates.get(task.spec_task_id)}
											{@const tStatus = state?.status ?? 'pending'}
											{@const tRole = state?.role || task.agent_config.role}
											{@const tModel = state?.model_id || task.agent_config.model_id}
											<div class="task-card" class:task-failed={tStatus === 'failed'}>
												<div class="task-head">
													<span class="task-id mono">{task.spec_task_id}</span>
													<span class="badge {taskStatusBadge(tStatus)}">{taskStatusLabel(tStatus)}</span>
												</div>
												<div class="task-meta">
													<span class="task-role">{tRole}</span>
													<span class="task-model mono">{tModel}</span>
												</div>
												{#if (tStatus === 'completed' || tStatus === 'failed') && (state?.duration_ms != null || state?.tokens_used != null)}
													<div class="task-stats">
														{#if state?.duration_ms != null}
															<span>耗时 {formatDuration(state.duration_ms)}</span>
														{/if}
														{#if state?.tokens_used != null}
															<span>token {state.tokens_used}</span>
														{/if}
													</div>
												{/if}
												{#if state?.output_summary}
													<p class="task-summary">{state.output_summary}</p>
												{/if}
												{#if tStatus === 'failed' && state?.error}
													<div class="task-error">{state.error}</div>
												{/if}
											</div>
										{/each}
									</div>
								</div>
							{/each}

							<!-- Live Events -->
							{#if events.length > 0}
								<div class="card">
									<h4 class="card-title">实时事件</h4>
									<div class="event-list">
										{#each events.slice(0, 20) as event (event.timestamp)}
											<div class="event-row">
												<span class="event-time">{formatTime(event.timestamp)}</span>
												<span class="event-msg">{event.message}</span>
											</div>
										{/each}
									</div>
								</div>
							{/if}
						</div>
					{:else}
						<div class="empty-state">
							{session.status === 'plan_generating' ? '正在生成执行计划...' : '执行计划尚未生成'}
						</div>
					{/if}

				{:else if activeTab === 'review'}
					<div class="stack">
						<!-- All Events (Review Log) -->
						<div class="card">
							<h4 class="card-title">审查日志</h4>
							{#if events.length === 0}
								<div class="empty-inline">暂无事件</div>
							{:else}
								<div class="event-list event-list-bordered">
									{#each events as event (event.timestamp)}
										<div class="event-row">
											<span class="event-time event-time-wide">{formatTime(event.timestamp)}</span>
											<span class="event-badge {eventBadgeClass(event.event_type)}">{event.event_type}</span>
											<span class="event-msg">{event.message}</span>
										</div>
									{/each}
								</div>
							{/if}
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>

<style>
	.orchestrator-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--color-bg-elevated);
		color: var(--color-fg);
		font-family: var(--font-sans);
	}

	/* ── Header ─────────────────────────────── */
	.panel-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		border-bottom: 1px solid var(--color-separator);
		flex-shrink: 0;
	}

	.panel-title {
		margin: 0;
		font-size: var(--text-headline);
		font-weight: var(--font-weight-semibold);
		color: var(--color-fg);
	}

	.header-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
		justify-content: flex-end;
	}

	/* ── Badges ─────────────────────────────── */
	.badge {
		display: inline-flex;
		align-items: center;
		padding: 2px 8px;
		border-radius: var(--radius-full);
		font-size: var(--text-caption2);
		font-weight: var(--font-weight-medium);
		line-height: 1.5;
		white-space: nowrap;
	}

	.badge-green {
		background: color-mix(in srgb, var(--color-green) 14%, transparent);
		color: var(--color-green);
	}

	.badge-accent {
		background: color-mix(in srgb, var(--color-accent) 14%, transparent);
		color: var(--color-accent);
	}

	.badge-yellow {
		background: color-mix(in srgb, var(--color-orange) 16%, transparent);
		color: var(--color-orange);
	}

	.badge-red {
		background: color-mix(in srgb, var(--color-red) 14%, transparent);
		color: var(--color-red);
	}

	.badge-neutral {
		background: var(--color-bg-hover);
		color: var(--color-fg-secondary);
	}

	/* ── Buttons ────────────────────────────── */
	.btn {
		font-family: inherit;
		font-size: var(--text-caption1);
		font-weight: var(--font-weight-medium);
		padding: 4px 10px;
		border-radius: var(--radius-sm);
		border: 1px solid transparent;
		cursor: pointer;
		transition: background var(--duration-fast) var(--ease-default), color var(--duration-fast) var(--ease-default);
	}

	.btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.btn-secondary {
		background: var(--color-bg-hover);
		color: var(--color-fg-secondary);
	}

	.btn-secondary:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-fg);
	}

	.btn-primary {
		background: var(--color-accent);
		color: #fff;
	}

	.btn-primary:hover {
		background: var(--color-accent-hover);
	}

	.btn-danger {
		background: color-mix(in srgb, var(--color-red) 15%, transparent);
		color: var(--color-red);
	}

	.btn-danger:hover {
		background: color-mix(in srgb, var(--color-red) 25%, transparent);
	}

	.btn-block {
		width: 100%;
		padding: 12px;
		border-radius: var(--radius-md);
		font-size: var(--text-body);
		font-weight: var(--font-weight-medium);
	}

	.btn-link {
		font-family: inherit;
		font-size: var(--text-caption1);
		padding: 4px;
		background: none;
		border: none;
		color: var(--color-fg-tertiary);
		cursor: pointer;
		transition: color var(--duration-fast) var(--ease-default);
	}

	.btn-link:hover {
		color: var(--color-fg-secondary);
	}

	/* ── Content / Input View ───────────────── */
	.panel-content {
		flex: 1;
		min-height: 0;
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}

	.input-view {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-lg);
		overflow-y: auto;
	}

	.input-card {
		width: 100%;
		max-width: 42rem;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.input-heading {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		text-align: center;
	}

	.input-heading h3 {
		margin: 0;
		font-size: var(--text-title3);
		font-weight: var(--font-weight-semibold);
		color: var(--color-fg);
	}

	.input-heading p {
		margin: 0;
		font-size: var(--text-footnote);
		color: var(--color-fg-secondary);
	}

	textarea {
		box-sizing: border-box;
		width: 100%;
		height: 128px;
		padding: var(--spacing-md);
		border-radius: var(--radius-md);
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-separator);
		color: var(--color-fg);
		font-family: inherit;
		font-size: var(--text-footnote);
		line-height: 1.5;
		resize: none;
		transition: border-color var(--duration-fast) var(--ease-default), box-shadow var(--duration-fast) var(--ease-default);
	}

	textarea::placeholder {
		color: var(--color-fg-tertiary);
	}

	textarea:focus {
		outline: none;
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px var(--color-focus-ring);
	}

	.error-banner {
		font-size: var(--text-footnote);
		color: var(--color-red);
		background: color-mix(in srgb, var(--color-red) 10%, transparent);
		border-radius: var(--radius-sm);
		padding: 12px;
	}

	.hint {
		margin: 0;
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
		text-align: center;
	}

	/* ── Tabs ───────────────────────────────── */
	.tabs {
		display: flex;
		border-bottom: 1px solid var(--color-separator);
		flex-shrink: 0;
	}

	.tab {
		font-family: inherit;
		font-size: var(--text-caption1);
		font-weight: var(--font-weight-medium);
		padding: 8px 16px;
		background: none;
		border: none;
		border-bottom: 2px solid transparent;
		color: var(--color-fg-tertiary);
		cursor: pointer;
		transition: color var(--duration-fast) var(--ease-default);
	}

	.tab:hover {
		color: var(--color-fg-secondary);
	}

	.tab.active {
		color: var(--color-accent);
		border-bottom-color: var(--color-accent);
	}

	.tab-content {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		padding: var(--spacing-md);
	}

	.stack {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	/* ── Cards ──────────────────────────────── */
	.card {
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-sm);
		padding: var(--spacing-md);
	}

	.card-title {
		margin: 0 0 var(--spacing-sm);
		font-size: var(--text-footnote);
		font-weight: var(--font-weight-semibold);
		color: var(--color-fg-secondary);
	}

	/* ── Text helpers ───────────────────────── */
	.body-text {
		margin: 0;
		font-size: var(--text-footnote);
		color: var(--color-fg-secondary);
		line-height: 1.5;
	}

	.body-strong {
		font-size: var(--text-footnote);
		font-weight: var(--font-weight-medium);
		color: var(--color-fg);
	}

	.weak {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
		line-height: 1.5;
	}

	.mono {
		font-family: var(--font-mono);
	}

	.meta-list {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.empty-state {
		text-align: center;
		padding: var(--spacing-xl) 0;
		font-size: var(--text-footnote);
		color: var(--color-fg-tertiary);
	}

	.empty-inline {
		font-size: var(--text-footnote);
		color: var(--color-fg-tertiary);
		padding: var(--spacing-sm) 0;
	}

	/* ── Input tab ──────────────────────────── */
	.request-text {
		white-space: pre-wrap;
		word-break: break-word;
	}

	/* ── Spec tab ───────────────────────────── */
	.spec-task-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.spec-task {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		padding: 12px;
		border-radius: var(--radius-sm);
		background: var(--color-bg-secondary);
	}

	.spec-task-id {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
		margin-top: 2px;
		flex-shrink: 0;
	}

	.spec-task-body {
		flex: 1;
		min-width: 0;
	}

	.spec-task-title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.spec-task-desc {
		margin-top: 4px;
		word-break: break-word;
	}

	.spec-task-body .weak {
		margin-top: var(--spacing-sm);
	}

	.complexity {
		font-size: 10px;
		font-weight: var(--font-weight-medium);
		text-transform: uppercase;
	}

	.complexity-high {
		color: var(--color-red);
	}

	.complexity-medium {
		color: var(--color-orange);
	}

	.complexity-low {
		color: var(--color-green);
	}

	.dep-row {
		font-family: var(--font-mono);
		word-break: break-word;
	}

	/* ── Execution tab ──────────────────────── */
	.progress-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-sm);
	}

	.progress-title {
		margin-bottom: 0;
	}

	.progress-label {
		font-size: var(--text-caption2);
		color: var(--color-fg-secondary);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.progress-track {
		margin-top: var(--spacing-sm);
		height: 6px;
		border-radius: var(--radius-full);
		background: var(--color-bg-secondary);
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		border-radius: var(--radius-full);
		background: var(--color-accent);
		transition: width var(--duration-normal) var(--ease-default);
	}

	.group-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-sm);
	}

	.group-label {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
	}

	.task-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.task-card {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-sm);
		padding: 12px;
	}

	.task-card.task-failed {
		border-color: color-mix(in srgb, var(--color-red) 40%, transparent);
	}

	.task-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-sm);
	}

	.task-id {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
	}

	.task-meta {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
		margin-top: 6px;
	}

	.task-role {
		font-size: var(--text-footnote);
		font-weight: var(--font-weight-medium);
		color: var(--color-fg);
	}

	.task-model {
		font-size: var(--text-caption2);
		color: var(--color-fg-secondary);
	}

	.task-stats {
		display: flex;
		gap: var(--spacing-md);
		margin-top: 8px;
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
		font-variant-numeric: tabular-nums;
	}

	.task-summary {
		margin: 8px 0 0;
		font-size: var(--text-caption2);
		color: var(--color-fg-secondary);
		line-height: 1.5;
		word-break: break-word;
		display: -webkit-box;
		-webkit-line-clamp: 3;
		line-clamp: 3;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.task-error {
		margin-top: 8px;
		padding: 8px 10px;
		font-size: var(--text-caption2);
		color: var(--color-red);
		background: color-mix(in srgb, var(--color-red) 8%, transparent);
		border-radius: var(--radius-sm);
		word-break: break-word;
		white-space: pre-wrap;
	}

	/* ── Event log ──────────────────────────── */
	.event-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
		max-height: 240px;
		overflow-y: auto;
	}

	.event-list-bordered {
		gap: 0;
		max-height: 384px;
	}

	.event-list-bordered .event-row {
		padding: 6px 0;
		border-bottom: 1px solid var(--color-separator);
	}

	.event-list-bordered .event-row:last-child {
		border-bottom: none;
	}

	.event-row {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		font-size: var(--text-caption2);
		line-height: 1.5;
	}

	.event-time {
		color: var(--color-fg-tertiary);
		flex-shrink: 0;
		font-variant-numeric: tabular-nums;
	}

	.event-time-wide {
		width: 64px;
	}

	.event-msg {
		color: var(--color-fg-secondary);
		min-width: 0;
		word-break: break-word;
	}

	.event-badge {
		display: inline-flex;
		align-items: center;
		padding: 1px 6px;
		border-radius: var(--radius-sm);
		font-family: var(--font-mono);
		font-size: 10px;
		font-weight: var(--font-weight-medium);
		line-height: 1.5;
		flex-shrink: 0;
	}
</style>
