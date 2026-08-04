<script lang="ts">
	import { invoke } from '$lib/api/client';
	import { agentApi } from '$lib/api';

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
	let newProvider = $state({ name: '', kind: 'openai', base_url: '', api_key: '' });
	let newModel = $state({ provider_id: '', model_id: '', display_name: '', is_default: true });
	let message = $state('');
	let messageType = $state<'success' | 'error'>('success');
	let agentName = $state('助手');
	let agentDesc = $state('一个有用的 AI 助手');
	let agentSystemPrompt = $state('你是一个有用的 AI 助手。请用中文回答用户的问题。');

	function showMessage(text: string, type: 'success' | 'error' = 'success') {
		message = text;
		messageType = type;
		setTimeout(() => message = '', 3000);
	}

	async function loadProviders() {
		try {
			providers = await invoke<any[]>('model_providers');
			models = await invoke<any[]>('model_list');
		} catch (e) {
			console.error('Failed to load:', e);
		}
	}

	async function handleAddProvider() {
		if (!newProvider.name.trim()) {
			showMessage('请输入 Provider 名称', 'error');
			return;
		}
		try {
			await invoke('settings_add_provider', {
				name: newProvider.name.trim(),
				kind: newProvider.kind,
				base_url: newProvider.base_url.trim() || null,
				api_key: newProvider.api_key.trim() || null,
			});
			await loadProviders();
			newProvider = { name: '', kind: 'openai', base_url: '', api_key: '' };
			showMessage('Provider 添加成功');
		} catch (e) {
			showMessage('添加失败: ' + e, 'error');
		}
	}

	async function handleSaveKey(providerId: string) {
		const input = document.getElementById(`key-${providerId}`) as HTMLInputElement;
		if (!input || !input.value.trim()) return;
		try {
			await invoke('settings_save_provider_key', {
				provider_id: providerId,
				api_key: input.value.trim()
			});
			input.value = '';
			showMessage('API Key 已保存');
		} catch (e) {
			showMessage('保存失败: ' + e, 'error');
		}
	}

	async function handleAddModel() {
		if (!newModel.provider_id || !newModel.model_id.trim()) {
			showMessage('请选择 Provider 并输入模型 ID', 'error');
			return;
		}
		try {
			await invoke('settings_add_model', {
				provider_id: newModel.provider_id,
				model_id: newModel.model_id.trim(),
				display_name: newModel.display_name.trim() || null,
				is_default: newModel.is_default,
			});
			await loadProviders();
			newModel = { provider_id: '', model_id: '', display_name: '', is_default: true };
			showMessage('模型添加成功');
		} catch (e) {
			showMessage('添加失败: ' + e, 'error');
		}
	}

	async function handleCreateAgent() {
		if (!agentName.trim()) {
			showMessage('请输入 Agent 名称', 'error');
			return;
		}
		try {
			const agent = await agentApi.create(
				agentName.trim(),
				agentDesc.trim() || undefined,
				agentSystemPrompt.trim() || undefined
			);
			showMessage(`Agent "${agent.name}" 创建成功`);
		} catch (e) {
			showMessage('创建失败: ' + e, 'error');
		}
	}

	$effect(() => {
		loadProviders();
	});
</script>

<div class="settings-page">
	<h1>设置</h1>

	{#if message}
		<div class="toast" class:error={messageType === 'error'}>{message}</div>
	{/if}

	<!-- Step 1: Add Provider -->
	<section class="card">
		<div class="card-header">
			<span class="step-badge">1</span>
			<h2>添加 Provider</h2>
		</div>
		<p class="hint">选择你的 LLM 服务提供商，填写 API Key</p>

		<div class="form-grid">
			<div class="form-group">
				<label>名称</label>
				<input placeholder="如：OpenAI、Ollama、通义千问" bind:value={newProvider.name} />
			</div>
			<div class="form-group">
				<label>类型</label>
				<select bind:value={newProvider.kind}>
					<option value="openai">OpenAI 兼容</option>
					<option value="ollama">Ollama (本地)</option>
					<option value="anthropic">Anthropic</option>
					<option value="custom">自定义</option>
				</select>
			</div>
			<div class="form-group">
				<label>Base URL {#if newProvider.kind === 'ollama'}<span class="optional">（默认 http://localhost:11434/v1）</span>{/if}</label>
				<input
					placeholder={newProvider.kind === 'ollama' ? 'http://localhost:11434/v1' : 'https://api.openai.com/v1'}
					bind:value={newProvider.base_url}
				/>
			</div>
			<div class="form-group">
				<label>API Key</label>
				<input type="password" placeholder="sk-..." bind:value={newProvider.api_key} />
			</div>
		</div>
		<button class="btn-primary" onclick={handleAddProvider}>添加 Provider</button>
	</section>

	<!-- Step 2: Add Model -->
	<section class="card">
		<div class="card-header">
			<span class="step-badge">2</span>
			<h2>添加模型</h2>
		</div>
		<p class="hint">选择 Provider，输入模型 ID（如 gpt-4o、qwen2.5）</p>

		{#if providers.length === 0}
			<p class="empty-hint">请先添加 Provider</p>
		{:else}
			<div class="form-grid">
				<div class="form-group">
					<label>Provider</label>
					<select bind:value={newModel.provider_id}>
						<option value="">-- 选择 Provider --</option>
						{#each providers as p}
							<option value={p.id}>{p.name} ({p.kind})</option>
						{/each}
					</select>
				</div>
				<div class="form-group">
					<label>模型 ID</label>
					<input placeholder="如：gpt-4o、qwen2.5、claude-3-opus" bind:value={newModel.model_id} />
				</div>
				<div class="form-group">
					<label>显示名称（可选）</label>
					<input placeholder="如：GPT-4o" bind:value={newModel.display_name} />
				</div>
				<div class="form-group checkbox-group">
					<label>
						<input type="checkbox" bind:checked={newModel.is_default} />
						设为默认模型
					</label>
				</div>
			</div>
			<button class="btn-primary" onclick={handleAddModel}>添加模型</button>
		{/if}
	</section>

	<!-- Step 3: Create Agent -->
	<section class="card">
		<div class="card-header">
			<span class="step-badge">3</span>
			<h2>创建 Agent</h2>
		</div>
		<p class="hint">创建你的第一个 AI 助手</p>

		<div class="form-grid">
			<div class="form-group">
				<label>名称</label>
				<input placeholder="如：助手、翻译官、程序员" bind:value={agentName} />
			</div>
			<div class="form-group">
				<label>描述（可选）</label>
				<input placeholder="这个 Agent 能做什么" bind:value={agentDesc} />
			</div>
			<div class="form-group full-width">
				<label>系统提示词（可选）</label>
				<textarea rows="3" placeholder="你是一个有用的 AI 助手..." bind:value={agentSystemPrompt}></textarea>
			</div>
		</div>
		<button class="btn-primary" onclick={handleCreateAgent}>创建 Agent</button>
	</section>

	<!-- Current Config -->
	{#if providers.length > 0 || models.length > 0}
		<section class="card">
			<div class="card-header">
				<h2>当前配置</h2>
			</div>

			{#if providers.length > 0}
				<h3>Providers</h3>
				<div class="config-list">
					{#each providers as p}
						<div class="config-item">
							<div class="config-info">
								<strong>{p.name}</strong>
								<span class="badge">{p.kind}</span>
								{#if p.base_url}
									<span class="url">{p.base_url}</span>
								{/if}
							</div>
							<div class="key-input">
								<input
									id="key-{p.id}"
									type="password"
									placeholder="输入 API Key 更新"
								/>
								<button class="btn-sm" onclick={() => handleSaveKey(p.id)}>保存</button>
							</div>
						</div>
					{/each}
				</div>
			{/if}

			{#if models.length > 0}
				<h3>已添加模型</h3>
				<div class="config-list">
					{#each models as m}
						<div class="config-item">
							<div class="config-info">
								<strong>{m.display_name || m.model_id}</strong>
								<span class="model-id">{m.model_id}</span>
								{#if m.is_default}
									<span class="badge default">默认</span>
								{/if}
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</section>
	{/if}
</div>

<style>
	.settings-page {
		padding: var(--space-8);
		max-width: 680px;
		margin: 0 auto;
		overflow-y: auto;
		height: 100%;
	}
	h1 {
		font-size: var(--text-2xl);
		font-weight: 700;
		margin: 0 0 var(--space-6);
	}
	h2 {
		font-size: var(--text-lg);
		font-weight: 600;
		margin: 0;
	}
	h3 {
		font-size: var(--text-base);
		font-weight: 600;
		margin: var(--space-4) 0 var(--space-2);
		color: var(--color-fg-secondary);
	}
	.card {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-lg);
		padding: var(--space-6);
		margin-bottom: var(--space-4);
	}
	.card-header {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		margin-bottom: var(--space-2);
	}
	.step-badge {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background: var(--color-accent);
		color: #fff;
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 700;
		font-size: var(--text-sm);
		flex-shrink: 0;
	}
	.hint {
		color: var(--color-fg-secondary);
		font-size: var(--text-sm);
		margin: 0 0 var(--space-4);
	}
	.empty-hint {
		color: var(--color-fg-tertiary);
		font-size: var(--text-sm);
		padding: var(--space-4);
		text-align: center;
		background: var(--color-bg);
		border-radius: var(--radius-md);
	}
	.form-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--space-3);
		margin-bottom: var(--space-4);
	}
	.form-group {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}
	.form-group.full-width {
		grid-column: 1 / -1;
	}
	.checkbox-group {
		justify-content: flex-end;
	}
	.checkbox-group label {
		flex-direction: row;
		align-items: center;
		gap: var(--space-2);
		cursor: pointer;
		font-size: var(--text-sm);
	}
	label {
		font-size: var(--text-sm);
		font-weight: 500;
	}
	.optional {
		font-weight: 400;
		color: var(--color-fg-tertiary);
	}
	input, select, textarea {
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: var(--text-sm);
		font-family: var(--font-sans);
		outline: none;
		transition: border-color var(--duration-fast);
	}
	input:focus, select:focus, textarea:focus {
		border-color: var(--color-accent);
	}
	textarea { resize: vertical; }
	.btn-primary {
		padding: var(--space-2) var(--space-5);
		border-radius: var(--radius-md);
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: var(--text-sm);
		font-weight: 600;
		cursor: pointer;
		transition: background var(--duration-fast);
	}
	.btn-primary:hover { background: var(--color-accent-hover); }
	.btn-sm {
		padding: var(--space-1) var(--space-3);
		border-radius: var(--radius-sm);
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: var(--text-xs);
		cursor: pointer;
	}
	.config-list {
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.config-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-3);
		background: var(--color-bg);
		border-radius: var(--radius-md);
		gap: var(--space-3);
	}
	.config-info {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		flex: 1;
		min-width: 0;
	}
	.badge {
		padding: 2px 8px;
		border-radius: var(--radius-pill);
		font-size: var(--text-xs);
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
	}
	.badge.default {
		background: rgba(0, 113, 227, 0.15);
		color: var(--color-accent);
	}
	.url {
		font-size: var(--text-xs);
		color: var(--color-fg-tertiary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.model-id {
		font-size: var(--text-xs);
		color: var(--color-fg-tertiary);
		font-family: var(--font-mono);
	}
	.key-input {
		display: flex;
		gap: var(--space-1);
	}
	.key-input input {
		width: 200px;
	}
	.toast {
		position: fixed;
		bottom: var(--space-6);
		right: var(--space-6);
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-md);
		background: var(--color-green);
		color: #fff;
		font-size: var(--text-sm);
		z-index: 1000;
		animation: slideUp var(--duration-base) var(--spring);
	}
	.toast.error { background: var(--color-red); }
	@keyframes slideUp {
		from { opacity: 0; transform: translateY(16px); }
		to { opacity: 1; transform: translateY(0); }
	}
</style>
