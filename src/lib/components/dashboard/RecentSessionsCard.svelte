<script lang="ts">
	import type { SessionSummary } from '$lib/stores/dashboard.svelte';

	let { sessions, onOpenSession }: {
		sessions: SessionSummary[];
		onOpenSession?: (sessionId: string) => void;
	} = $props();

	function formatTime(dateStr: string): string {
		const d = new Date(dateStr);
		const now = new Date();
		const diff = now.getTime() - d.getTime();
		const mins = Math.floor(diff / 60_000);
		if (mins < 1) return '刚刚';
		if (mins < 60) return `${mins} 分钟前`;
		const hours = Math.floor(mins / 60);
		if (hours < 24) return `${hours} 小时前`;
		const days = Math.floor(hours / 24);
		if (days < 7) return `${days} 天前`;
		return `${d.getMonth() + 1}/${d.getDate()}`;
	}

	const agentColors: Record<string, string> = {};
	const palette = ['#FF6900', '#34C759', '#FF9500', '#AF52DE', '#FF3B30', '#5AC8FA', '#5856D6'];

	function agentColor(name: string): string {
		if (!agentColors[name]) {
			agentColors[name] = palette[Object.keys(agentColors).length % palette.length];
		}
		return agentColors[name];
	}
</script>

<div class="sessions-card">
	<div class="card-header">
		<h3>最近会话</h3>
	</div>

	{#if sessions.length === 0}
		<div class="empty">暂无会话记录</div>
	{:else}
		<div class="session-list">
			{#each sessions as session}
				<button class="session-row" onclick={() => onOpenSession?.(session.id)}>
					<div class="session-dot" style:background={agentColor(session.agent_name)}></div>
					<div class="session-content">
						<div class="session-title">{session.title || '新会话'}</div>
						<div class="session-meta">
							<span class="session-agent">{session.agent_name}</span>
							<span class="session-sep">·</span>
							<span class="session-time">{formatTime(session.updated_at)}</span>
						</div>
					</div>
					<span class="session-count">{session.message_count} 条</span>
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.sessions-card {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-md);
		padding: 16px;
	}

	.card-header {
		margin-bottom: 12px;
	}

	.card-header h3 {
		font-size: var(--text-headline);
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.session-list {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.session-row {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 12px;
		border-radius: var(--radius-sm);
		border: none;
		background: transparent;
		cursor: pointer;
		text-align: left;
		width: 100%;
		transition: background 0.12s ease;
	}

	.session-row:hover {
		background: var(--color-bg);
	}

	.session-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.session-content {
		flex: 1;
		min-width: 0;
	}

	.session-title {
		font-size: var(--text-subheadline);
		font-weight: 500;
		color: var(--color-fg);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.session-meta {
		display: flex;
		align-items: center;
		gap: 4px;
		margin-top: 2px;
	}

	.session-agent {
		font-size: var(--text-caption2);
		color: var(--color-fg-secondary);
	}

	.session-sep {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
	}

	.session-time {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
	}

	.session-count {
		font-size: var(--text-caption2);
		color: var(--color-fg-tertiary);
		flex-shrink: 0;
	}

	.empty {
		padding: 24px;
		text-align: center;
		color: var(--color-fg-tertiary);
		font-size: var(--text-subheadline);
	}
</style>
