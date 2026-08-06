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
		if (mins < 1) return 'just now';
		if (mins < 60) return `${mins}m ago`;
		const hours = Math.floor(mins / 60);
		if (hours < 24) return `${hours}h ago`;
		const days = Math.floor(hours / 24);
		if (days < 7) return `${days}d ago`;
		return `${d.getMonth() + 1}/${d.getDate()}`;
	}

	const palette = ['#FF6900', '#34C759', '#FF9500', '#AF52DE', '#FF3B30', '#5AC8FA', '#5856D6'];
	const colorMap: Record<string, string> = {};
	let colorIdx = 0;

	function agentColor(name: string): string {
		if (!colorMap[name]) {
			colorMap[name] = palette[colorIdx % palette.length];
			colorIdx++;
		}
		return colorMap[name];
	}
</script>

<div class="sessions-card">
	<div class="card-header">
		<h3>Recent Sessions</h3>
	</div>
	{#if sessions.length === 0}
		<div class="empty">No sessions yet</div>
	{:else}
		<div class="session-list">
			{#each sessions as session}
				<button class="session-row" onclick={() => onOpenSession?.(session.id)}>
					<span class="dot" style:background={agentColor(session.agent_name)}></span>
					<div class="content">
						<span class="title">{session.title || 'New session'}</span>
						<span class="meta">{session.agent_name} · {formatTime(session.updated_at)}</span>
					</div>
				</button>
			{/each}
		</div>
	{/if}
</div>

<style>
	.sessions-card {
		background: #f7f7f8;
		border: 1px solid rgba(0, 0, 0, 0.06);
		border-radius: 12px;
		padding: 20px;
	}

	.card-header {
		margin-bottom: 12px;
	}

	.card-header h3 {
		font-size: 15px;
		font-weight: 600;
		color: #171717;
		margin: 0;
	}

	.session-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.session-row {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 10px;
		border-radius: 8px;
		border: none;
		background: transparent;
		cursor: pointer;
		text-align: left;
		width: 100%;
		transition: background 0.12s ease;
	}

	.session-row:hover {
		background: rgba(0, 0, 0, 0.04);
	}

	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		flex-shrink: 0;
	}

	.content {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.title {
		font-size: 13px;
		font-weight: 500;
		color: #171717;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.meta {
		font-size: 12px;
		color: #a0a0a0;
	}

	.empty {
		padding: 20px;
		text-align: center;
		color: #a0a0a0;
		font-size: 13px;
	}
</style>
