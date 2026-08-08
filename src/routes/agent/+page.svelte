<script lang="ts">
	import { invoke } from '$lib/api/client';
	import { agentApi } from '$lib/api';
	import { agentStore } from '$lib/stores/agents.svelte';
	import { chatStore } from '$lib/stores/chat.svelte';

	import MessageList from '$lib/components/chat/MessageList.svelte';
	import Composer from '$lib/components/chat/Composer.svelte';
	import ModelSelector from '$lib/components/chat/ModelSelector.svelte';

	let newAgentName = $state('');
	let showNewAgent = $state(false);
	let models = $state<any[]>([]);

	$effect(() => {
		agentStore.loadAgents();
		invoke<any[]>('model_list').then((m) => { models = m; }).catch(() => {});
	});

	$effect(() => {
		if (agentStore.currentAgent) {
			agentStore.loadSessions(agentStore.currentAgent.id);
		}
	});

	async function createAgent() {
		if (!newAgentName.trim()) return;
		try {
			await agentStore.createAgent(newAgentName.trim());
			newAgentName = '';
			showNewAgent = false;
		} catch (e) {
			console.error('Failed to create agent:', e);
		}
	}

	async function handleNewSession() {
		const agent = agentStore.currentAgent;
		if (!agent) return;
		try {
			const session = await agentStore.createSession(agent.id, '新会话');
			if (session) {
				chatStore.loadHistory(session.id);
			}
		} catch (e) {
			console.error('Failed to create session:', e);
			alert('创建会话失败: ' + e);
		}
	}

	function handleSelectSession(session: any) {
		agentStore.selectSession(session);
		chatStore.loadHistory(session.id);
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
</script>

<div class="agent-page">
	<!-- 左侧：Agent + 会话列表 -->
	<aside class="agent-list-pane">
		<div class="list-header">
			<span class="pane-title">Agent</span>
			<button class="icon-btn-sm" onclick={() => showNewAgent = !showNewAgent} aria-label="新建 Agent">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
			</button>
		</div>

		{#if showNewAgent}
			<div class="new-form">
				<input
					type="text"
					placeholder="Agent 名称"
					bind:value={newAgentName}
					onkeydown={(e) => e.key === 'Enter' && createAgent()}
					aria-label="Agent 名称"
				/>
				<button class="btn-confirm" onclick={createAgent}>创建</button>
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
					onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); agentStore.selectAgent(agent); } }}
				>
					<div class="avatar">{agent.name[0]}</div>
					<div class="item-content">
						<div class="item-title">{agent.name}</div>
						{#if agent.description}
							<div class="item-subtitle">{agent.description}</div>
						{/if}
					</div>
					<button class="add-btn" onclick={(e) => { e.stopPropagation(); agentStore.selectAgent(agent); handleNewSession(); }} title="新建对话" aria-label="新建对话">
						<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
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

		{#if agentStore.currentAgent}
			<div class="list-header session-header">
				<span class="pane-title">会话</span>
				<button class="icon-btn-sm" onclick={handleNewSession} aria-label="新建会话">
					<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
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
						onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); handleSelectSession(session); } }}
					>
						<div class="item-content">
							<div class="item-title">{session.title || '新会话'}</div>
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</aside>

	<!-- 右侧：聊天区 -->
	<div class="chat">
		<div class="chat-header">
			<div class="header-info">
				<h2>{agentStore.currentAgent?.name || 'Agent'}</h2>
				<span class="session-name">{agentStore.currentSession?.title || '选择或新建会话'}</span>
			</div>
			<div class="header-spacer"></div>
			<ModelSelector
				modelId={agentStore.currentAgent?.model_id ?? null}
				models={models}
				onSelect={handleSelectModel}
			/>
		</div>

		{#if agentStore.currentSession}
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
		{:else}
			<div class="chat-empty">
				<p>选择左侧 Agent 并新建会话开始对话</p>
				<button class="btn-primary" onclick={handleNewSession} disabled={!agentStore.currentAgent}>
					新建会话
				</button>
			</div>
		{/if}
	</div>
</div>

<style>
	.agent-page {
		display: flex;
		height: 100%;
		min-height: 0;
	}

	/* ── 左列表 ─────────────────────────────── */
	.agent-list-pane {
		width: 240px;
		min-width: 240px;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-secondary);
		border-right: 1px solid var(--color-separator);
		overflow-y: auto;
	}
	.list-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 14px 6px;
	}
	.session-header { margin-top: 8px; }
	.pane-title {
		font-size: 12px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}
	.icon-btn-sm {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 26px;
		height: 26px;
		border: none;
		border-radius: 6px;
		background: transparent;
		color: var(--color-accent);
		cursor: pointer;
	}
	.icon-btn-sm:hover { background: var(--color-bg-tertiary); }
	.new-form {
		display: flex;
		gap: 6px;
		padding: 4px 10px 8px;
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
	.list { padding: 0 8px 8px; }
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
		position: relative;
		transition: background 0.15s ease;
	}
	.list-item:hover { background: var(--color-bg-tertiary); }
	.list-item.active { background: var(--color-accent); color: #fff; }
	.avatar {
		width: 30px;
		height: 30px;
		border-radius: 8px;
		background: var(--color-bg-tertiary);
		color: var(--color-fg);
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: 13px;
		font-weight: 600;
		flex-shrink: 0;
	}
	.list-item.active .avatar { background: rgba(255,255,255,0.2); }
	.item-content { flex: 1; min-width: 0; }
	.item-title {
		font-size: 13px;
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.item-subtitle {
		font-size: 11px;
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
		width: 22px;
		height: 22px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		opacity: 0.4;
		flex-shrink: 0;
		transition: opacity 0.15s ease;
	}
	.add-btn:hover { opacity: 1; background: var(--color-bg-tertiary); }
	.list-item.active .add-btn { color: #fff; }
	.empty {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 14px;
		font-size: 13px;
		color: var(--color-fg-secondary);
	}
	.btn-text {
		padding: 4px 8px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: var(--color-accent);
		font-size: 13px;
		cursor: pointer;
	}

	/* ── 聊天区 ─────────────────────────────── */
	.chat {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
	}
	.chat-header {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 12px 20px;
		border-bottom: 1px solid var(--color-separator);
		min-height: 56px;
	}
	.header-info {
		display: flex;
		flex-direction: column;
		gap: 1px;
		min-width: 0;
	}
	.header-info h2 {
		margin: 0;
		font-size: 15px;
		font-weight: 600;
		color: var(--color-fg);
	}
	.session-name {
		font-size: 12px;
		color: var(--color-fg-secondary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.header-spacer { flex: 1; }
	.chat-empty {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
		color: var(--color-fg-secondary);
		font-size: 14px;
	}
	.btn-primary {
		padding: 8px 20px;
		border: none;
		border-radius: 8px;
		background: var(--color-accent);
		color: #fff;
		font-size: 14px;
		font-weight: 500;
		cursor: pointer;
	}
	.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
