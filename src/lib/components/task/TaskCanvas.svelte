<script lang="ts">
	import { taskStore } from '$lib/stores/task.svelte';
	import TaskNodeInspector from './TaskNodeInspector.svelte';

	let selectedStageId = $state<string | null>(null);
	let dragOverIndex = $state<number | null>(null);

	let selectedStage = $derived(
		selectedStageId
			? taskStore.definition?.stages.find((s) => s.id === selectedStageId) ?? null
			: null
	);

	function handleSelectStage(id: string) {
		selectedStageId = selectedStageId === id ? null : id;
	}

	function handleAddStage(afterIndex?: number) {
		const idx = afterIndex !== undefined ? afterIndex + 1 : undefined;
		taskStore.addStage(idx);
	}

	function handleDragStart(e: DragEvent, stageId: string) {
		e.dataTransfer?.setData('text/plain', stageId);
	}

	function handleDragOver(e: DragEvent, index: number) {
		e.preventDefault();
		dragOverIndex = index;
	}

	function handleDrop(e: DragEvent, targetIndex: number) {
		e.preventDefault();
		dragOverIndex = null;
		const sourceId = e.dataTransfer?.getData('text/plain');
		if (sourceId && taskStore.definition) {
			const stages = [...taskStore.definition.stages];
			const sourceIdx = stages.findIndex((s) => s.id === sourceId);
			if (sourceIdx !== -1 && sourceIdx !== targetIndex) {
				const [moved] = stages.splice(sourceIdx, 1);
				stages.splice(targetIndex, 0, moved);
				taskStore.definition = { ...taskStore.definition, stages };
			}
		}
	}

	const roleColors: Record<string, string> = {
		'研究员': '#7c5cfc',
		'分析师': '#0ea5e9',
		'写手': '#f97316',
		'翻译': '#10b981',
		'审校员': '#ec4899',
		'编辑': '#8b5cf6',
		'审查员': '#ef4444',
		'顾问': '#06b6d4',
		'创意官': '#f59e0b',
		'评审官': '#6366f1',
		'策略师': '#14b8a6',
	};

	function getRoleColor(role: string): string {
		return roleColors[role] || 'var(--color-fg-secondary)';
	}
</script>

<div class="canvas-wrapper">
	<!-- Toolbar -->
	<div class="toolbar">
		<div class="toolbar-left">
			<input
				type="text"
				class="name-input"
				value={taskStore.definition?.name ?? ''}
				oninput={(e) => {
					if (taskStore.definition) {
						taskStore.definition = { ...taskStore.definition, name: (e.target as HTMLInputElement).value };
					}
				}}
				placeholder="输入工作流名称"
			/>
		</div>
		<div class="toolbar-right">
			<button class="tb-btn" onclick={taskStore.validate}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 6L9 17l-5-5"/></svg>
				验证
			</button>
			<button class="tb-btn" onclick={taskStore.saveTemplate}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/></svg>
				保存模板
			</button>
			<button class="tb-btn run" onclick={() => taskStore.startRun()}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21"/></svg>
				运行
			</button>
		</div>
	</div>

	<!-- Validation errors -->
	{#if taskStore.validation && !taskStore.validation.ok}
		<div class="validation-bar">
			{#each taskStore.validation.errors as err}
				<span class="err-item">{err}</span>
			{/each}
		</div>
	{/if}

	<!-- Canvas -->
	<div class="canvas">
		<!-- Start Node -->
		<div class="node-card start-node">
			<div class="node-left">
				<div class="node-icon-circle" style:background="linear-gradient(135deg, var(--color-indigo), var(--color-purple))">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2"><circle cx="12" cy="12" r="10"/><polyline points="12 8 12 12 14 14"/></svg>
				</div>
			</div>
			<div class="node-body">
				<span class="node-label">开始</span>
				<span class="node-hint">用户输入参数</span>
			</div>
		</div>

		<!-- Connection -->
		<div class="conn">
			<div class="conn-line"></div>
			<button class="conn-add" onclick={() => handleAddStage(-1)} title="添加阶段">
				<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
			</button>
		</div>

		<!-- Stage Nodes -->
		{#each taskStore.definition?.stages ?? [] as stage, i (stage.id)}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="node-card stage-node"
				class:selected={selectedStageId === stage.id}
				class:drag-over={dragOverIndex === i}
				role="button"
				tabindex="0"
				draggable="true"
				onclick={() => handleSelectStage(stage.id)}
				onkeydown={(e) => e.key === 'Enter' && handleSelectStage(stage.id)}
				ondragstart={(e) => handleDragStart(e, stage.id)}
				ondragover={(e) => handleDragOver(e, i)}
				ondrop={(e) => handleDrop(e, i)}
				ondragleave={() => { dragOverIndex = null; }}
			>
				<div class="node-left">
					<div class="node-num" style:background={getRoleColor(stage.role)}>
						{i + 1}
					</div>
				</div>
				<div class="node-body">
					<div class="node-top-row">
						<span class="node-label">{stage.name || `阶段 ${i + 1}`}</span>
						<button class="node-del" onclick={(e) => { e.stopPropagation(); taskStore.removeStage(stage.id); }} title="删除">
							<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
						</button>
					</div>
					<div class="node-chips">
						<span class="chip" style:background="{getRoleColor(stage.role)}15" style:color={getRoleColor(stage.role)}>{stage.role}</span>
						{#if stage.agent_id}
							<span class="chip chip-accent">已绑定 Agent</span>
						{/if}
						{#if stage.tools.length}
							<span class="chip">{stage.tools.length} 个工具</span>
						{/if}
					</div>
					{#if stage.prompt_template}
						<span class="node-prompt">{stage.prompt_template.slice(0, 60)}{stage.prompt_template.length > 60 ? '...' : ''}</span>
					{/if}
				</div>
			</div>

			<!-- Connector with dependency label -->
			<div class="conn">
				<div class="conn-line"></div>
				{#if stage.depends_on.length > 0}
					<span class="conn-label">依赖: {stage.depends_on.length}</span>
				{/if}
				<button class="conn-add" onclick={() => handleAddStage(i)} title="添加阶段">
					<svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
				</button>
			</div>
		{/each}

		<!-- Empty state -->
		{#if !taskStore.definition?.stages.length}
			<div class="empty-canvas">
				<div class="empty-icon-wrap">
					<svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="var(--color-muted)" stroke-width="1.5">
						<rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="3" x2="9" y2="21"/><line x1="3" y1="9" x2="21" y2="9"/>
					</svg>
				</div>
				<p class="empty-text">还没有阶段</p>
				<button class="empty-btn" onclick={() => handleAddStage()}>+ 添加第一个阶段</button>
			</div>
		{/if}

		<!-- End Node -->
		{#if taskStore.definition?.stages.length}
			<div class="node-card end-node">
				<div class="node-left">
					<div class="node-icon-circle" style:background="linear-gradient(135deg, var(--color-green), var(--color-teal))">
						<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>
					</div>
				</div>
				<div class="node-body">
					<span class="node-label">输出</span>
					<span class="node-hint">最终结果</span>
				</div>
			</div>
		{/if}
	</div>
</div>

<!-- Inspector Panel -->
{#if selectedStage}
	<TaskNodeInspector stage={selectedStage} onClose={() => { selectedStageId = null; }} />
{/if}

<style>
	.canvas-wrapper {
		display: flex;
		flex-direction: column;
		flex: 1;
		overflow: hidden;
	}

	/* ── Toolbar ──────────────────────────────── */
	.toolbar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 16px;
		border-bottom: 1px solid var(--color-separator);
		gap: 10px;
		background: var(--color-bg-elevated);
	}

	.toolbar-left { flex: 1; min-width: 0; }
	.toolbar-right { display: flex; gap: 6px; flex-shrink: 0; }

	.name-input {
		width: 100%;
		padding: 6px 10px;
		border: 1px solid var(--color-separator);
		border-radius: 6px;
		background: var(--color-bg-elevated);
		color: var(--color-fg);
		font-size: 13px;
		font-weight: 500;
		outline: none;
	}
	.name-input:focus { border-color: var(--color-accent); box-shadow: 0 0 0 2px var(--color-focus-ring); }
	.name-input::placeholder { color: var(--color-muted); }

	.tb-btn {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 5px 10px;
		border-radius: 6px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg-elevated);
		color: var(--color-fg-secondary);
		font-size: 12px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.12s;
	}
	.tb-btn:hover { background: var(--color-bg-hover); color: var(--color-fg); }
	.tb-btn.run {
		background: var(--color-accent);
		color: #fff;
		border-color: var(--color-accent);
	}
	.tb-btn.run:hover { background: var(--color-accent-hover); }

	/* ── Validation ───────────────────────────── */
	.validation-bar {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
		padding: 6px 16px;
		background: color-mix(in srgb, var(--color-red) 6%, transparent);
		border-bottom: 1px solid color-mix(in srgb, var(--color-red) 25%, transparent);
	}
	.err-item {
		font-size: 11px;
		color: var(--color-red);
		background: var(--color-bg-elevated);
		padding: 2px 6px;
		border-radius: 4px;
	}

	/* ── Canvas ───────────────────────────────── */
	.canvas {
		flex: 1;
		overflow-y: auto;
		padding: 20px 16px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0;
		background: var(--color-bg-secondary);
	}

	/* ── Node Card ────────────────────────────── */
	.node-card {
		width: 100%;
		max-width: 420px;
		display: flex;
		align-items: flex-start;
		gap: 12px;
		padding: 14px 16px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-separator);
		border-radius: 10px;
		cursor: pointer;
		transition: all 0.15s;
		box-shadow: var(--shadow-sm);
	}
	.node-card:hover {
		border-color: var(--color-border-strong);
		box-shadow: var(--shadow-md);
	}
	.node-card.selected {
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px var(--color-focus-ring), var(--shadow-sm);
	}
	.node-card.drag-over {
		border-color: var(--color-accent);
		background: color-mix(in srgb, var(--color-accent) 6%, transparent);
	}

	.start-node, .end-node {
		cursor: default;
		background: var(--color-bg-elevated);
	}
	.start-node:hover, .end-node:hover {
		box-shadow: var(--shadow-sm);
	}

	.node-left {
		flex-shrink: 0;
	}

	.node-icon-circle {
		width: 36px;
		height: 36px;
		border-radius: 10px;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.node-num {
		width: 36px;
		height: 36px;
		border-radius: 10px;
		display: flex;
		align-items: center;
		justify-content: center;
		color: #fff;
		font-size: 14px;
		font-weight: 700;
	}

	.node-body {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 5px;
	}

	.node-top-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.node-label {
		font-size: 14px;
		font-weight: 600;
		color: var(--color-fg);
	}

	.node-hint {
		font-size: 12px;
		color: var(--color-muted);
	}

	.node-chips {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}

	.chip {
		padding: 2px 7px;
		border-radius: 5px;
		font-size: 11px;
		font-weight: 500;
		background: var(--color-bg-hover);
		color: var(--color-fg-secondary);
	}
	.chip-accent {
		background: color-mix(in srgb, var(--color-accent) 10%, transparent);
		color: var(--color-accent);
	}

	.node-prompt {
		font-size: 12px;
		color: var(--color-muted);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		margin-top: 2px;
	}

	.node-del {
		width: 22px;
		height: 22px;
		border-radius: 5px;
		border: none;
		background: transparent;
		color: var(--color-fg-tertiary);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		opacity: 0;
		transition: all 0.12s;
	}
	.node-card:hover .node-del { opacity: 1; }
	.node-del:hover {
		background: color-mix(in srgb, var(--color-red) 10%, transparent);
		color: var(--color-red);
	}

	/* ── Connection ───────────────────────────── */
	.conn {
		display: flex;
		flex-direction: column;
		align-items: center;
		position: relative;
		height: 32px;
	}

	.conn-line {
		width: 1px;
		height: 100%;
		background: var(--color-border-strong);
	}

	.conn-label {
		position: absolute;
		top: 50%;
		left: calc(50% + 10px);
		transform: translateY(-50%);
		font-size: 10px;
		color: var(--color-muted);
		background: var(--color-bg-hover);
		padding: 1px 5px;
		border-radius: 3px;
		white-space: nowrap;
	}

	.conn-add {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: 20px;
		height: 20px;
		border-radius: 50%;
		border: 1px solid var(--color-separator);
		background: var(--color-bg-elevated);
		color: var(--color-muted);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		opacity: 0;
		transition: all 0.15s;
		z-index: 1;
	}
	.conn:hover .conn-add { opacity: 1; }
	.conn-add:hover {
		background: var(--color-accent);
		color: #fff;
		border-color: var(--color-accent);
	}

	/* ── Empty ────────────────────────────────── */
	.empty-canvas {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 10px;
		padding: 40px 20px;
	}

	.empty-icon-wrap {
		width: 56px;
		height: 56px;
		border-radius: 14px;
		background: var(--color-bg-hover);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.empty-text {
		font-size: 13px;
		color: var(--color-muted);
		margin: 0;
	}

	.empty-btn {
		padding: 7px 14px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg-elevated);
		color: var(--color-fg);
		font-size: 12px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.12s;
	}
	.empty-btn:hover { background: var(--color-bg-hover); }
</style>
