<script lang="ts">
	import { taskStore } from '$lib/stores/task.svelte';
	import TaskNodeInspector from './TaskNodeInspector.svelte';

	let selectedStageId = $state<string | null>(null);

	let selectedStage = $derived(
		selectedStageId
			? taskStore.definition?.stages.find((s) => s.id === selectedStageId) ?? null
			: null
	);

	function handleSelectStage(id: string) {
		selectedStageId = selectedStageId === id ? null : id;
	}

	function handleCloseInspector() {
		selectedStageId = null;
	}
</script>

<div class="canvas-container">
	<div class="canvas-toolbar">
		<div class="toolbar-left">
			<div class="field">
				<label for="task-name">任务名称</label>
				<input
					id="task-name"
					type="text"
					value={taskStore.definition?.name ?? ''}
					oninput={(e) => {
						if (taskStore.definition) {
							taskStore.definition = { ...taskStore.definition, name: (e.target as HTMLInputElement).value };
						}
					}}
					placeholder="输入任务名称"
				/>
			</div>
			<div class="field">
				<label for="task-desc">描述</label>
				<input
					id="task-desc"
					type="text"
					value={taskStore.definition?.description ?? ''}
					oninput={(e) => {
						if (taskStore.definition) {
							taskStore.definition = { ...taskStore.definition, description: (e.target as HTMLInputElement).value };
						}
					}}
					placeholder="简要描述任务"
				/>
			</div>
		</div>
		<div class="toolbar-right">
			<button class="btn-sm btn-secondary" onclick={taskStore.addInput}>+ 输入变量</button>
			<button class="btn-sm btn-primary" onclick={taskStore.validate}>验证</button>
			<button class="btn-sm btn-primary" onclick={taskStore.saveTemplate}>保存模板</button>
		</div>
	</div>

	{#if taskStore.definition?.inputs.length}
		<div class="inputs-section">
			<h4>输入变量</h4>
			<div class="inputs-list">
				{#each taskStore.definition.inputs as input (input.key)}
					<div class="input-chip">
						<span class="input-name">{input.label || input.key}</span>
						<span class="input-kind">{input.kind}</span>
						{#if input.required}
							<span class="input-required">必填</span>
						{/if}
						<button class="chip-remove" onclick={() => taskStore.removeInput(input.key)}>&times;</button>
					</div>
				{/each}
			</div>
		</div>
	{/if}

	<div class="stages-section">
		<div class="stages-header">
			<h4>执行阶段</h4>
			<button class="btn-sm btn-ghost" onclick={taskStore.addStage}>+ 添加阶段</button>
		</div>

		{#if !taskStore.definition?.stages.length}
			<div class="empty-stages">
				<p>暂无阶段，点击上方添加</p>
			</div>
		{:else}
			<div class="stages-flow">
				{#each taskStore.definition.stages as stage, i (stage.id)}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						class="stage-node"
						class:selected={selectedStageId === stage.id}
						onclick={() => handleSelectStage(stage.id)}
					>
						<div class="node-header">
							<span class="node-index">{i + 1}</span>
							<span class="node-name">{stage.name}</span>
							<button class="node-remove" onclick={(e) => { e.stopPropagation(); taskStore.removeStage(stage.id); }}>&times;</button>
						</div>
						<div class="node-body">
							<span class="node-role">{stage.role}</span>
							{#if stage.agent_id}
								<span class="node-agent">Agent: {stage.agent_id.slice(0, 8)}</span>
							{/if}
							{#if stage.tools.length}
								<span class="node-tools">{stage.tools.length} 工具</span>
							{/if}
							<span class="node-iter">最多 {stage.max_iterations} 轮</span>
						</div>
						{#if stage.prompt_template}
							<div class="node-prompt">{stage.prompt_template.slice(0, 60)}{stage.prompt_template.length > 60 ? '...' : ''}</div>
						{/if}
					</div>
					{#if i < taskStore.definition!.stages.length - 1}
						<div class="flow-arrow">
							<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M12 5v14M5 12l7 7 7-7"/>
							</svg>
						</div>
					{/if}
				{/each}
			</div>
		{/if}
	</div>

	{#if taskStore.validation}
		<div class="validation-bar" class:ok={taskStore.validation.ok}>
			{#if taskStore.validation.ok}
				<span>验证通过</span>
			{:else}
				<span>{taskStore.validation.errors.length} 个错误</span>
				<ul>
					{#each taskStore.validation.errors as err}
						<li>{err}</li>
					{/each}
				</ul>
			{/if}
		</div>
	{/if}
</div>

{#if selectedStage}
	<TaskNodeInspector stage={selectedStage} onClose={handleCloseInspector} />
{/if}

<style>
	.canvas-container {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		flex: 1;
		overflow-y: auto;
	}

	.canvas-toolbar {
		display: flex;
		gap: var(--spacing-md);
		align-items: flex-end;
		flex-wrap: wrap;
	}

	.toolbar-left {
		display: flex;
		gap: var(--spacing-sm);
		flex: 1;
		min-width: 0;
	}

	.toolbar-right {
		display: flex;
		gap: var(--spacing-xs);
		flex-shrink: 0;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 4px;
		flex: 1;
		min-width: 0;
	}

	.field label {
		font-size: var(--text-caption2);
		font-weight: 500;
		color: var(--color-fg-secondary);
	}

	.field input {
		padding: 8px 12px;
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: var(--text-base);
		outline: none;
	}

	.field input:focus {
		border-color: var(--color-accent);
	}

	.btn-sm {
		padding: 6px 12px;
		border-radius: var(--radius-full);
		border: none;
		cursor: pointer;
		font-size: var(--text-caption1);
		font-weight: 500;
	}

	.btn-primary {
		background: var(--color-accent);
		color: #fff;
	}

	.btn-secondary {
		background: var(--color-bg);
		color: var(--color-fg);
	}

	.btn-ghost {
		background: transparent;
		color: var(--color-accent);
	}

	/* Inputs */
	.inputs-section h4,
	.stages-header h4 {
		font-size: var(--text-subheadline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0 0 var(--spacing-xs);
	}

	.inputs-list {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-xs);
	}

	.input-chip {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 10px;
		background: var(--color-bg);
		border-radius: var(--radius-full);
		font-size: var(--text-caption1);
		color: var(--color-fg);
	}

	.input-kind {
		color: var(--color-fg-tertiary);
		font-size: var(--text-caption2);
	}

	.input-required {
		color: var(--color-orange);
		font-size: var(--text-caption2);
	}

	.chip-remove {
		border: none;
		background: none;
		color: var(--color-fg-tertiary);
		cursor: pointer;
		font-size: 14px;
		padding: 0 2px;
		line-height: 1;
	}

	.chip-remove:hover {
		color: var(--color-red);
	}

	/* Stages */
	.stages-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.empty-stages {
		padding: var(--spacing-xxl);
		text-align: center;
		color: var(--color-fg-tertiary);
		font-size: var(--text-subheadline);
	}

	.stages-flow {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0;
	}

	.stage-node {
		width: 100%;
		max-width: 480px;
		padding: var(--spacing-sm);
		background: var(--color-bg);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		cursor: pointer;
		transition: border-color 0.15s ease, box-shadow 0.15s ease;
	}

	.stage-node:hover {
		border-color: var(--color-accent);
	}

	.stage-node.selected {
		border-color: var(--color-accent);
		box-shadow: 0 0 0 1px var(--color-accent);
	}

	.node-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		margin-bottom: var(--spacing-xs);
	}

	.node-index {
		width: 22px;
		height: 22px;
		border-radius: 50%;
		background: var(--color-accent);
		color: #fff;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: var(--text-caption2);
		font-weight: 600;
		flex-shrink: 0;
	}

	.node-name {
		font-size: var(--text-subheadline);
		font-weight: 600;
		color: var(--color-fg);
		flex: 1;
	}

	.node-remove {
		border: none;
		background: none;
		color: var(--color-fg-tertiary);
		cursor: pointer;
		font-size: 16px;
		padding: 2px 6px;
		border-radius: var(--radius-sm);
		line-height: 1;
	}

	.node-remove:hover {
		color: var(--color-red);
		background: var(--color-bg-secondary);
	}

	.node-body {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-xs);
	}

	.node-role,
	.node-agent,
	.node-tools,
	.node-iter {
		font-size: var(--text-caption2);
		padding: 2px 8px;
		border-radius: var(--radius-full);
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
	}

	.node-prompt {
		margin-top: var(--spacing-xs);
		font-size: var(--text-caption1);
		color: var(--color-fg-tertiary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.flow-arrow {
		color: var(--color-fg-tertiary);
		padding: var(--spacing-xs) 0;
	}

	/* Validation */
	.validation-bar {
		padding: var(--spacing-sm);
		border-radius: var(--radius-md);
		background: rgba(255, 59, 48, 0.1);
		color: var(--color-red);
		font-size: var(--text-caption1);
	}

	.validation-bar.ok {
		background: rgba(52, 199, 89, 0.1);
		color: var(--color-green);
	}

	.validation-bar ul {
		margin: var(--spacing-xs) 0 0;
		padding-left: var(--spacing-lg);
	}
</style>
