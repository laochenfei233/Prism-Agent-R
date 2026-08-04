<script lang="ts">
	import { goto } from '$app/navigation';
	import { invoke } from '$lib/api/client';
	import { agentApi } from '$lib/api';

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
	let showConfig = $state(false);

	// Quick add provider
	let providerName = $state('');
	let providerKind = $state('openai');
	let providerUrl = $state('');
	let providerKey = $state('');

	// Quick add model
	let modelProviderId = $state('');
	let modelId = $state('');
	let modelDefault = $state(true);

	let step = $state(1); // 1=add provider, 2=add model, 3=create agent
	let message = $state('');

	async function loadConfig() {
		providers = await invoke<any[]>('model_providers');
		models = await invoke<any[]>('model_list');
		if (providers.length > 0) step = 2;
		if (models.length > 0) step = 3;
	}

	async function addProvider() {
		if (!providerName.trim()) return;
		await invoke('settings_add_provider', {
			name: providerName.trim(),
			kind: providerKind,
			base_url: providerUrl.trim() || null,
			api_key: providerKey.trim() || null,
		});
		providerName = '';
		providerUrl = '';
		providerKey = '';
		await loadConfig();
		message = 'Provider 添加成功';
		setTimeout(() => message = '', 2000);
	}

	async function addModel() {
		if (!modelProviderId || !modelId.trim()) return;
		await invoke('settings_add_model', {
			provider_id: modelProviderId,
			model_id: modelId.trim(),
			display_name: null,
			is_default: modelDefault,
		});
		modelId = '';
		await loadConfig();
		message = '模型添加成功';
		setTimeout(() => message = '', 2000);
	}

	async function createAgent() {
		const agent = await agentApi.create('助手', '一个有用的 AI 助手', '你是一个有用的 AI 助手。请用中文回答。');
		message = `Agent "${agent.name}" 创建成功`;
		setTimeout(() => message = '', 2000);
	}

	$effect(() => {
		loadConfig();
	});
</script>

<div class="chat-page">
	{#if !showConfig && providers.length > 0 && models.length > 0}
		<!-- Normal welcome -->
		<div class="welcome">
			<div class="welcome-content">
				<img src="/icon.svg" alt="Prism" width="80" height="80" />
				<h1>Prism Agent</h1>
				<p>AI Agent 驱动的智能助手</p>
				<p>选择左侧 Agent 开始对话，或点击 <strong>+</strong> 创建新 Agent</p>
				<button class="link-btn" onclick={() => showConfig = true}>⚙ 修改配置</button>
			</div>
		</div>
	{:else}
		<!-- Quick Setup -->
		<div class="setup-page">
			<div class="setup-card">
				<img src="/icon.svg" alt="Prism" width="48" height="48" />
				<h1>Prism Agent</h1>
				<p class="subtitle">快速配置，3 步开始</p>

				{#if message}
					<div class="msg">{message}</div>
				{/if}

				<!-- Step 1: Provider -->
				<div class="step" class:done={providers.length > 0}>
					<div class="step-header">
						<span class="num">{providers.length > 0 ? '✓' : '1'}</span>
						<span>添加 Provider</span>
					</div>
					{#if providers.length === 0}
						<div class="step-body">
							<select bind:value={providerKind}>
								<option value="openai">OpenAI 兼容</option>
								<option value="ollama">Ollama (本地)</option>
								<option value="custom">自定义</option>
							</select>
							<input placeholder="名称（如 OpenAI）" bind:value={providerName} />
							<input
								placeholder={providerKind === 'ollama' ? 'http://localhost:11434/v1' : 'Base URL'}
								bind:value={providerUrl}
							/>
							<input type="password" placeholder="API Key" bind:value={providerKey} />
							<button class="btn" onclick={addProvider}>保存 Provider</button>
						</div>
					{:else}
						<div class="step-done">
							{#each providers as p}
								<span class="tag">{p.name}</span>
							{/each}
							<button class="link-btn small" onclick={() => showConfig = true}>管理</button>
						</div>
					{/if}
				</div>

				<!-- Step 2: Model -->
				<div class="step" class:done={models.length > 0} class:disabled={providers.length === 0}>
					<div class="step-header">
						<span class="num">{models.length > 0 ? '✓' : '2'}</span>
						<span>添加模型</span>
					</div>
					{#if providers.length > 0 && models.length === 0}
						<div class="step-body">
							<select bind:value={modelProviderId}>
								<option value="">选择 Provider</option>
								{#each providers as p}
									<option value={p.id}>{p.name}</option>
								{/each}
							</select>
							<input placeholder="模型 ID（如 gpt-4o、qwen2.5）" bind:value={modelId} />
							<label class="check">
								<input type="checkbox" bind:checked={modelDefault} /> 设为默认
							</label>
							<button class="btn" onclick={addModel}>保存模型</button>
						</div>
					{:else if models.length > 0}
						<div class="step-done">
							{#each models as m}
								<span class="tag">{m.display_name || m.model_id}{m.is_default ? ' ⭐' : ''}</span>
							{/each}
						</div>
					{:else}
						<p class="disabled-hint">请先添加 Provider</p>
					{/if}
				</div>

				<!-- Step 3: Agent -->
				<div class="step" class:disabled={models.length === 0}>
					<div class="step-header">
						<span class="num">3</span>
						<span>创建 Agent</span>
					</div>
					{#if models.length > 0}
						<div class="step-body">
							<p class="hint">点击下方按钮创建默认 Agent</p>
							<button class="btn" onclick={createAgent}>创建 Agent</button>
						</div>
					{:else}
						<p class="disabled-hint">请先添加模型</p>
					{/if}
				</div>

				{#if providers.length > 0 && models.length > 0}
					<a href="/" class="done-link">→ 开始使用</a>
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.chat-page { height: 100%; }

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
		gap: var(--space-3);
	}
	.welcome-content h1 { font-size: var(--text-3xl); font-weight: 700; margin: 0; }
	.welcome-content p { color: var(--color-fg-secondary); margin: 0; }

	.setup-page {
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--space-6);
	}
	.setup-card {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-xl);
		padding: var(--space-8);
		max-width: 480px;
		width: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--space-4);
	}
	.setup-card h1 { font-size: var(--text-2xl); font-weight: 700; margin: 0; }
	.subtitle { color: var(--color-fg-secondary); margin: 0; }

	.msg {
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-md);
		background: var(--color-green);
		color: #fff;
		font-size: var(--text-sm);
	}

	.step {
		width: 100%;
		background: var(--color-bg);
		border-radius: var(--radius-lg);
		padding: var(--space-4);
		transition: opacity var(--duration-fast);
	}
	.step.disabled { opacity: 0.5; pointer-events: none; }
	.step.done { border: 2px solid var(--color-green); }

	.step-header {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-weight: 600;
		margin-bottom: var(--space-3);
	}
	.num {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: var(--color-accent);
		color: #fff;
		display: flex;
		align-items: center;
		justify-content: center;
		font-size: var(--text-sm);
		flex-shrink: 0;
	}
	.step.done .num { background: var(--color-green); }

	.step-body {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	input, select {
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		font-size: var(--text-sm);
		outline: none;
	}
	input:focus, select:focus { border-color: var(--color-accent); }

	.btn {
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-md);
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: var(--text-sm);
		font-weight: 600;
		cursor: pointer;
		margin-top: var(--space-1);
	}
	.btn:hover { background: var(--color-accent-hover); }

	.check {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
		color: var(--color-fg-secondary);
		cursor: pointer;
	}

	.step-done {
		display: flex;
		flex-wrap: wrap;
		gap: var(--space-2);
		align-items: center;
	}
	.tag {
		padding: 2px 10px;
		border-radius: var(--radius-pill);
		background: rgba(52, 199, 89, 0.15);
		color: var(--color-green);
		font-size: var(--text-sm);
	}

	.link-btn {
		background: none;
		border: none;
		color: var(--color-accent);
		cursor: pointer;
		font-size: var(--text-sm);
		padding: 0;
		text-decoration: underline;
	}
	.link-btn.small { font-size: var(--text-xs); }

	.done-link {
		color: var(--color-accent);
		font-size: var(--text-lg);
		font-weight: 600;
		text-decoration: none;
		margin-top: var(--space-2);
	}
	.done-link:hover { text-decoration: underline; }

	.hint { font-size: var(--text-sm); color: var(--color-fg-secondary); margin: 0; }
	.disabled-hint { font-size: var(--text-sm); color: var(--color-fg-tertiary); margin: 0; }
</style>
