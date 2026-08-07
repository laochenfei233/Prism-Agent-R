<script lang="ts">
	let { template, onSelect }: {
		template: { id: string; name: string; description: string; stage_count: number; icon?: string };
		onSelect?: (template: any) => void;
	} = $props();
</script>

<button class="template-card" onclick={() => onSelect?.(template)}>
	<span class="card-icon">{template.icon || '⚡'}</span>
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
		background: #f7f7f8;
		border: 1px solid rgba(0, 0, 0, 0.04);
		border-radius: 10px;
		cursor: pointer;
		text-align: left;
		transition: all 0.12s;
		width: 100%;
	}
	.template-card:hover {
		background: #f0f0f0;
		border-color: rgba(0, 0, 0, 0.08);
	}

	.card-icon {
		font-size: 20px;
		flex-shrink: 0;
		width: 32px;
		text-align: center;
	}

	.card-body {
		flex: 1;
		min-width: 0;
	}

	.card-body h4 {
		font-size: 13px;
		font-weight: 600;
		color: #171717;
		margin: 0;
	}

	.card-body p {
		font-size: 12px;
		color: #6b6b6b;
		margin: 2px 0 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.card-stages {
		font-size: 11px;
		color: #a0a0a0;
		flex-shrink: 0;
	}
</style>
