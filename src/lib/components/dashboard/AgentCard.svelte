<script lang="ts">
	import type { AgentSummary } from '$lib/stores/dashboard.svelte';

	let { agent, onStartChat }: {
		agent: AgentSummary;
		onStartChat?: (agentId: string) => void;
	} = $props();

	function formatRelative(dateStr: string | null): string {
		if (!dateStr) return '从未使用';
		const d = new Date(dateStr);
		const now = new Date();
		const diff = now.getTime() - d.getTime();
		const mins = Math.floor(diff / 60_000);
		if (mins < 1) return '刚刚';
		if (mins < 60) return `${mins} 分钟前`;
		const hours = Math.floor(mins / 60);
		if (hours < 24) return `${hours} 小时前`;
		const days = Math.floor(hours / 24);
		return `${days} 天前`;
	}
</script>

<div class="agent-card">
	<div class="card-top">
		{#if agent.avatar}
			<img class="avatar-img" src={agent.avatar} alt={agent.name} />
		{:else}
			<img src="/icon.svg" alt="" class="avatar-icon" />
		{/if}
		<div class="card-info">
			<h4 class="card-name">{agent.name}</h4>
			{#if agent.description}
				<p class="card-desc">{agent.description}</p>
			{/if}
		</div>
	</div>

	<div class="card-meta">
		{#if agent.model_name}
			<span class="meta-tag model-tag">{agent.model_name}</span>
		{/if}
		<span class="meta-tag skill-tag">{agent.skill_count} 技能</span>
		<span class="meta-tag mcp-tag">{agent.mcp_count} MCP</span>
	</div>

	<div class="card-footer">
		<span class="last-used">{formatRelative(agent.last_used)}</span>
		<button class="start-btn" onclick={() => onStartChat?.(agent.id)}>
			<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
				<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
			</svg>
			开始对话
		</button>
	</div>
</div>

<style>
	.agent-card {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-md);
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 12px;
		transition: transform 0.15s var(--ease-default), box-shadow 0.15s ease;
	}

	.agent-card:hover {
		transform: translateY(-2px);
		box-shadow: var(--shadow-md);
	}

	.card-top {
		display: flex;
		align-items: flex-start;
		gap: 12px;
	}

	.avatar-img {
		width: 44px;
		height: 44px;
		border-radius: var(--radius-sm);
		object-fit: cover;
		flex-shrink: 0;
	}

	.avatar-icon {
		width: 40px;
		height: 40px;
		border-radius: 8px;
		flex-shrink: 0;
	}

	.card-info {
		flex: 1;
		min-width: 0;
	}

	.card-name {
		font-size: var(--text-headline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.card-desc {
		font-size: var(--text-caption1);
		color: var(--color-fg-secondary);
		margin: 2px 0 0;
		display: -webkit-box;
		line-clamp: 2;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.card-meta {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}

	.meta-tag {
		display: inline-flex;
		align-items: center;
		padding: 2px 8px;
		border-radius: 9999px;
		font-size: var(--text-caption2);
		font-weight: 500;
	}

	.model-tag {
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
		color: var(--color-accent);
	}

	.skill-tag {
		background: color-mix(in srgb, var(--color-green) 12%, transparent);
		color: var(--color-green);
	}

	.mcp-tag {
		background: color-mix(in srgb, var(--color-purple) 12%, transparent);
		color: var(--color-purple);
	}

	.card-footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-top: 8px;
		border-top: 1px solid var(--color-separator);
	}

	.last-used {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
	}

	.start-btn {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 14px;
		border-radius: 9999px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: var(--text-caption1);
		font-weight: 600;
		cursor: pointer;
		transition: background 0.15s ease, transform 0.1s ease;
	}

	.start-btn:hover {
		background: var(--color-accent-hover);
	}

	.start-btn:active {
		transform: scale(0.96);
	}
</style>
