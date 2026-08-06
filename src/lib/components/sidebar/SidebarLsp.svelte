<script lang="ts">
	import type { AgentContext } from '$lib/stores/context.svelte';

	let { data }: { data: AgentContext } = $props();

	const servers = $derived(data.lsp);

	function statusColor(status: string): string {
		if (status === 'running') return 'var(--color-green, #10b981)';
		if (status === 'error') return 'var(--color-red, #ef4444)';
		return 'var(--color-fg-secondary)';
	}
</script>

<div class="lsp-panel">
	{#if servers.length === 0}
		<div class="empty">
			<span>无 LSP 服务器</span>
		</div>
	{:else}
		<div class="server-list">
			{#each servers as server}
				<div class="server-item">
					<div class="server-header">
						<div class="status-dot" style:background={statusColor(server.status)}></div>
						<div class="server-info">
							<span class="server-name">{server.id}</span>
							<span class="server-cmd">{server.cmd}</span>
						</div>
					</div>

					<div class="server-detail">
						<div class="lang-tags">
							{#each server.langs as lang}
								<span class="lang-tag">{lang}</span>
							{/each}
						</div>

						<div class="detail-row">
							<span class="detail-label">状态</span>
							<span class="detail-value">{server.status}</span>
						</div>

						{#if server.index_file_count !== null}
							<div class="detail-row">
								<span class="detail-label">索引文件</span>
								<span class="detail-value">{server.index_file_count}</span>
							</div>
						{/if}

						{#if server.last_error}
							<div class="error-msg">{server.last_error}</div>
						{/if}

						{#if server.install_hint}
							<div class="install-hint">{server.install_hint}</div>
						{/if}
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.lsp-panel {
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

	.server-list {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.server-item {
		background: var(--color-bg);
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		overflow: hidden;
	}

	.server-header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 10px 12px;
	}

	.status-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.server-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.server-name {
		font-size: 13px;
		font-weight: 500;
		color: var(--color-fg);
	}

	.server-cmd {
		font-size: 11px;
		font-family: var(--font-mono);
		color: var(--color-fg-secondary);
	}

	.server-detail {
		padding: 0 12px 12px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.lang-tags {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}

	.lang-tag {
		padding: 2px 8px;
		border-radius: 4px;
		font-size: 11px;
		font-weight: 500;
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
		border: 1px solid var(--color-separator);
	}

	.detail-row {
		display: flex;
		justify-content: space-between;
		font-size: 12px;
	}

	.detail-label {
		color: var(--color-fg-secondary);
	}

	.detail-value {
		color: var(--color-fg);
		font-weight: 500;
	}

	.error-msg {
		font-size: 12px;
		color: var(--color-red, #ef4444);
		padding: 6px 8px;
		background: rgba(239, 68, 68, 0.08);
		border-radius: 4px;
		word-break: break-all;
	}

	.install-hint {
		font-size: 11px;
		color: var(--color-fg-secondary);
		padding: 6px 8px;
		background: var(--color-bg-secondary);
		border-radius: 4px;
		font-style: italic;
	}
</style>
