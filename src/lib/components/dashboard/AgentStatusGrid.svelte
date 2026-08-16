<script lang="ts">
	import type { AgentSummary } from '$lib/stores/dashboard.svelte';

	interface Props {
		agents: AgentSummary[];
		onStartChat: (agentId: string) => void;
		onCreateAgent: () => void;
		onDeleteAgent?: (agentId: string) => void;
	}

	let { agents, onStartChat, onCreateAgent, onDeleteAgent }: Props = $props();

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

	function isActive(dateStr: string | null): boolean {
		if (!dateStr) return false;
		const d = new Date(dateStr);
		return Date.now() - d.getTime() < 30_000;
	}
</script>

{#if agents.length === 0}
	<div class="empty-state">
		<div class="empty-icon">
			<svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="var(--color-muted)" stroke-width="1.5">
				<path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/>
				<circle cx="9" cy="7" r="4"/>
				<path d="M23 21v-2a4 4 0 0 0-3-3.87"/>
				<path d="M16 3.13a4 4 0 0 1 0 7.75"/>
			</svg>
		</div>
		<p class="empty-text">还没有 Agent</p>
		<button class="empty-btn" onclick={onCreateAgent}>创建第一个 Agent</button>
	</div>
{:else}
	<div class="glass-container">
		<div class="header-row">
			<h2 class="header-title">Agents</h2>
			<button class="new-btn" onclick={onCreateAgent}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
					<line x1="12" y1="5" x2="12" y2="19"/>
					<line x1="5" y1="12" x2="19" y2="12"/>
				</svg>
				新建 Agent
			</button>
		</div>

		<div class="grid">
			{#each agents as agent (agent.id)}
				<div class="card" role="button" tabindex="0" onclick={() => onStartChat(agent.id)} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onStartChat(agent.id); } }}>
					<div class="card-top">
						<span class="status-dot" class:active={isActive(agent.last_used)}></span>
						{#if onDeleteAgent}
							<button
								class="menu-btn"
								title="删除"
								onclick={(e) => { e.stopPropagation(); if (confirm(`确定删除 Agent「${agent.name}」？此操作不可撤销。`)) { onDeleteAgent(agent.id); } }}
							>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<polyline points="3 6 5 6 21 6"/>
									<path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>
									<line x1="10" y1="11" x2="10" y2="17"/>
									<line x1="14" y1="11" x2="14" y2="17"/>
								</svg>
							</button>
						{/if}
					</div>

					<div class="card-info">
						<div class="avatar">
							{#if agent.avatar}
								<img src={agent.avatar} alt={agent.name} />
							{:else}
								<span class="avatar-fallback">{agent.name.charAt(0)}</span>
							{/if}
						</div>
						<span class="agent-name">{agent.name}</span>
					</div>

					{#if agent.model_name}
						<span class="tag model-tag">{agent.model_name}</span>
					{/if}

					<div class="meta-row">
						{#if agent.skill_count > 0}
							<span class="tag skill-tag">{agent.skill_count} 技能</span>
						{/if}
						{#if agent.mcp_count > 0}
							<span class="tag mcp-tag">{agent.mcp_count} MCP</span>
						{/if}
					</div>

					<span class="last-used">{formatRelative(agent.last_used)}</span>

					<button class="start-btn" onclick={(e) => { e.stopPropagation(); onStartChat(agent.id); }}>
						开始对话
					</button>
				</div>
			{/each}

			<button class="new-card" onclick={onCreateAgent}>
				<svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<line x1="12" y1="5" x2="12" y2="19"/>
					<line x1="5" y1="12" x2="19" y2="12"/>
				</svg>
				<span>新建 Agent</span>
			</button>
		</div>
	</div>
{/if}

<style>
	/* ── Empty state ──────────────────────────────── */
	.empty-state {
		background: var(--glass-solid-bg);
		backdrop-filter: var(--glass-solid-blur);
		-webkit-backdrop-filter: var(--glass-solid-blur);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		box-shadow: var(--glass-edge-highlight), var(--shadow-sm);
		padding: 48px 20px;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
	}
	.empty-icon {
		width: 64px;
		height: 64px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--color-muted) 8%, transparent);
		border-radius: 50%;
	}
	.empty-text {
		color: var(--color-fg-secondary);
		font-size: 14px;
		margin: 0;
	}
	.empty-btn {
		padding: 6px 16px;
		background: var(--color-accent);
		color: #fff;
		border: none;
		border-radius: 9999px;
		font-size: var(--text-caption1);
		font-weight: 600;
		cursor: pointer;
	}

	/* ── Glass container ──────────────────────────── */
	.glass-container {
		background: var(--glass-solid-bg);
		backdrop-filter: var(--glass-solid-blur);
		-webkit-backdrop-filter: var(--glass-solid-blur);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		box-shadow: var(--glass-edge-highlight), var(--shadow-sm);
		padding: 20px;
	}

	.header-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 16px;
	}
	.header-title {
		font-size: 15px;
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}
	.new-btn {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 5px 12px;
		border: 1px solid var(--color-separator);
		border-radius: 8px;
		background: transparent;
		color: var(--color-fg-secondary);
		font-size: var(--text-caption1);
		font-weight: 500;
		cursor: pointer;
		transition: border-color 0.15s ease, color 0.15s ease;
	}
	.new-btn:hover {
		border-color: var(--color-accent);
		color: var(--color-accent);
	}

	/* ── Grid ─────────────────────────────────────── */
	.grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
		gap: 12px;
	}

	@media (max-width: 600px) {
		.grid {
			grid-template-columns: 1fr;
		}
	}

	/* ── Agent card ───────────────────────────────── */
	.card {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-md);
		padding: 16px;
		display: flex;
		flex-direction: column;
		gap: 10px;
		cursor: pointer;
		transition: transform 0.15s var(--ease-default), box-shadow 0.15s ease;
	}
	.card:hover {
		transform: translateY(-2px);
		box-shadow: var(--shadow-md);
	}

	.card-top {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}
	.status-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--color-muted);
	}
	.status-dot.active {
		background: var(--color-green);
	}
	.menu-btn {
		width: 24px;
		height: 24px;
		display: flex;
		align-items: center;
		justify-content: center;
		border: none;
		background: transparent;
		color: var(--color-muted);
		border-radius: 6px;
		cursor: pointer;
		transition: color 0.15s ease, background 0.15s ease;
	}
	.menu-btn:hover {
		color: var(--color-fg);
		background: color-mix(in srgb, var(--color-fg) 8%, transparent);
	}

	.card-info {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.avatar {
		width: 44px;
		height: 44px;
		border-radius: 10px;
		overflow: hidden;
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.avatar img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}
	.avatar-fallback {
		font-size: 18px;
		font-weight: 600;
		color: var(--color-accent);
	}
	.agent-name {
		font-size: 15px;
		font-weight: 600;
		color: var(--color-fg);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	/* ── Tags ──────────────────────────────────────── */
	.tag {
		padding: 2px 8px;
		border-radius: 9999px;
		font-size: var(--text-caption2);
		font-weight: 500;
		display: inline-block;
		width: fit-content;
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

	.meta-row {
		display: flex;
		gap: 6px;
		flex-wrap: wrap;
	}

	.last-used {
		font-size: var(--text-caption2);
		color: var(--color-muted);
	}

	/* ── Start button ──────────────────────────────── */
	.start-btn {
		background: var(--color-accent);
		color: #fff;
		border: none;
		border-radius: 9999px;
		padding: 6px 14px;
		font-size: var(--text-caption1);
		font-weight: 600;
		cursor: pointer;
		align-self: flex-start;
		transition: opacity 0.15s ease;
	}
	.start-btn:hover {
		opacity: 0.85;
	}

	/* ── New card (dashed) ────────────────────────── */
	.new-card {
		border: 2px dashed var(--color-separator);
		border-radius: var(--radius-md);
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-height: 160px;
		color: var(--color-fg-secondary);
		background: transparent;
		cursor: pointer;
		gap: 8px;
		font-size: var(--text-caption1);
		font-weight: 500;
		transition: border-color 0.15s ease, color 0.15s ease;
	}
	.new-card:hover {
		border-color: var(--color-accent);
		color: var(--color-accent);
	}
</style>
