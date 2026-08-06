<script lang="ts">
	import { taskStore } from '$lib/stores/task.svelte';
	import { onMount } from 'svelte';

	let runInputs = $state<Record<string, string>>({});
	let submitting = $state(false);
	let statusMessage = $state('');
	let elapsed = $state(0);
	let timer: ReturnType<typeof setInterval> | null = null;

	let stagesTotal = $derived(taskStore.definition?.stages.length ?? 0);
	let currentStageIdx = $derived(taskStore.runStatus?.stages_done ?? 0);
	let progress = $derived(stagesTotal > 0 ? (currentStageIdx / stagesTotal) * 100 : 0);

	function initInputs() {
		if (!taskStore.definition) return;
		runInputs = {};
		for (const inp of taskStore.definition.inputs) {
			runInputs[inp.key] = String(inp.default ?? '');
		}
	}

	function startTimer() {
		elapsed = 0;
		timer = setInterval(() => { elapsed += 1; }, 1000);
	}

	function stopTimer() {
		if (timer) { clearInterval(timer); timer = null; }
	}

	async function handleStart() {
		if (!taskStore.definition || submitting) return;
		submitting = true;
		statusMessage = '启动中...';
		startTimer();

		const inputs: Record<string, any> = {};
		for (const inp of taskStore.definition.inputs) {
			const raw = runInputs[inp.key] ?? '';
			if (inp.kind === 'Number') inputs[inp.key] = Number(raw) || 0;
			else inputs[inp.key] = raw;
		}

		try {
			await taskStore.startRun(inputs);
			statusMessage = '运行中';
		} catch (e) {
			statusMessage = '启动失败: ' + (e instanceof Error ? e.message : String(e));
			stopTimer();
		} finally {
			submitting = false;
		}
	}

	function formatElapsed(s: number): string {
		const m = Math.floor(s / 60);
		const sec = s % 60;
		return `${m}:${sec.toString().padStart(2, '0')}`;
	}

	function handleBack() {
		stopTimer();
		taskStore.viewMode = 'design';
		taskStore.resetRun();
	}

	onMount(() => {
		initInputs();
		return () => stopTimer();
	});
</script>

<div class="run-panel">
	<div class="run-header">
		<button class="back-btn" onclick={handleBack}>
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M19 12H5M12 19l-7-7 7-7"/>
			</svg>
			返回设计
		</button>
		<h3>运行任务</h3>
		<span class="run-id">{taskStore.runId ?? '未启动'}</span>
	</div>

	{#if !taskStore.runId}
		<div class="run-inputs">
			{#if taskStore.definition?.inputs.length}
				<h4>输入参数</h4>
				{#each taskStore.definition.inputs as inp (inp.key)}
					<div class="field">
						<label for="run-{inp.key}">
							{inp.label || inp.key}
							{#if inp.required}<span class="req">*</span>{/if}
						</label>
						{#if inp.kind === 'Textarea'}
							<textarea id="run-{inp.key}" bind:value={runInputs[inp.key]} rows="3"></textarea>
						{:else if inp.kind === 'Select'}
							<select id="run-{inp.key}" bind:value={runInputs[inp.key]}>
								{#each inp.options ?? [] as opt}
									<option value={opt}>{opt}</option>
								{/each}
							</select>
						{:else}
							<input id="run-{inp.key}" type={inp.kind === 'Number' ? 'number' : 'text'} bind:value={runInputs[inp.key]} />
						{/if}
					</div>
				{/each}
			{:else}
				<p class="no-inputs">此任务无需输入参数</p>
			{/if}

			<button class="start-btn" onclick={handleStart} disabled={submitting}>
				{submitting ? '启动中...' : '开始运行'}
			</button>
		</div>
	{:else}
		<div class="run-progress">
			<div class="progress-info">
				<span class="status-text">{statusMessage}</span>
				<span class="elapsed">{formatElapsed(elapsed)}</span>
			</div>
			<div class="progress-bar">
				<div class="progress-fill" style:width="{progress}%"></div>
			</div>
			<div class="progress-detail">
				阶段 {currentStageIdx} / {stagesTotal}
			</div>
		</div>

		<div class="run-timeline">
			<h4>执行时间线</h4>
			{#if taskStore.definition}
				{#each taskStore.definition.stages as stage, i (stage.id)}
					<div class="timeline-item" class:done={i < currentStageIdx} class:active={i === currentStageIdx}>
						<div class="timeline-dot"></div>
						<div class="timeline-content">
							<span class="timeline-name">{stage.name}</span>
							<span class="timeline-role">{stage.role}</span>
						</div>
					</div>
				{/each}
			{/if}
		</div>

		{#if taskStore.runStatus?.outputs}
			<div class="run-outputs">
				<h4>输出结果</h4>
				{#each Object.entries(taskStore.runStatus.outputs) as [key, value]}
					<div class="output-item">
						<span class="output-key">{key}</span>
						<span class="output-value">{value}</span>
					</div>
				{/each}
			</div>
		{/if}

		{#if taskStore.runStatus?.error}
			<div class="run-error">
				<span>错误: {taskStore.runStatus.error}</span>
			</div>
		{/if}
	{/if}
</div>

<style>
	.run-panel {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		flex: 1;
		overflow-y: auto;
	}

	.run-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.back-btn {
		display: flex;
		align-items: center;
		gap: 4px;
		border: none;
		background: none;
		color: var(--color-accent);
		cursor: pointer;
		font-size: var(--text-caption1);
		padding: 4px 8px;
		border-radius: var(--radius-sm);
	}

	.back-btn:hover {
		background: var(--color-bg-secondary);
	}

	.run-header h3 {
		font-size: var(--text-headline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.run-id {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
		font-family: var(--font-mono);
	}

	/* Inputs */
	.run-inputs {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		max-width: 480px;
	}

	.run-inputs h4 {
		font-size: var(--text-subheadline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.no-inputs {
		color: var(--color-fg-tertiary);
		font-size: var(--text-subheadline);
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.field label {
		font-size: var(--text-caption2);
		font-weight: 500;
		color: var(--color-fg-secondary);
	}

	.req {
		color: var(--color-red);
	}

	.field input,
	.field select,
	.field textarea {
		padding: 8px 12px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		font-size: var(--text-base);
		font-family: var(--font-sans);
		outline: none;
	}

	.field input:focus,
	.field select:focus,
	.field textarea:focus {
		border-color: var(--color-accent);
	}

	.start-btn {
		align-self: flex-start;
		padding: 10px 24px;
		border-radius: var(--radius-full);
		border: none;
		background: var(--color-green);
		color: #fff;
		font-size: var(--text-base);
		font-weight: 600;
		cursor: pointer;
		margin-top: var(--spacing-sm);
	}

	.start-btn:hover { opacity: 0.9; }
	.start-btn:disabled { opacity: 0.5; cursor: not-allowed; }

	/* Progress */
	.run-progress {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.progress-info {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.status-text {
		font-size: var(--text-subheadline);
		font-weight: 500;
		color: var(--color-fg);
	}

	.elapsed {
		font-size: var(--text-caption1);
		color: var(--color-fg-tertiary);
		font-family: var(--font-mono);
	}

	.progress-bar {
		height: 6px;
		border-radius: 3px;
		background: var(--color-bg);
		overflow: hidden;
	}

	.progress-fill {
		height: 100%;
		border-radius: 3px;
		background: var(--color-accent);
		transition: width 0.3s ease;
	}

	.progress-detail {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
	}

	/* Timeline */
	.run-timeline {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.run-timeline h4 {
		font-size: var(--text-subheadline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.timeline-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-xs) 0;
	}

	.timeline-dot {
		width: 12px;
		height: 12px;
		border-radius: 50%;
		background: var(--color-bg-secondary);
		border: 2px solid var(--color-separator);
		flex-shrink: 0;
	}

	.timeline-item.done .timeline-dot {
		background: var(--color-green);
		border-color: var(--color-green);
	}

	.timeline-item.active .timeline-dot {
		background: var(--color-accent);
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.2);
	}

	.timeline-content {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.timeline-name {
		font-size: var(--text-subheadline);
		font-weight: 500;
		color: var(--color-fg);
	}

	.timeline-role {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
	}

	/* Outputs */
	.run-outputs {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.run-outputs h4 {
		font-size: var(--text-subheadline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.output-item {
		display: flex;
		gap: var(--spacing-xs);
		padding: var(--spacing-xs);
		background: var(--color-bg-secondary);
		border-radius: var(--radius-sm);
	}

	.output-key {
		font-size: var(--text-caption1);
		font-weight: 500;
		color: var(--color-fg-secondary);
		flex-shrink: 0;
	}

	.output-value {
		font-size: var(--text-caption1);
		color: var(--color-fg);
		word-break: break-word;
	}

	/* Error */
	.run-error {
		padding: var(--spacing-sm);
		border-radius: var(--radius-md);
		background: rgba(255, 59, 48, 0.1);
		color: var(--color-red);
		font-size: var(--text-caption1);
	}
</style>
