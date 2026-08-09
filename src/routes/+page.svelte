<script lang="ts">
	import { goto } from '$app/navigation';
	import { invoke } from '$lib/api/client';
	import { agentApi } from '$lib/api';
	import { agentStore } from '$lib/stores/agents.svelte';
	import { dashboardStore } from '$lib/stores/dashboard.svelte';

	import DashboardHeader from '$lib/components/dashboard/DashboardHeader.svelte';
	import UsageStatsCard from '$lib/components/dashboard/UsageStatsCard.svelte';
	import UsageTrendChart from '$lib/components/dashboard/UsageTrendChart.svelte';
	import AgentLauncher from '$lib/components/dashboard/AgentLauncher.svelte';
	import SkillOverviewCard from '$lib/components/dashboard/SkillOverviewCard.svelte';
	import McpOverviewCard from '$lib/components/dashboard/McpOverviewCard.svelte';
	import OrchestratorPanel from '$lib/components/dashboard/OrchestratorPanel.svelte';
	import RecentSessionsCard from '$lib/components/dashboard/RecentSessionsCard.svelte';
	import TaskDesigner from '$lib/components/task/TaskDesigner.svelte';

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
			await agentStore.createSession(agent.id, '新会话');
			goto('/agent');
		} catch (e) {
			console.error('Failed to start chat:', e);
		}
	}

	function handleOpenSession(sessionId: string) {
		const session = agentStore.sessions.find((s) => s.id === sessionId);
		if (session) {
			agentStore.selectSession(session);
			goto('/agent');
		}
	}

	$effect(() => {
		load();
		dashboardStore.loadOverview();
	});
</script>

<div class="dashboard">
	<DashboardHeader agentCount={dashboardStore.overview?.agents.length ?? agentStore.agents.length} />

	<div class="dashboard-body">
		<!-- Row 1: Orchestrator Panel（自主编排主入口） -->
		<div class="section-row orchestrator-card">
			<OrchestratorPanel />
		</div>

		<!-- Row 2: Task Designer（手动任务设计器） -->
		<div class="section-row">
			<TaskDesigner />
		</div>

		<!-- Row 3: Agent Launcher + Usage Trend -->
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

		<!-- Row 4: Stats + Skill + MCP（三列紧凑布局） -->
		<div class="section-row three-col">
			<UsageStatsCard usage={dashboardStore.overview?.usage ?? null} />
			<SkillOverviewCard skills={dashboardStore.overview?.skills ?? null} />
			<McpOverviewCard servers={dashboardStore.overview?.mcp_servers ?? []} />
		</div>

		<!-- Row 5: Recent Sessions -->
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

<style>
	.dashboard {
		flex: 1;
		min-width: 0;
		overflow-y: auto;
		padding: 24px 32px;
	}

	.dashboard-body {
		max-width: 1200px;
		margin: 0 auto;
		display: flex;
		flex-direction: column;
		gap: 20px;
	}

	.section-row {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.section-row.two-col {
		flex-direction: row;
		gap: 16px;
	}
	.section-row.three-col {
		flex-direction: row;
		gap: 16px;
	}
	.col-main {
		flex: 2;
		min-width: 0;
	}
	.col-side {
		flex: 1;
		min-width: 0;
	}
	.section-row.three-col > :global(*) {
		flex: 1;
		min-width: 0;
	}

	.orchestrator-card {
		height: 560px;
		overflow: hidden;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		box-shadow: var(--shadow-sm);
	}

	.setup-banner {
		margin-top: 8px;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-separator);
		border-radius: 12px;
		padding: 16px 20px;
	}
	.setup-banner-content {
		display: flex;
		align-items: center;
		gap: 12px;
	}
	.setup-icon {
		width: 36px;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--color-accent) 10%, transparent);
		border-radius: 10px;
		flex-shrink: 0;
	}
	.setup-text {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.setup-text strong {
		font-size: 15px;
		color: var(--color-fg);
	}
	.setup-text span {
		font-size: 13px;
		color: var(--color-fg-secondary);
	}
	.setup-btn {
		padding: 8px 16px;
		border: none;
		border-radius: 8px;
		background: var(--color-accent);
		color: #fff;
		font-size: 14px;
		font-weight: 500;
		cursor: pointer;
	}

	@media (max-width: 900px) {
		.section-row.two-col,
		.section-row.three-col {
			flex-direction: column;
		}
	}
</style>
