<script lang="ts">
	let { template, onSelect }: {
		template: { id: string; name: string; description: string; stage_count: number; icon?: string };
		onSelect?: (template: any) => void;
	} = $props();
</script>

<button class="template-card" onclick={() => onSelect?.(template)}>
	<span class="card-icon">
		{#if template.icon && template.icon.length > 3}
			<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d={template.icon}/></svg>
		{:else}
			<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z"/></svg>
		{/if}
	</span>
	<div class="card-body">
		<h4>{template.name}</h4>
		<p>{template.description}</p>
	</div>
	<span class="card-stages">{template.stage_count} 步</span>
</button>

<style>
	.template-card {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 14px;
		background: var(--color-bg-hover);
		border: 1px solid var(--color-separator);
		border-radius: 10px;
		cursor: pointer;
		text-align: left;
		transition: all 0.12s;
		width: 100%;
	}
	.template-card:hover {
		background: var(--color-bg-elevated);
		border-color: var(--color-border-strong);
	}

	.card-icon {
		font-size: 20px;
		flex-shrink: 0;
		width: 32px;
		text-align: center;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		color: var(--color-accent);
	}

	.card-body {
		flex: 1;
		min-width: 0;
	}

	.card-body h4 {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.card-body p {
		font-size: 12px;
		color: var(--color-fg-secondary);
		margin: 2px 0 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.card-stages {
		font-size: 11px;
		color: var(--color-muted);
		flex-shrink: 0;
	}
</style>
