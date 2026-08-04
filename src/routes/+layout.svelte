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
		try {
			agentStore.selectAgent(agent);
			await agentStore.createSession(agent.id, '新会话');
			if (agentStore.currentSession) {
				chatStore.loadHistory(agentStore.currentSession.id);
			}
		} catch (e) {
			console.error('Failed to create session:', e);
		}
	}

	function handleSelectSession(session: SessionDto) {
		agentStore.selectSession(session);
		chatStore.loadHistory(session.id);
	}
</script>

<div class="app-shell">
	<!-- Left Sidebar -->
	<aside class="sidebar">
		<div class="sidebar-header">
			<div class="logo">
				<img src="/icon.svg" alt="Prism" width="28" height="28" />
				<span class="logo-text">Prism Agent</span>
			</div>
		</div>

		<div class="sidebar-content">
			<!-- Agent List -->
			<div class="section">
				<div class="section-header">
					<span>Agent</span>
					<button class="btn-icon" onclick={() => showNewAgent = !showNewAgent}>+</button>
				</div>

				{#if showNewAgent}
					<div class="new-agent-form">
						<input
							type="text"
							placeholder="Agent 名称"
							bind:value={newAgentName}
							onkeydown={(e) => e.key === 'Enter' && handleCreateAgent()}
						/>
						<button class="btn-sm" onclick={handleCreateAgent}>创建</button>
					</div>
				{/if}

				{#each agentStore.agents as agent}
					<div
						class="agent-item"
						class:active={agentStore.currentAgent?.id === agent.id}
					>
						<div class="agent-info" onclick={() => agentStore.selectAgent(agent)}>
							<div class="agent-avatar">{agent.name[0]}</div>
							<div class="agent-meta">
								<span class="agent-name">{agent.name}</span>
								{#if agent.description}
									<span class="agent-desc">{agent.description}</span>
								{/if}
							</div>
						</div>
						<button class="btn-icon btn-new-chat" onclick={() => handleNewSession(agent)} title="新建对话">+</button>
					</div>
				{/each}

				{#if agentStore.agents.length === 0 && !showNewAgent}
					<div class="empty-hint">
						<p>暂无 Agent</p>
						<button class="btn-sm" onclick={() => showNewAgent = true}>创建第一个</button>
					</div>
				{/if}
			</div>

			<!-- Session List -->
			{#if agentStore.currentAgent}
				<div class="section">
					<div class="section-header">
						<span>会话</span>
					</div>
					{#each agentStore.sessions as session}
						<div
							class="session-item"
							class:active={agentStore.currentSession?.id === session.id}
							onclick={() => handleSelectSession(session)}
						>
							<span class="session-title">{session.title || '新会话'}</span>
						</div>
					{/each}
				</div>
			{/if}
		</div>

		<!-- Sidebar Footer -->
		<div class="sidebar-footer">
			<button class="settings-btn" onclick={() => goto('/settings')}>
				⚙ 设置
			</button>
		</div>
	</aside>

	<!-- Main Content -->
	<main class="content">
		{@render children()}
	</main>
</div>

<style>
	.app-shell {
		display: flex;
		height: 100vh;
		overflow: hidden;
	}

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
		padding: var(--space-4);
		border-bottom: 1px solid var(--color-separator);
	}

	.logo {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.logo-text {
		font-size: var(--text-lg);
		font-weight: 700;
	}

	.sidebar-content {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-2);
	}

	.section {
		margin-bottom: var(--space-4);
	}

	.section-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-2) var(--space-2);
		font-size: var(--text-sm);
		font-weight: 600;
		color: var(--color-fg-secondary);
	}

	.btn-icon {
		width: 24px;
		height: 24px;
		border-radius: var(--radius-sm);
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		font-size: 16px;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.btn-icon:hover { background: var(--color-bg-tertiary); color: var(--color-fg); }

	.new-agent-form {
		display: flex;
		gap: var(--space-1);
		padding: var(--space-1) var(--space-2);
	}
	.new-agent-form input {
		flex: 1;
		padding: 4px 8px;
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-sm);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: var(--text-sm);
	}
	.btn-sm {
		padding: 4px 10px;
		border-radius: var(--radius-sm);
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: var(--text-xs);
		cursor: pointer;
	}

	.agent-item {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2);
		border-radius: var(--radius-md);
		cursor: pointer;
		transition: background var(--duration-fast);
	}
	.agent-item:hover { background: var(--color-bg-tertiary); }
	.agent-item.active { background: var(--color-accent); color: #fff; }
	.agent-item.active .agent-desc { color: rgba(255,255,255,0.7); }

	.agent-info {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex: 1;
		min-width: 0;
	}

	.agent-avatar {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: var(--color-accent);
		color: #fff;
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		font-size: var(--text-sm);
		flex-shrink: 0;
	}

	.agent-meta {
		flex: 1;
		min-width: 0;
	}

	.agent-name {
		display: block;
		font-size: var(--text-sm);
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.agent-desc {
		display: block;
		font-size: var(--text-xs);
		color: var(--color-fg-secondary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.btn-new-chat {
		opacity: 0;
		transition: opacity var(--duration-fast);
	}
	.agent-item:hover .btn-new-chat { opacity: 1; }

	.session-item {
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		cursor: pointer;
		font-size: var(--text-sm);
		transition: background var(--duration-fast);
	}
	.session-item:hover { background: var(--color-bg-tertiary); }
	.session-item.active { background: var(--color-bg-tertiary); font-weight: 500; }

	.session-title {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		display: block;
	}

	.empty-hint {
		padding: var(--space-4);
		text-align: center;
		color: var(--color-fg-secondary);
		font-size: var(--text-sm);
	}

	.sidebar-footer {
		padding: var(--space-3);
		border-top: 1px solid var(--color-separator);
	}

	.settings-btn {
		width: 100%;
		padding: var(--space-2) var(--space-3);
		border-radius: var(--radius-md);
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		font-size: var(--text-sm);
		text-align: left;
		transition: background var(--duration-fast);
	}
	.settings-btn:hover { background: var(--color-bg-tertiary); color: var(--color-fg); }

	.content {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow-y: auto;
		background: var(--color-bg);
	}
</style>
