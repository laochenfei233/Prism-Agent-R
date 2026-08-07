<script lang="ts">
	import type { AgentSummary } from '$lib/stores/dashboard.svelte';
	import AgentCard from './AgentCard.svelte';

	let { agents, onStartChat, onCreateAgent, onOpenMarket }: {
		agents: AgentSummary[];
		onStartChat?: (agentId: string) => void;
		onCreateAgent?: () => void;
		onOpenMarket?: () => void;
	} = $props();
</script>

<div class="agent-launcher">
	<div class="launcher-header">
		<h2>Agent</h2>
		<div class="launcher-actions">
			<button class="action-btn" onclick={onCreateAgent}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
				</svg>
				新建
			</button>
			<button class="action-btn secondary" onclick={onOpenMarket}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>
				</svg>
				市场
			</button>
		</div>
	</div>

	{#if agents.length === 0}
		<div class="empty-state">
			<div class="empty-icon">
				<svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
					<circle cx="12" cy="7" r="4"/>
				</svg>
			</div>
			<p>还没有 Agent</p>
			<button class="create-btn" onclick={onCreateAgent}>创建第一个 Agent</button>
		</div>
	{:else}
		<div class="agent-grid">
			{#each agents as agent (agent.id)}
				<AgentCard {agent} {onStartChat} />
			{/each}
		</div>
	{/if}
</div>

<style>
	.agent-launcher {
		background: #f7f7f8;
		border: 1px solid rgba(0, 0, 0, 0.06);
		border-radius: 12px;
		padding: 20px;
		min-height: 200px;
	}

	.launcher-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 16px;
	}

	.launcher-header h2 {
		font-size: 15px;
		font-weight: 600;
		color: #171717;
		margin: 0;
	}

	.launcher-actions {
		display: flex;
		gap: 8px;
	}

	.action-btn {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		padding: 6px 14px;
		border-radius: 8px;
		border: 1px solid rgba(0, 0, 0, 0.1);
		background: #fff;
		color: #171717;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.15s ease;
	}

	.action-btn:hover {
		background: #efefef;
		border-color: rgba(0, 0, 0, 0.15);
	}

	.action-btn.secondary {
		border-color: rgba(0, 0, 0, 0.06);
		color: #6b6b6b;
	}

	.action-btn.secondary:hover {
		background: #f0f0f0;
		color: #171717;
	}

	.agent-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
		gap: 10px;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 40px 16px;
		gap: 12px;
	}

	.empty-icon {
		color: #c5c5c5;
	}

	.empty-state p {
		font-size: 14px;
		color: #6b6b6b;
		margin: 0;
	}

	.create-btn {
		padding: 8px 18px;
		border-radius: 8px;
		border: none;
		background: #171717;
		color: #fff;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition: background 0.15s ease;
	}

	.create-btn:hover {
		background: #404040;
	}
</style>
