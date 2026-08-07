<script lang="ts">
	import type { AgentContext } from '$lib/stores/context.svelte';

	let { data }: { data: AgentContext } = $props();

	const instructions = $derived(data.instructions);
	const sorted = $derived([...instructions].sort((a, b) => b.priority - a.priority));
</script>

<div class="instructions-panel">
	{#if sorted.length === 0}
		<div class="empty">
			<span>无指令文件</span>
		</div>
	{:else}
		<div class="file-list">
			{#each sorted as file}
				<div class="file-item" class:injected={file.injected}>
					<div class="file-header">
						<div class="file-icon">
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
								<polyline points="14 2 14 8 20 8"/>
							</svg>
						</div>
						<div class="file-info">
							<span class="file-name">{file.name}</span>
							<span class="file-meta">{file.lines} 行 · 优先级 {file.priority}</span>
						</div>
						<div class="inject-status" class:active={file.injected}>
							{#if file.injected}
								<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<polyline points="20 6 9 17 4 12"></polyline>
								</svg>
								注入
							{:else}
								排除
							{/if}
						</div>
					</div>
					<div class="file-path">{file.path}</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.instructions-panel {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.empty {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 32px 0;
		font-size: 13px;
		color: var(--color-fg-secondary);
	}

	.file-list {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.file-item {
		padding: 10px 12px;
		background: var(--color-bg);
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		transition: border-color 0.15s ease;
	}
	.file-item:hover {
		border-color: var(--color-accent);
	}
	.file-item.injected {
		border-left: 3px solid var(--color-accent);
	}

	.file-header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.file-icon {
		color: var(--color-fg-secondary);
		flex-shrink: 0;
	}

	.file-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.file-name {
		font-size: 13px;
		font-weight: 500;
		color: var(--color-fg);
	}

	.file-meta {
		font-size: 11px;
		color: var(--color-fg-secondary);
	}

	.inject-status {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 3px 8px;
		border-radius: 4px;
		font-size: 11px;
		font-weight: 500;
		color: var(--color-fg-secondary);
		background: var(--color-bg-secondary);
		flex-shrink: 0;
	}
	.inject-status.active {
		color: var(--color-green, #10b981);
		background: rgba(16, 185, 129, 0.1);
	}

	.file-path {
		margin-top: 6px;
		font-size: 11px;
		font-family: var(--font-mono);
		color: var(--color-fg-secondary);
		word-break: break-all;
	}
</style>
