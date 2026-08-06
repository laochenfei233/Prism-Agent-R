<script lang="ts">
	import type { AgentContext } from '$lib/stores/context.svelte';

	let { data }: { data: AgentContext } = $props();

	const workspace = $derived(data.workspace);
</script>

<div class="workdir-panel">
	<!-- Current Directory -->
	<div class="section">
		<div class="section-label">当前目录</div>
		<div class="dir-path">
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
			</svg>
			<span class="path-text">{workspace.current_dir}</span>
		</div>
	</div>

	<!-- Binding Status -->
	<div class="section">
		<div class="section-label">绑定状态</div>
		{#if workspace.bound_agent_id}
			<div class="badge badge-active">
				<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<polyline points="20 6 9 17 4 12"></polyline>
				</svg>
				已绑定: {workspace.bound_agent_id}
			</div>
		{:else}
			<div class="badge badge-inactive">
				未绑定
			</div>
		{/if}
	</div>

	<!-- Recent Directories -->
	{#if workspace.recent_dirs.length > 0}
		<div class="section">
			<div class="section-label">最近目录</div>
			<div class="recent-list">
				{#each workspace.recent_dirs as dir}
					<div class="recent-item">
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<circle cx="12" cy="12" r="1"></circle>
						</svg>
						<span class="recent-path">{dir}</span>
					</div>
				{/each}
			</div>
		</div>
	{/if}
</div>

<style>
	.workdir-panel {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.section-label {
		font-size: 11px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.dir-path {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 10px 12px;
		background: var(--color-bg);
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		color: var(--color-fg);
	}

	.path-text {
		font-size: 13px;
		font-family: var(--font-mono);
		word-break: break-all;
	}

	.badge {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 10px;
		border-radius: 6px;
		font-size: 12px;
		font-weight: 500;
	}

	.badge-active {
		background: rgba(16, 185, 129, 0.1);
		color: var(--color-green, #10b981);
	}

	.badge-inactive {
		background: var(--color-bg);
		color: var(--color-fg-secondary);
		border: 1px solid var(--color-separator);
	}

	.recent-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.recent-item {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 10px;
		border-radius: 6px;
		font-size: 12px;
		color: var(--color-fg-secondary);
		cursor: default;
		transition: background 0.15s ease;
	}
	.recent-item:hover {
		background: var(--color-bg-tertiary);
	}

	.recent-path {
		font-family: var(--font-mono);
		word-break: break-all;
	}
</style>
