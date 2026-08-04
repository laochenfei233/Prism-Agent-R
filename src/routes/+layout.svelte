<script lang="ts">
	import { goto } from '$app/navigation';
	import { agentStore } from '$lib/stores/agents.svelte';
	import { chatStore } from '$lib/stores/chat.svelte';
	import type { AgentDto, SessionDto } from '$lib/api';

	let { children } = $props();

	let newAgentName = $state('');
	let showNewAgent = $state(false);

	$effect(() => {
		agentStore.loadAgents();
	});

	async function handleCreateAgent() {
		if (!newAgentName.trim()) return;
		await agentStore.createAgent(newAgentName.trim());
		newAgentName = '';
		showNewAgent = false;
	}

	async function handleNewSession(agent: AgentDto) {
		console.log('Creating session for agent:', agent.id, agent.name);
		try {
			agentStore.selectAgent(agent);
			const session = await agentStore.createSession(agent.id, '新会话');
			console.log('Session created:', session);
			if (agentStore.currentSession) {
				chatStore.loadHistory(agentStore.currentSession.id);
			}
		} catch (e) {
			console.error('Failed to create session:', e);
			alert('创建会话失败: ' + e);
		}
	}

	function handleSelectSession(session: SessionDto) {
		agentStore.selectSession(session);
		chatStore.loadHistory(session.id);
	}
</script>

<div class="app">
	<!-- Sidebar -->
	<aside class="sidebar">
		<!-- Sidebar Header -->
		<div class="sidebar-header">
			<div class="logo">
				<img src="/icon.svg" alt="" width="24" height="24" />
				<span class="logo-text">Prism</span>
			</div>
			<button class="icon-btn" onclick={() => goto('/settings')} title="设置">
				<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
					<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
				</svg>
			</button>
		</div>

		<!-- Agent Section -->
		<div class="section">
			<div class="section-header">
				<span class="section-title">Agent</span>
				<button class="icon-btn-sm" onclick={() => showNewAgent = !showNewAgent}>
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
					</svg>
				</button>
			</div>

			{#if showNewAgent}
				<div class="new-form">
					<input
						type="text"
						placeholder="Agent 名称"
						bind:value={newAgentName}
						onkeydown={(e) => e.key === 'Enter' && handleCreateAgent()}
					/>
					<button class="btn-confirm" onclick={handleCreateAgent}>创建</button>
				</div>
			{/if}

			<div class="list">
				{#each agentStore.agents as agent}
					<div
						class="list-item"
						class:active={agentStore.currentAgent?.id === agent.id}
						onclick={() => agentStore.selectAgent(agent)}
						role="button"
						tabindex="0"
						onkeydown={(e) => e.key === 'Enter' && agentStore.selectAgent(agent)}
					>
						<div class="avatar">{agent.name[0]}</div>
						<div class="item-content">
							<div class="item-title">{agent.name}</div>
							{#if agent.description}
								<div class="item-subtitle">{agent.description}</div>
							{/if}
						</div>
						<button class="add-btn" onclick={(e) => { e.stopPropagation(); handleNewSession(agent); }} title="新建对话">
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
							</svg>
						</button>
					</div>
				{/each}

				{#if agentStore.agents.length === 0 && !showNewAgent}
					<div class="empty">
						<span>暂无 Agent</span>
						<button class="btn-text" onclick={() => showNewAgent = true}>创建</button>
					</div>
				{/if}
			</div>
		</div>

		<!-- Session Section -->
		{#if agentStore.currentAgent}
			<div class="section">
				<div class="section-header">
					<span class="section-title">会话</span>
					<button class="icon-btn-sm" onclick={() => handleNewSession(agentStore.currentAgent!)}>
						<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
						</svg>
					</button>
				</div>
				<div class="list">
					{#each agentStore.sessions as session}
						<div
							class="list-item"
							class:active={agentStore.currentSession?.id === session.id}
							onclick={() => handleSelectSession(session)}
							role="button"
							tabindex="0"
							onkeydown={(e) => e.key === 'Enter' && handleSelectSession(session)}
						>
							<div class="item-content">
								<div class="item-title">{session.title || '新会话'}</div>
							</div>
						</div>
					{/each}
				</div>
			</div>
		{/if}
	</aside>

	<!-- Main Content -->
	<main class="content">
		{@render children()}
	</main>
</div>

<style>
	.app {
		display: flex;
		height: 100vh;
		overflow: hidden;
		background: var(--color-bg);
	}

	/* ── Sidebar ────────────────────────────────── */
	.sidebar {
		width: 260px;
		min-width: 260px;
		background: var(--color-bg-secondary);
		border-right: 1px solid var(--color-separator);
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.sidebar-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		border-bottom: 1px solid var(--color-separator);
		min-height: 52px;
	}

	.logo {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.logo-text {
		font-size: 17px;
		font-weight: 600;
		color: var(--color-fg);
		letter-spacing: -0.41px;
	}

	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border-radius: 8px;
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		transition: background 0.15s ease;
	}
	.icon-btn:hover { background: var(--color-bg-tertiary); }

	/* ── Sections ───────────────────────────────── */
	.section {
		padding: 8px 0;
	}

	.section-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 16px 4px;
	}

	.section-title {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.icon-btn-sm {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: var(--color-accent);
		cursor: pointer;
		transition: background 0.15s ease;
	}
	.icon-btn-sm:hover { background: rgba(0, 113, 227, 0.1); }

	/* ── List ───────────────────────────────────── */
	.list {
		padding: 0 8px;
	}

	.list-item {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 10px;
		border-radius: 8px;
		border: none;
		background: transparent;
		color: var(--color-fg);
		cursor: pointer;
		width: 100%;
		text-align: left;
		transition: background 0.15s ease;
	}
	.list-item:hover { background: var(--color-bg-tertiary); }
	.list-item.active { background: var(--color-accent); color: #fff; }

	.avatar {
		width: 32px;
		height: 32px;
		border-radius: 8px;
		background: var(--color-accent);
		color: #fff;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 14px;
		font-weight: 600;
		flex-shrink: 0;
	}
	.list-item.active .avatar { background: rgba(255,255,255,0.2); }

	.item-content {
		flex: 1;
		min-width: 0;
	}

	.item-title {
		font-size: 14px;
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.item-subtitle {
		font-size: 12px;
		color: var(--color-fg-secondary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.list-item.active .item-subtitle { color: rgba(255,255,255,0.7); }

	.add-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		opacity: 0.5;
		transition: opacity 0.15s ease;
		flex-shrink: 0;
	}
	.add-btn:hover { opacity: 1; background: var(--color-bg-tertiary); }
	.list-item.active .add-btn { color: #fff; }

	.empty {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		font-size: 14px;
		color: var(--color-fg-secondary);
	}

	/* ── New Agent Form ─────────────────────────── */
	.new-form {
		display: flex;
		gap: 6px;
		padding: 4px 8px 8px;
	}

	.new-form input {
		flex: 1;
		padding: 6px 10px;
		border-radius: 6px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 13px;
		outline: none;
	}
	.new-form input:focus { border-color: var(--color-accent); }

	.btn-confirm {
		padding: 6px 12px;
		border-radius: 6px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
	}
	.btn-confirm:hover { opacity: 0.9; }

	.btn-text {
		padding: 4px 8px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: var(--color-accent);
		font-size: 13px;
		cursor: pointer;
	}

	/* ── Content ────────────────────────────────── */
	.content {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow-y: auto;
	}
</style>
