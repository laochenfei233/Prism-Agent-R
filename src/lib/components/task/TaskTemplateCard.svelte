<script lang="ts">
	import type { TaskTemplateSummary } from '$lib/stores/task.svelte';

	let { template, onSelect }: {
		template: TaskTemplateSummary;
		onSelect?: (template: TaskTemplateSummary) => void;
	} = $props();
</script>

<button class="template-card" onclick={() => onSelect?.(template)}>
	<div class="card-icon">
		<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
			<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
			<polyline points="14 2 14 8 20 8"/>
			<line x1="16" y1="13" x2="8" y2="13"/>
			<line x1="16" y1="17" x2="8" y2="17"/>
			<polyline points="10 9 9 9 8 9"/>
		</svg>
	</div>
	<div class="card-body">
		<h4>{template.name}</h4>
		<p>{template.description}</p>
	</div>
	<div class="card-meta">
		<span class="meta-badge">{template.stage_count} 阶段</span>
		{#if template.input_count > 0}
			<span class="meta-badge">{template.input_count} 输入</span>
		{/if}
	</div>
</button>

<style>
	.template-card {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		background: var(--color-bg);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		cursor: pointer;
		text-align: left;
		transition: border-color 0.15s ease, box-shadow 0.15s ease;
		width: 100%;
	}
	.template-card:hover {
		border-color: var(--color-accent);
		box-shadow: 0 0 0 1px var(--color-accent);
	}

	.card-icon {
		color: var(--color-accent);
	}

	.card-body h4 {
		font-size: var(--text-subheadline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0 0 4px;
	}

	.card-body p {
		font-size: var(--text-caption1);
		color: var(--color-fg-secondary);
		margin: 0;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.card-meta {
		display: flex;
		gap: var(--spacing-xs);
		flex-wrap: wrap;
	}

	.meta-badge {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
		background: var(--color-bg-secondary);
		padding: 2px 8px;
		border-radius: var(--radius-full);
	}
</style>
