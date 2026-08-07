<script lang="ts">
	import { goto } from '$app/navigation';
	import { invoke } from '$lib/api/client';
	import { agentApi } from '$lib/api';
	import { agentStore } from '$lib/stores/agents.svelte';
	import { chatStore } from '$lib/stores/chat.svelte';
	import { dashboardStore } from '$lib/stores/dashboard.svelte';

	import DashboardHeader from '$lib/components/dashboard/DashboardHeader.svelte';
	import UsageStatsCard from '$lib/components/dashboard/UsageStatsCard.svelte';
	import UsageTrendChart from '$lib/components/dashboard/UsageTrendChart.svelte';
	import AgentLauncher from '$lib/components/dashboard/AgentLauncher.svelte';
	import SkillOverviewCard from '$lib/components/dashboard/SkillOverviewCard.svelte';
	import McpOverviewCard from '$lib/components/dashboard/McpOverviewCard.svelte';
	import RecentSessionsCard from '$lib/components/dashboard/RecentSessionsCard.svelte';
	import TaskDesigner from '$lib/components/task/TaskDesigner.svelte';
	import MessageList from '$lib/components/chat/MessageList.svelte';
	import Composer from '$lib/components/chat/Composer.svelte';
	import ModelSelector from '$lib/components/chat/ModelSelector.svelte';

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);

	async function load() {
		providers = await invoke<any[]>('model_providers');
		models = await invoke<any[]>('model_list');
	}

	async function createAgent() {
		try {
			await agentApi.create('助手', 'AI 助手', '你是一个有用的 AI 助手。请用中文回答。');
			agentStore.loadAgents();
			dashboardStore.loadOverview();
		} catch (e) {
			console.error('Failed to create agent:', e);
		}
	}

	async function handleStartChat(agentId: string) {
		const agent = agentStore.agents.find((a) => a.id === agentId);
		if (!agent) return;
		try {
			agentStore.selectAgent(agent);
			const session = await agentStore.createSession(agent.id, '新会话');
			if (agentStore.currentSession) {
				chatStore.loadHistory(agentStore.currentSession.id);
			}
		} catch (e) {
			console.error('Failed to start chat:', e);
		}
	}

	function handleOpenSession(sessionId: string) {
		const session = agentStore.sessions.find((s) => s.id === sessionId);
		if (session) {
			agentStore.selectSession(session);
			chatStore.loadHistory(session.id);
		}
	}

	function goToDashboard() {
		agentStore.currentSession = null;
		chatStore.messages = [];
	}

	async function handleSend(content: string, attachments?: string[]) {
		if (!agentStore.currentSession) return;
		await chatStore.send(agentStore.currentSession.id, content, attachments);
	}

	async function handleSelectModel(modelId: string) {
		const agent = agentStore.currentAgent;
		if (!agent) return;
		try {
			await agentApi.update(agent.id, { model_id: modelId });
			await agentStore.loadAgents();
			agentStore.currentAgent = agentStore.agents.find((a) => a.id === agent.id) ?? agent;
		} catch (e) {
			console.error('Failed to update model:', e);
		}
	}

	$effect(() => {
		load();
		dashboardStore.loadOverview();
	});
</script>

{#if !agentStore.currentSession}
	<!-- 无会话：Dashboard（始终显示） -->
	<div class="dashboard">
		<DashboardHeader agentCount={dashboardStore.overview?.agents.length ?? agentStore.agents.length} />

		<div class="dashboard-body">
			<!-- Row 1: Task Designer（核心功能前置） -->
			<div class="section-row">
				<TaskDesigner />
			</div>

			<!-- Row 2: Agent Launcher + Usage Trend -->
			<div class="section-row two-col">
				<div class="col-main">
					<AgentLauncher
						agents={dashboardStore.overview?.agents ?? agentStore.agents.map(a => ({
							id: a.id, name: a.name, description: a.description ?? '',
							avatar: null, model_name: null, skill_count: 0, mcp_count: 0,
							last_used: null, order_key: a.order_key ?? 0
						}))}
						onStartChat={handleStartChat}
						onCreateAgent={createAgent}
					/>
				</div>
				<div class="col-side">
					<UsageTrendChart data={dashboardStore.overview?.usage_trend ?? []} />
				</div>
			</div>

			<!-- Row 3: Stats + Skill + MCP（三列紧凑布局） -->
			<div class="section-row three-col">
				<UsageStatsCard usage={dashboardStore.overview?.usage ?? null} />
				<SkillOverviewCard skills={dashboardStore.overview?.skills ?? null} />
				<McpOverviewCard servers={dashboardStore.overview?.mcp_servers ?? []} />
			</div>

			<!-- Row 4: Recent Sessions -->
			<RecentSessionsCard
				sessions={dashboardStore.overview?.recent_sessions ?? []}
				onOpenSession={handleOpenSession}
			/>

			<!-- Quick Setup Banner -->
			{#if providers.length === 0 || models.length === 0}
				<div class="setup-banner">
					<div class="setup-banner-content">
						<span class="setup-icon">
							<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--color-accent)" stroke-width="2"><path d="M13 2 3 14h9l-1 8 10-12h-9l1-8z"/></svg>
						</span>
						<div class="setup-text">
							<strong>快速开始</strong>
							<span>配置 Provider 和模型后即可开始对话</span>
						</div>
						<button class="setup-btn" onclick={() => goto('/settings')}>去设置</button>
					</div>
				</div>
			{/if}
		</div>
	</div>
{:else}
	<!-- 有会话：对话界面 -->
	<div class="chat">
		<div class="chat-header">
			<button class="back-btn" onclick={goToDashboard} title="返回面板">
				<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
			</button>
			<div class="header-info">
				<h2>{agentStore.currentAgent?.name || 'Agent'}</h2>
				<span class="session-name">{agentStore.currentSession.title || '新会话'}</span>
			</div>
			<div class="header-spacer"></div>
			<ModelSelector
				modelId={agentStore.currentAgent?.model_id ?? null}
				models={models}
				onSelect={handleSelectModel}
			/>
		</div>

		<MessageList
			messages={chatStore.messages}
			streaming={chatStore.streaming}
			streamingText={chatStore.streamingText}
		/>

		<Composer
			disabled={chatStore.isGenerating}
			generating={chatStore.isGenerating}
			onSend={handleSend}
			onAbort={() => chatStore.abort(agentStore.currentSession?.id ?? '')}
		/>
	</div>
{/if}

<style>
	/* ── Dashboard ─────────────────────────────── */
	.dashboard {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow-y: auto;
		background: var(--color-bg-secondary);
	}

	.dashboard-body {
		max-width: 960px;
		width: 100%;
		margin: 0 auto;
		padding: 20px 32px 48px;
		display: flex;
		flex-direction: column;
		gap: 24px;
	}

	.section-row {
		width: 100%;
	}

	.section-row.two-col {
		display: grid;
		grid-template-columns: 1.6fr 1fr;
		gap: 16px;
	}

	.section-row.three-col {
		display: grid;
		grid-template-columns: 1fr 1fr 1fr;
		gap: 16px;
	}

	.col-main, .col-side {
		min-width: 0;
	}

	/* ── Setup Banner ─────────────────────────── */
	.setup-banner {
		width: 100%;
	}
	.setup-banner-content {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 16px 20px;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-sm);
	}
	.setup-icon {
		font-size: 20px;
		flex-shrink: 0;
	}
	.setup-text {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}
	.setup-text strong {
		font-size: 14px;
		font-weight: 600;
		color: var(--color-fg);
	}
	.setup-text span {
		font-size: 13px;
		color: var(--color-fg-secondary);
	}
	.setup-btn {
		padding: 7px 14px;
		border-radius: 8px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		white-space: nowrap;
		transition: background 0.15s;
	}
	.setup-btn:hover { background: var(--color-accent-hover); }

	@media (max-width: 900px) {
		.section-row.two-col,
		.section-row.three-col {
			grid-template-columns: 1fr;
		}
		.dashboard-body {
			padding: 16px;
		}
	}

	/* ── Chat ───────────────────────────────────── */
	.chat {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.chat-header {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px 16px;
		border-bottom: 1px solid var(--color-separator);
		background: var(--color-bg);
	}

	.back-btn {
		width: 32px;
		height: 32px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg-elevated);
		color: var(--color-fg-secondary);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		transition: all 0.12s;
	}
	.back-btn:hover {
		background: var(--color-bg-hover);
		color: var(--color-fg);
		border-color: var(--color-border-strong);
	}

	.header-info {
		display: flex;
		flex-direction: column;
		gap: 1px;
		min-width: 0;
	}

	.header-info h2 {
		font-size: 15px;
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}

	.session-name {
		font-size: 12px;
		color: var(--color-muted);
	}

	.header-spacer {
		flex: 1;
	}
</style>
