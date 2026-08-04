<script lang="ts">
	import { goto } from '$app/navigation';
	import { invoke } from '$lib/api/client';
	import { agentApi } from '$lib/api';
	import { agentStore } from '$lib/stores/agents.svelte';
	import { chatStore } from '$lib/stores/chat.svelte';

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
	let msg = $state('');

	// Setup form
	let pName = $state('');
	let pKind = $state('openai');
	let pUrl = $state('');
	let pKey = $state('');
	let mProvider = $state('');
	let mModelId = $state('');
	let availableModels = $state<string[]>([]);
	let loadingModels = $state(false);

	// Chat
	let input = $state('');

	async function fetchModels() {
		if (!mProvider) return;
		loadingModels = true;
		availableModels = [];
		try {
			const result = await invoke<{models: string[]}>('model_fetch_available', { provider_id: mProvider });
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
				base_url: pUrl.trim() || null, api_key: pKey.trim() || null
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
				provider_id: mProvider, model_id: mModelId.trim(),
				display_name: null, is_default: true
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
		} catch (e) {
			msg = '错误: ' + String(e);
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

	$effect(() => { load(); });
</script>

{#if !agentStore.currentSession}
	<!-- 无会话：设置向导或欢迎页 -->
	{#if providers.length > 0 && models.length > 0}
		<div class="welcome">
			<div class="welcome-content">
				<img src="/icon.svg" alt="" width="64" height="64" />
				<h1>Prism Agent</h1>
				<p>选择左侧 Agent，点击 + 创建会话开始对话</p>
			</div>
		</div>
	{:else}
		<div class="page">
			<div class="header">
				<h1>Prism Agent</h1>
				<p>开始使用前，请先配置模型</p>
			</div>

			{#if msg}
				<div class="toast" class:error={msg.startsWith('错误')}>{msg}</div>
			{/if}

			<!-- Step 1: Provider -->
			<div class="card">
				<div class="card-header">
					<span class="step-num">1</span>
					<span class="step-title">添加 Provider</span>
				</div>
				<div class="form">
					<div class="input-group">
						<label for="p-kind">类型</label>
						<select id="p-kind" bind:value={pKind}>
							<option value="openai">OpenAI 兼容</option>
							<option value="ollama">Ollama（本地）</option>
						</select>
					</div>
					<div class="input-group">
						<label for="p-name">名称</label>
						<input id="p-name" bind:value={pName} placeholder="如 OpenAI、通义千问" />
					</div>
					<div class="input-group">
						<label for="p-url">Base URL</label>
						<input id="p-url" bind:value={pUrl} placeholder={pKind === 'ollama' ? 'http://localhost:11434/v1' : 'https://api.openai.com/v1'} />
					</div>
					<div class="input-group">
						<label for="p-key">API Key</label>
						<input id="p-key" bind:value={pKey} type="password" placeholder="sk-..." />
					</div>
					<button class="btn-primary" onclick={saveProvider}>保存 Provider</button>
				</div>
				{#if providers.length > 0}
					<div class="done-badge">✓ 已添加：{providers.map(p => p.name).join(', ')}</div>
				{/if}
			</div>

			<!-- Step 2: Model -->
			<div class="card" class:disabled={providers.length === 0}>
				<div class="card-header">
					<span class="step-num">2</span>
					<span class="step-title">添加模型</span>
				</div>
				{#if providers.length === 0}
					<p class="hint">请先完成步骤 1</p>
				{:else}
					<div class="form">
						<div class="input-group">
							<label for="m-provider">Provider</label>
							<select id="m-provider" bind:value={mProvider} onchange={() => { availableModels = []; mModelId = ''; }}>
								<option value="">选择 Provider</option>
								{#each providers as p}<option value={p.id}>{p.name}</option>{/each}
							</select>
						</div>
						{#if mProvider}
							<button class="btn-secondary" onclick={fetchModels} disabled={loadingModels}>
								{loadingModels ? '拉取中...' : '拉取可用模型'}
							</button>
						{/if}
						{#if availableModels.length > 0}
							<div class="input-group">
								<label for="m-model">选择模型</label>
								<select id="m-model" bind:value={mModelId}>
									<option value="">-- 选择模型 --</option>
									{#each availableModels as m}<option value={m}>{m}</option>{/each}
								</select>
							</div>
						{:else}
							<div class="input-group">
								<label for="m-model-id">模型 ID</label>
								<input id="m-model-id" bind:value={mModelId} placeholder="如 gpt-4o、qwen2.5" />
							</div>
						{/if}
						<button class="btn-primary" onclick={saveModel}>保存模型</button>
					</div>
				{/if}
				{#if models.length > 0}
					<div class="done-badge">✓ 已添加：{models.map(m => m.display_name || m.model_id).join(', ')}</div>
				{/if}
			</div>

			<!-- Step 3: Agent -->
			<div class="card" class:disabled={models.length === 0}>
				<div class="card-header">
					<span class="step-num">3</span>
					<span class="step-title">创建 Agent</span>
				</div>
				{#if models.length === 0}
					<p class="hint">请先完成步骤 2</p>
				{:else}
					<button class="btn-green" onclick={createAgent}>创建默认 Agent</button>
				{/if}
			</div>
		</div>
	{/if}
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
	/* ── Welcome ────────────────────────────────── */
	.welcome {
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.welcome-content {
		text-align: center;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
	}
	.welcome-content h1 {
		font-size: 28px;
		font-weight: 700;
		color: var(--color-fg);
		margin: 0;
	}
	.welcome-content p {
		font-size: 15px;
		color: var(--color-fg-secondary);
		margin: 0;
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
		background: #007AFF;
		color: #fff;
		font-size: 17px;
		font-weight: 600;
		cursor: pointer;
	}
	.btn-primary:hover { background: #0066D6; }
	.btn-primary:active { transform: scale(0.98); }

	.btn-secondary {
		padding: 10px 16px;
		border-radius: 10px;
		border: 1px solid #007AFF;
		background: transparent;
		color: #007AFF;
		font-size: 15px;
		font-weight: 500;
		cursor: pointer;
	}
	.btn-secondary:hover { background: rgba(0, 122, 255, 0.08); }
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
		background: #007AFF;
		color: #fff;
		border-bottom-right-radius: 4px;
	}
	.bubble:not(.message.user .bubble) {
		border-bottom-left-radius: 4px;
	}

	.streaming { border-bottom-left-radius: 4px; }
	.cursor { animation: blink 1s infinite; color: #007AFF; }

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
	textarea:focus { border-color: #007AFF; }

	.send-btn {
		width: 40px;
		height: 40px;
		border-radius: 50%;
		border: none;
		background: #007AFF;
		color: #fff;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.send-btn:hover { background: #0066D6; }
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
