<script lang="ts">
	import '../app.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { agentStore } from '$lib/stores/agents.svelte';
	import { chatStore } from '$lib/stores/chat.svelte';
	import { contextStore } from '$lib/stores/context.svelte';
	import { themeStore } from '$lib/stores/theme.svelte';
	import AgentSidebar from '$lib/components/sidebar/AgentSidebar.svelte';
	import ToolApprovalDialog from '$lib/components/dialogs/ToolApprovalDialog.svelte';
	import CommandPalette, { type CommandItem } from '$lib/components/base/CommandPalette.svelte';
	import { useKeyboard } from '$lib/hooks/useKeyboard.svelte';
	import type { AgentDto, SessionDto } from '$lib/api';

	let { children } = $props();

	let newAgentName = $state('');
	let showNewAgent = $state(false);
	let paletteOpen = $state(false);

	const keyboard = useKeyboard();

	$effect(() => {
		keyboard.register('cmd+k', () => {
			paletteOpen = !paletteOpen;
		});
		keyboard.register('cmd+n', () => {
			const agent = agentStore.currentAgent;
			if (agent) {
				handleNewSession(agent);
			} else {
				showNewAgent = true;
			}
		});
		keyboard.register('cmd+\\', () => {
			if (agentStore.currentAgent) contextStore.toggleCollapse();
		});
		keyboard.register('cmd+1', () => {
			if (agentStore.currentAgent) contextStore.activeTab = 'usage';
		});
		keyboard.register('cmd+2', () => {
			if (agentStore.currentAgent) contextStore.activeTab = 'mcp';
		});
		keyboard.register('cmd+3', () => {
			if (agentStore.currentAgent) contextStore.activeTab = 'files';
		});
	});

	const paletteCommands = $derived.by<CommandItem[]>(() => [
		{
			id: 'new-session',
			title: '新建会话',
			shortcut: '⌘N',
			icon: 'plus',
			action: () => {
				const agent = agentStore.currentAgent;
				if (agent) {
					handleNewSession(agent);
				} else {
					showNewAgent = true;
				}
			}
		},
		{
			id: 'open-settings',
			title: '打开设置',
			icon: 'settings',
			action: () => goto('/settings')
		},
		{
			id: 'open-wiki',
			title: '知识库 (Wiki)',
			icon: 'chat',
			action: () => goto('/wiki')
		},
		{
			id: 'open-meetings',
			title: '会议纪要',
			icon: 'chat',
			action: () => goto('/meetings')
		},
		{
			id: 'open-translate',
			title: '翻译工具',
			icon: 'chat',
			action: () => goto('/translate')
		},
		{
			id: 'back-home',
			title: '返回面板',
			icon: 'back',
			action: () => {
				agentStore.currentSession = null;
				chatStore.messages = [];
				goto('/');
			}
		},
		...agentStore.agents.map((agent) => ({
			id: `chat-${agent.id}`,
			title: `与 ${agent.name} 对话`,
			icon: 'chat' as const,
			action: async () => {
				agentStore.selectAgent(agent);
				const session = await agentStore.createSession(agent.id, '新会话');
				if (agentStore.currentSession) {
					chatStore.loadHistory(agentStore.currentSession.id);
				}
			}
		})),
		...agentStore.sessions.map((session) => ({
			id: `session-${session.id}`,
			title: `打开会话：${session.title || '新会话'}`,
			icon: 'back' as const,
			action: () => {
				agentStore.selectSession(session);
				chatStore.loadHistory(session.id);
			}
		}))
	]);

	$effect(() => {
		themeStore.init();
	});

	$effect(() => {
		agentStore.loadAgents();
	});

	$effect(() => {
		const agent = agentStore.currentAgent;
		const session = agentStore.currentSession;
		if (agent) {
			contextStore.loadContext(agent.id, session?.id);
		}
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
	{#if $page.url.pathname === '/'}
	<!-- Sidebar -->
	<aside class="sidebar">
		<!-- Sidebar Header -->
		<div class="sidebar-header">
			<button class="logo" onclick={() => { agentStore.currentSession = null; chatStore.messages = []; goto('/'); }} title="返回面板">
				<img src="/icon.svg" alt="" width="24" height="24" />
				<span class="logo-text">Prism</span>
			</button>
			<button class="icon-btn" onclick={() => themeStore.toggle()} title={themeStore.theme === 'dark' ? '切换到浅色模式' : '切换到深色模式'} aria-label="切换主题">
				{#if themeStore.theme === 'dark'}
					<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/></svg>
				{:else}
					<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>
				{/if}
			</button>
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

		<!-- Quick Navigation -->
		<div class="section nav-section">
			<div class="section-header">
				<span class="section-title">工具</span>
			</div>
			<div class="list">
				<a href="/wiki" class="list-item nav-link">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
					<div class="item-content"><div class="item-title">知识库</div></div>
				</a>
				<a href="/meetings" class="list-item nav-link">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" y1="19" x2="12" y2="23"/></svg>
					<div class="item-content"><div class="item-title">会议纪要</div></div>
				</a>
				<a href="/translate" class="list-item nav-link">
					<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m5 8 6 6"/><path d="m4 14 6-6 2-3"/><path d="M2 5h12"/><path d="M7 2h1"/></svg>
					<div class="item-content"><div class="item-title">翻译工具</div></div>
				</a>
			</div>
		</div>
	</aside>
	{/if}

	<!-- Main Content -->
	<main class="content">
		{@render children()}
	</main>

	<!-- Agent Context Sidebar - 仅在聊天页显示 -->
	{#if agentStore.currentAgent && $page.url.pathname === '/'}
		<AgentSidebar />
	{/if}
</div>

<!-- Global Tool Approval Dialog -->
<ToolApprovalDialog />

<!-- Global Command Palette -->
<CommandPalette bind:open={paletteOpen} items={paletteCommands} onOpenSession={handleSelectSession} />

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
		border: none;
		background: none;
		padding: 0;
		cursor: pointer;
		border-radius: 6px;
		transition: opacity 0.15s;
	}
	.logo:hover { opacity: 0.8; }

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
	.icon-btn-sm:hover { background: color-mix(in srgb, var(--color-accent) 10%, transparent); }

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

	/* ── Navigation Links ──────────────────────── */
	.nav-section {
		margin-top: auto;
		border-top: 1px solid var(--color-separator);
	}
	.nav-link {
		text-decoration: none;
		color: inherit;
	}
</style>
