<script lang="ts">
	import '../app.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { agentStore } from '$lib/stores/agents.svelte';
	import { contextStore } from '$lib/stores/context.svelte';
	import PrimaryNav from '$lib/components/layout/PrimaryNav.svelte';
	import AgentSidebar from '$lib/components/sidebar/AgentSidebar.svelte';
	import ToolApprovalDialog from '$lib/components/dialogs/ToolApprovalDialog.svelte';
	import CommandPalette, { type CommandItem } from '$lib/components/base/CommandPalette.svelte';
	import { useKeyboard } from '$lib/hooks/useKeyboard.svelte';
	import type { SessionDto } from '$lib/api';

	let { children } = $props();

	let paletteOpen = $state(false);

	const keyboard = useKeyboard();

	$effect(() => {
		keyboard.register('cmd+k', () => {
			paletteOpen = !paletteOpen;
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
			id: 'open-dashboard',
			title: '打开面板',
			icon: 'back',
			action: () => {
				agentStore.currentSession = null;
				goto('/');
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
		...agentStore.agents.map((agent) => ({
			id: `chat-${agent.id}`,
			title: `与 ${agent.name} 对话`,
			icon: 'chat' as const,
			action: async () => {
				agentStore.selectAgent(agent);
				goto('/agent');
			}
		})),
		...agentStore.sessions.map((session) => ({
			id: `session-${session.id}`,
			title: `打开会话：${session.title || '新会话'}`,
			icon: 'back' as const,
			action: () => {
				agentStore.selectSession(session);
				goto('/agent');
			}
		}))
	]);

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

	function handleSelectSession(session: SessionDto) {
		agentStore.selectSession(session);
		goto('/agent');
	}
</script>

<div class="app">
	<!-- 最左侧窄导航 -->
	<PrimaryNav />

	<!-- Main Content -->
	<main class="content">
		{@render children()}
	</main>

	<!-- Agent Context Sidebar - 仅在 Agent 页显示 -->
	{#if agentStore.currentAgent && $page.url.pathname === '/agent'}
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
		min-width: 375px;
		overflow: hidden;
		background: var(--color-bg);
	}

	.content {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		overflow-y: auto;
	}
</style>
