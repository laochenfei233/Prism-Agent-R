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

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);

	// Setup form
	let pName = $state('');
	let pKind = $state('openai');
	let pUrl = $state('');
	let pKey = $state('');
	let mProvider = $state('');
	let mModelId = $state('');
	let availableModels = $state<string[]>([]);
	let loadingModels = $state(false);
	let msg = $state('');

	// Chat
	let input = $state('');

	async function fetchModels() {
		if (!mProvider) return;
		loadingModels = true;
		availableModels = [];
		try {
			const result = await invoke<{models: string[]}>('model_fetch_available', { providerId: mProvider });
			availableModels = result.models || [];
		} catch (e) {
			msg = '拉取失败: ' + String(e);
		} finally {
			loadingModels = false;
		}
	}

	async function load() {
		providers = await invoke<any[]>('model_providers');
		models = await invoke<any[]>('model_list');
	}

	async function saveProvider() {
		if (!pName.trim()) { msg = '请输入名称'; return; }
		try {
			await invoke('settings_add_provider', {
				name: pName.trim(), kind: pKind,
				baseUrl: pUrl.trim() || null, apiKey: pKey.trim() || null
			});
			pName = ''; pUrl = ''; pKey = '';
			await load();
			msg = '✓ Provider 已添加';
		} catch (e) {
			msg = '错误: ' + String(e);
		}
	}

	async function saveModel() {
		if (!mProvider || !mModelId.trim()) { msg = '请选择 Provider 并输入模型 ID'; return; }
		try {
			await invoke('settings_add_model', {
				providerId: mProvider, modelId: mModelId.trim(),
				displayName: null, isDefault: true
			});
			mModelId = '';
			await load();
			msg = '✓ 模型已添加';
		} catch (e) {
			msg = '错误: ' + String(e);
		}
	}

	async function createAgent() {
		try {
			await agentApi.create('助手', 'AI 助手', '你是一个有用的 AI 助手。请用中文回答。');
			msg = '✓ Agent 已创建';
			await load();
			agentStore.loadAgents();
			dashboardStore.loadOverview();
		} catch (e) {
			msg = '错误: ' + String(e);
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

	async function handleSend() {
		if (!input.trim() || !agentStore.currentSession) return;
		const content = input.trim();
		input = '';
		await chatStore.send(agentStore.currentSession.id, content);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend();
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
			<!-- Row 1: Usage Stats -->
			<UsageStatsCard usage={dashboardStore.overview?.usage ?? null} />

			<!-- Row 2: Agent Launcher + Trend -->
			<div class="row-two">
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

			<!-- Row 3: Skill + MCP -->
			<div class="row-three">
				<SkillOverviewCard skills={dashboardStore.overview?.skills ?? null} />
				<McpOverviewCard servers={dashboardStore.overview?.mcp_servers ?? []} />
			</div>

			<!-- Row 4: Recent Sessions -->
			<RecentSessionsCard
				sessions={dashboardStore.overview?.recent_sessions ?? []}
				onOpenSession={handleOpenSession}
			/>

			<!-- Row 5: Task Designer -->
			<div class="row-five">
				<TaskDesigner />
			</div>

			<!-- Row 6: Quick Setup (if no providers/models) -->
			{#if providers.length === 0 || models.length === 0}
				<div class="setup-banner">
					<div class="setup-banner-content">
						<span class="setup-icon">⚡</span>
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
			<h2>{agentStore.currentAgent?.name || 'Agent'}</h2>
			<span class="session-name">{agentStore.currentSession.title || '新会话'}</span>
		</div>

		<div class="messages">
			{#each chatStore.messages as msg}
				<div class="message" class:user={msg.role === 'user'}>
					<div class="bubble">
						{msg.content}
					</div>
				</div>
			{/each}

			{#if chatStore.streaming && chatStore.streamingText}
				<div class="message">
					<div class="bubble streaming">{chatStore.streamingText}<span class="cursor">|</span></div>
				</div>
			{/if}
		</div>

		<div class="composer">
			<textarea
				bind:value={input}
				onkeydown={handleKeydown}
				placeholder="输入消息..."
				rows="1"
				disabled={chatStore.isGenerating}
			></textarea>
			<button
				class="send-btn"
				onclick={handleSend}
				disabled={!input.trim() || chatStore.isGenerating}
			>
				{#if chatStore.isGenerating}
					<span class="spinner"></span>
				{:else}
					<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
						<path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"/>
					</svg>
				{/if}
			</button>
		</div>
	</div>
{/if}

<style>
	/* ── Dashboard ──────────────────────────────── */
	.dashboard {
		display: flex;
		flex-direction: column;
		height: 100%;
		overflow-y: auto;
	}

	.dashboard-body {
		display: flex;
		flex-direction: column;
		gap: 16px;
		padding-bottom: 24px;
	}

	.row-two {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
		padding: 0 24px;
	}

	.col-main, .col-side {
		min-width: 0;
	}

	.row-three {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 16px;
		padding: 0 24px;
	}

	.row-five {
		padding: 0 24px;
		height: 520px;
	}

	/* ── Setup Banner ─────────────────────────── */
	.setup-banner {
		padding: 0 24px;
	}
	.setup-banner-content {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 14px 20px;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-accent);
		border-radius: var(--radius-md);
	}
	.setup-icon {
		font-size: 24px;
		flex-shrink: 0;
	}
	.setup-text {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 2px;
	}
	.setup-text strong {
		font-size: var(--text-body);
		color: var(--color-fg);
	}
	.setup-text span {
		font-size: var(--text-caption1);
		color: var(--color-fg-secondary);
	}
	.setup-btn {
		padding: 8px 16px;
		border-radius: 9999px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: var(--text-caption1);
		font-weight: 600;
		cursor: pointer;
		white-space: nowrap;
	}
	.setup-btn:hover { background: var(--color-accent-hover); }

	@media (max-width: 900px) {
		.row-two, .row-three {
			grid-template-columns: 1fr;
		}
	}

	/* ── Setup Page ─────────────────────────────── */
	.page {
		padding: 24px;
		max-width: 480px;
		overflow-y: auto;
	}

	.header { margin-bottom: 20px; }
	.header h1 {
		font-size: 28px;
		font-weight: 700;
		color: var(--color-fg);
		margin: 0 0 4px;
	}
	.header p {
		font-size: 15px;
		color: var(--color-fg-secondary);
		margin: 0;
	}

	.toast {
		padding: 10px 16px;
		border-radius: 10px;
		background: #34C759;
		color: #fff;
		font-size: 15px;
		margin-bottom: 16px;
	}
	.toast.error { background: #FF3B30; }

	.card {
		background: var(--color-bg-secondary);
		border-radius: 14px;
		padding: 16px;
		margin-bottom: 12px;
	}
	.card.disabled { opacity: 0.5; pointer-events: none; }

	.card-header {
		display: flex;
		align-items: center;
		gap: 10px;
		margin-bottom: 14px;
	}
	.step-num {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: var(--color-accent);
		color: #fff;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 13px;
		font-weight: 600;
	}
	.step-title {
		font-size: 17px;
		font-weight: 600;
		color: var(--color-fg);
	}

	.form { display: flex; flex-direction: column; gap: 12px; }
	.input-group { display: flex; flex-direction: column; gap: 4px; }
	.input-group label {
		font-size: 13px;
		font-weight: 500;
		color: var(--color-fg-secondary);
	}
	.input-group input,
	.input-group select {
		padding: 10px 12px;
		border-radius: 10px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 15px;
		outline: none;
	}
	.input-group input:focus,
	.input-group select:focus { border-color: var(--color-accent); }

	.hint { font-size: 14px; color: var(--color-fg-tertiary); margin: 0; }

	.done-badge {
		margin-top: 12px;
		padding: 8px 12px;
		border-radius: 8px;
		background: rgba(52, 199, 89, 0.12);
		color: #34C759;
		font-size: 14px;
	}

	.btn-primary {
		padding: 12px 20px;
		border-radius: 12px;
		border: none;
		background: #FF6900;
		color: #fff;
		font-size: 17px;
		font-weight: 600;
		cursor: pointer;
	}
	.btn-primary:hover { background: #E85D00; }
	.btn-primary:active { transform: scale(0.98); }

	.btn-secondary {
		padding: 10px 16px;
		border-radius: 10px;
		border: 1px solid #FF6900;
		background: transparent;
		color: #FF6900;
		font-size: 15px;
		font-weight: 500;
		cursor: pointer;
	}
	.btn-secondary:hover { background: rgba(255, 105, 0, 0.08); }
	.btn-secondary:disabled { opacity: 0.5; cursor: not-allowed; }

	.btn-green {
		padding: 12px 20px;
		border-radius: 12px;
		border: none;
		background: #34C759;
		color: #fff;
		font-size: 17px;
		font-weight: 600;
		cursor: pointer;
	}
	.btn-green:hover { background: #2DB84E; }
	.btn-green:active { transform: scale(0.98); }

	/* ── Chat ───────────────────────────────────── */
	.chat {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.chat-header {
		padding: 12px 20px;
		border-bottom: 1px solid var(--color-separator);
		background: var(--color-glass);
		backdrop-filter: saturate(180%) blur(20px);
	}
	.chat-header h2 {
		font-size: 17px;
		font-weight: 600;
		color: var(--color-fg);
		margin: 0;
	}
	.session-name {
		font-size: 13px;
		color: var(--color-fg-secondary);
	}

	.messages {
		flex: 1;
		overflow-y: auto;
		padding: 20px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.message {
		display: flex;
		max-width: 80%;
	}
	.message.user { justify-content: flex-end; margin-left: auto; }

	.bubble {
		padding: 10px 14px;
		border-radius: 18px;
		font-size: 15px;
		line-height: 1.5;
		color: var(--color-fg);
		background: var(--color-bg-secondary);
		word-break: break-word;
	}
	.message.user .bubble {
		background: #FF6900;
		color: #fff;
		border-bottom-right-radius: 4px;
	}
	.bubble:not(.message.user .bubble) {
		border-bottom-left-radius: 4px;
	}

	.streaming { border-bottom-left-radius: 4px; }
	.cursor { animation: blink 1s infinite; color: #FF6900; }

	.composer {
		padding: 12px 16px;
		border-top: 1px solid var(--color-separator);
		background: var(--color-glass);
		backdrop-filter: saturate(180%) blur(20px);
		display: flex;
		gap: 10px;
		align-items: flex-end;
	}

	textarea {
		flex: 1;
		padding: 10px 14px;
		border-radius: 20px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 15px;
		font-family: var(--font-sans);
		resize: none;
		outline: none;
		min-height: 40px;
		max-height: 120px;
	}
	textarea:focus { border-color: #FF6900; }

	.send-btn {
		width: 40px;
		height: 40px;
		border-radius: 50%;
		border: none;
		background: #FF6900;
		color: #fff;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.send-btn:hover { background: #E85D00; }
	.send-btn:active { transform: scale(0.95); }
	.send-btn:disabled { opacity: 0.4; cursor: not-allowed; }

	.spinner {
		width: 18px;
		height: 18px;
		border: 2px solid rgba(255,255,255,0.3);
		border-top-color: #fff;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes blink { 0%, 100% { opacity: 1; } 50% { opacity: 0; } }
	@keyframes spin { to { transform: rotate(360deg); } }
</style>
