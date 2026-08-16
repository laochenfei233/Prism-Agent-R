<script lang="ts">
	type ModelOption = {
		id: string;
		model_id: string;
		display_name?: string | null;
	};

	let {
		modelId = null,
		models = [],
		onSelect
	}: {
		modelId?: string | null;
		models?: ModelOption[];
		onSelect?: (modelId: string) => void;
	} = $props();

	let open = $state(false);
	let containerEl = $state<HTMLDivElement | null>(null);

	const current = $derived(
		models.find((m) => m.id === modelId) ?? models.find((m) => m.model_id === modelId)
	);

	function label(): string {
		return current?.display_name || current?.model_id || '未配置模型';
	}

	function select(option: ModelOption) {
		console.log('[ModelSelector] select: option.id=', option.id, 'option.model_id=', option.model_id);
		open = false;
		onSelect?.(option.id);
	}

	$effect(() => {
		if (!open) return;
		function onClick(e: MouseEvent) {
			if (containerEl && !containerEl.contains(e.target as Node)) open = false;
		}
		document.addEventListener('click', onClick);
		return () => document.removeEventListener('click', onClick);
	});
</script>

<div class="model-selector" bind:this={containerEl}>
	{#if models.length > 0}
		<button class="model-chip" onclick={() => (open = !open)} title="切换模型">
			<span class="model-dot"></span>
			<span class="model-label">{label()}</span>
			<span class="chevron" class:expanded={open}>{open ? '▾' : '▸'}</span>
		</button>
		{#if open}
			<div class="model-dropdown">
				{#each models as option}
					<button
						class="model-option"
						class:active={option.id === modelId}
						onclick={() => select(option)}
					>
						<span class="option-name">{option.display_name || option.model_id}</span>
						<span class="option-meta">{option.model_id}</span>
					</button>
				{/each}
			</div>
		{/if}
	{:else}
		<div class="model-chip static" title="未配置模型">
			<span class="model-dot"></span>
			<span class="model-label">{label()}</span>
		</div>
	{/if}
</div>

<style>
	.model-selector {
		position: relative;
	}

	.model-chip {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 5px 10px;
		border-radius: var(--radius-pill);
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
		font-size: var(--text-footnote);
		cursor: pointer;
		transition: all 0.12s;
		max-width: 220px;
	}
	.model-chip:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-fg);
	}
	.model-chip.static {
		cursor: default;
	}

	.model-dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		background: var(--color-accent);
		flex-shrink: 0;
	}

	.model-label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.chevron {
		font-size: 10px;
		color: var(--color-fg-tertiary);
		transition: transform var(--duration-fast) var(--ease-default);
	}
	.chevron.expanded {
		transform: rotate(180deg);
	}

	.model-dropdown {
		position: absolute;
		top: calc(100% + 4px);
		right: 0;
		min-width: 240px;
		max-width: 320px;
		max-height: 280px;
		overflow-y: auto;
		background: var(--color-glass);
		backdrop-filter: saturate(180%) blur(20px);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-md);
		z-index: 100;
		padding: 4px;
	}

	.model-option {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 1px;
		width: 100%;
		text-align: left;
		padding: 7px 10px;
		border: none;
		background: transparent;
		border-radius: var(--radius-sm);
		cursor: pointer;
		font-size: var(--text-footnote);
		color: var(--color-fg);
	}
	.model-option:hover {
		background: var(--color-bg-secondary);
	}
	.model-option.active {
		color: var(--color-accent);
	}

	.option-name {
		font-weight: var(--font-weight-medium);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		width: 100%;
	}

	.option-meta {
		font-size: 11px;
		color: var(--color-fg-tertiary);
		font-family: var(--font-mono);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		width: 100%;
	}
</style>
