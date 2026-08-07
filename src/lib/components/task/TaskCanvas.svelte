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

	function statusColor(_stageId: string): string {
		return '#d4d4d4';
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
				placeholder="Workflow name"
			/>
		</div>
		<div class="toolbar-right">
			<button class="tb-btn" onclick={taskStore.validate}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M20 6L9 17l-5-5"/></svg>
				Validate
			</button>
			<button class="tb-btn" onclick={taskStore.saveTemplate}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z"/><polyline points="17 21 17 13 7 13 7 21"/></svg>
				Save
			</button>
			<button class="tb-btn primary" onclick={() => taskStore.startRun()}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21"/></svg>
				Run
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
		<div class="node start-node">
			<div class="node-icon start">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="16"/><line x1="8" y1="12" x2="16" y2="12"/></svg>
			</div>
			<div class="node-info">
				<span class="node-title">Start</span>
				<span class="node-sub">User Input</span>
			</div>
		</div>

		<!-- Connection line -->
		<div class="connector">
			<div class="line"></div>
			<button class="add-btn" onclick={() => handleAddStage(-1)} title="Add stage">
				<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
			</button>
		</div>

		<!-- Stage Nodes -->
		{#each taskStore.definition?.stages ?? [] as stage, i (stage.id)}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="node stage-node"
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
				<div class="node-status" style:background={statusColor(stage.id)}></div>
				<div class="node-icon stage">
					<span class="stage-num">{i + 1}</span>
				</div>
				<div class="node-info">
					<span class="node-title">{stage.name || `Stage ${i + 1}`}</span>
					<div class="node-tags">
						{#if stage.role}<span class="tag">{stage.role}</span>{/if}
						{#if stage.agent_id}<span class="tag accent">Agent</span>{/if}
						{#if stage.tools.length}<span class="tag">{stage.tools.length} tools</span>{/if}
						<span class="tag muted">{stage.max_iterations} max turns</span>
					</div>
					{#if stage.prompt_template}
						<span class="node-preview">{stage.prompt_template.slice(0, 80)}{stage.prompt_template.length > 80 ? '...' : ''}</span>
					{/if}
				</div>
				<button class="node-delete" onclick={(e) => { e.stopPropagation(); taskStore.removeStage(stage.id); }} title="Remove">
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
				</button>
			</div>

			<!-- Connector after each stage -->
			<div class="connector">
				<div class="line"></div>
				<button class="add-btn" onclick={() => handleAddStage(i)} title="Add stage">
					<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
				</button>
			</div>
		{/each}

		<!-- Empty state -->
		{#if !taskStore.definition?.stages.length}
			<div class="empty-canvas">
				<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" opacity="0.3">
					<rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="3" x2="9" y2="21"/><line x1="3" y1="9" x2="21" y2="9"/>
				</svg>
				<p>No stages yet</p>
				<button class="add-first" onclick={() => handleAddStage()}>Add first stage</button>
			</div>
		{/if}

		<!-- End Node -->
		{#if taskStore.definition?.stages.length}
			<div class="node end-node">
				<div class="node-icon end">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 12l2 2 4-4"/></svg>
				</div>
				<div class="node-info">
					<span class="node-title">Output</span>
					<span class="node-sub">Final result</span>
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
		padding: 12px 20px;
		border-bottom: 1px solid rgba(0, 0, 0, 0.06);
		gap: 12px;
	}

	.toolbar-left { flex: 1; min-width: 0; }
	.toolbar-right { display: flex; gap: 6px; flex-shrink: 0; }

	.name-input {
		width: 100%;
		padding: 7px 12px;
		border: 1px solid rgba(0, 0, 0, 0.08);
		border-radius: 8px;
		background: #fff;
		color: #171717;
		font-size: 14px;
		font-weight: 500;
		outline: none;
		transition: border-color 0.15s;
	}
	.name-input:focus { border-color: #FF6900; }
	.name-input::placeholder { color: #c0c0c0; }

	.tb-btn {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		padding: 6px 12px;
		border-radius: 8px;
		border: 1px solid rgba(0, 0, 0, 0.08);
		background: #fff;
		color: #171717;
		font-size: 12px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.12s;
	}
	.tb-btn:hover { background: #f5f5f5; }
	.tb-btn.primary {
		background: #FF6900;
		color: #fff;
		border-color: #FF6900;
	}
	.tb-btn.primary:hover { background: #E85D00; }

	/* ── Validation ───────────────────────────── */
	.validation-bar {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
		padding: 8px 20px;
		background: #fef2f2;
		border-bottom: 1px solid #fecaca;
	}
	.err-item {
		font-size: 12px;
		color: #dc2626;
		background: #fff;
		padding: 2px 8px;
		border-radius: 6px;
		border: 1px solid #fecaca;
	}

	/* ── Canvas ───────────────────────────────── */
	.canvas {
		flex: 1;
		overflow-y: auto;
		padding: 24px 20px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0;
	}

	/* ── Nodes ────────────────────────────────── */
	.node {
		width: 100%;
		max-width: 440px;
		display: flex;
		align-items: flex-start;
		gap: 12px;
		padding: 14px 16px;
		background: #fff;
		border: 1px solid rgba(0, 0, 0, 0.08);
		border-radius: 12px;
		cursor: pointer;
		transition: all 0.15s;
		position: relative;
	}
	.node:hover { border-color: rgba(0, 0, 0, 0.15); }
	.node.selected {
		border-color: #FF6900;
		box-shadow: 0 0 0 3px rgba(255, 105, 0, 0.1);
	}
	.node.drag-over {
		border-color: #FF6900;
		background: #fff8f0;
	}

	.start-node, .end-node {
		cursor: default;
		background: #f9f9f9;
	}
	.start-node:hover, .end-node:hover { border-color: rgba(0, 0, 0, 0.08); }

	.node-status {
		position: absolute;
		left: 0;
		top: 12px;
		bottom: 12px;
		width: 3px;
		border-radius: 0 2px 2px 0;
	}

	.node-icon {
		width: 36px;
		height: 36px;
		border-radius: 10px;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.node-icon.start { background: #f0f0f0; color: #6b6b6b; }
	.node-icon.end { background: #f0f0f0; color: #6b6b6b; }
	.node-icon.stage { background: #FF6900; color: #fff; }

	.stage-num {
		font-size: 14px;
		font-weight: 700;
	}

	.node-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.node-title {
		font-size: 14px;
		font-weight: 600;
		color: #171717;
	}

	.node-sub {
		font-size: 12px;
		color: #a0a0a0;
	}

	.node-tags {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}

	.tag {
		padding: 2px 7px;
		border-radius: 5px;
		font-size: 11px;
		font-weight: 500;
		background: #f0f0f0;
		color: #6b6b6b;
	}
	.tag.accent { background: #fff0e6; color: #FF6900; }
	.tag.muted { background: transparent; color: #a0a0a0; }

	.node-preview {
		font-size: 12px;
		color: #a0a0a0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		margin-top: 2px;
	}

	.node-delete {
		position: absolute;
		top: 8px;
		right: 8px;
		width: 24px;
		height: 24px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: #c0c0c0;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		opacity: 0;
		transition: all 0.12s;
	}
	.node:hover .node-delete { opacity: 1; }
	.node-delete:hover { background: #fee2e2; color: #dc2626; }

	/* ── Connector ────────────────────────────── */
	.connector {
		display: flex;
		flex-direction: column;
		align-items: center;
		position: relative;
		height: 36px;
	}

	.line {
		width: 1px;
		height: 100%;
		background: rgba(0, 0, 0, 0.12);
	}

	.add-btn {
		position: absolute;
		top: 50%;
		left: 50%;
		transform: translate(-50%, -50%);
		width: 22px;
		height: 22px;
		border-radius: 50%;
		border: 1px solid rgba(0, 0, 0, 0.1);
		background: #fff;
		color: #a0a0a0;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		opacity: 0;
		transition: all 0.15s;
		z-index: 1;
	}
	.connector:hover .add-btn { opacity: 1; }
	.add-btn:hover {
		background: #FF6900;
		color: #fff;
		border-color: #FF6900;
	}

	/* ── Empty ────────────────────────────────── */
	.empty-canvas {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
		padding: 48px 20px;
	}
	.empty-canvas p {
		font-size: 14px;
		color: #a0a0a0;
		margin: 0;
	}
	.add-first {
		padding: 8px 16px;
		border-radius: 8px;
		border: 1px solid rgba(0, 0, 0, 0.1);
		background: #fff;
		color: #171717;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
	}
	.add-first:hover { background: #f5f5f5; }
</style>
