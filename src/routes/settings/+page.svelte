<script lang="ts">
	import { invoke } from '$lib/api/client';
	import { agentApi } from '$lib/api';

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
	let newProvider = $state({ name: '', kind: 'openai', base_url: '', api_key: '' });
	let newModel = $state({ provider_id: '', model_id: '', display_name: '', is_default: false });
	let message = $state('');
	let agentName = $state('助手');
	let agentDesc = $state('一个有用的 AI 助手');
	let agentSystemPrompt = $state('你是一个有用的 AI 助手。请用中文回答用户的问题。');

	async function loadProviders() {
		providers = await invoke<any[]>('model_providers');
		models = await invoke<any[]>('model_list');
	}

	async function handleAddProvider() {
		if (!newProvider.name || !newProvider.kind) return;
		await invoke('settings_add_provider', {
			name: newProvider.name,
			kind: newProvider.kind,
			base_url: newProvider.base_url || null,
			api_key: newProvider.api_key || null,
		});
		await loadProviders();
		newProvider = { name: '', kind: 'openai', base_url: '', api_key: '' };
		message = 'Provider 添加成功';
		setTimeout(() => message = '', 3000);
	}

	async function handleSaveKey(providerId: string, key: string) {
		await invoke('settings_save_provider_key', { provider_id: providerId, api_key: key });
		message = 'API Key 已保存';
		setTimeout(() => message = '', 3000);
	}

	async function handleAddModel() {
		if (!newModel.provider_id || !newModel.model_id) return;
		await invoke('settings_add_model', {
			provider_id: newModel.provider_id,
			model_id: newModel.model_id,
			display_name: newModel.display_name || null,
			is_default: newModel.is_default,
		});
		await loadProviders();
		newModel = { provider_id: '', model_id: '', display_name: '', is_default: false };
		message = '模型添加成功';
		setTimeout(() => message = '', 3000);
	}

	async function handleCreateDefaultAgent() {
		const agent = await agentApi.create(
			agentName,
			agentDesc,
			agentSystemPrompt
		);
		message = `Agent "${agent.name}" 创建成功`;
		setTimeout(() => message = '', 3000);
	}

	$effect(() => {
		loadProviders();
	});
</script>

<div class="settings-page">
	<h1>设置</h1>

	{#if message}
		<div class="toast">{message}</div>
	{/if}

	<!-- Quick Setup -->
	<section class="card">
		<h2>快速开始</h2>
		<p class="hint">按顺序完成以下步骤即可开始对话</p>

		<div class="steps">
			<div class="step">
				<span class="step-num">1</span>
				<div class="step-content">
					<h3>添加 Provider</h3>
					<div class="form-row">
						<input placeholder="名称" bind:value={newProvider.name} />
						<select bind:value={newProvider.kind}>
							<option value="openai">OpenAI</option>
							<option value="ollama">Ollama (本地)</option>
							<option value="anthropic">Anthropic</option>
							<option value="custom">自定义</option>
						</select>
					</div>
					<input placeholder="Base URL（可选，Ollama 默认 http://localhost:11434/v1）" bind:value={newProvider.base_url} />
					<input placeholder="API Key" type="password" bind:value={newProvider.api_key} />
					<button class="btn" onclick={handleAddProvider}>添加 Provider</button>
				</div>
			</div>

			<div class="step">
				<span class="step-num">2</span>
				<div class="step-content">
					<h3>添加模型</h3>
					{#if providers.length > 0}
						<div class="form-row">
							<select bind:value={newModel.provider_id}>
								<option value="">选择 Provider</option>
								{#each providers as p}
									<option value={p.id}>{p.name}</option>
								{/each}
							</select>
							<input placeholder="模型 ID（如 gpt-4o）" bind:value={newModel.model_id} />
							<input placeholder="显示名称（可选）" bind:value={newModel.display_name} />
							<label class="checkbox-label">
								<input type="checkbox" bind:checked={newModel.is_default} /> 默认
							</label>
						</div>
						<button class="btn" onclick={handleAddModel}>添加模型</button>
					{:else}
						<p class="hint">请先添加 Provider</p>
					{/if}
				</div>
			</div>

			<div class="step">
				<span class="step-num">3</span>
				<div class="step-content">
					<h3>创建 Agent</h3>
					<input placeholder="Agent 名称" bind:value={agentName} />
					<input placeholder="描述" bind:value={agentDesc} />
					<textarea placeholder="系统提示词" bind:value={agentSystemPrompt} rows="3"></textarea>
					<button class="btn" onclick={handleCreateDefaultAgent}>创建 Agent</button>
				</div>
			</div>
		</div>
	</section>

	<!-- Current Config -->
	<section class="card">
		<h2>当前配置</h2>
		{#if providers.length === 0}
			<p class="hint">暂无 Provider</p>
		{:else}
			<table>
				<thead>
					<tr>
						<th>Provider</th>
						<th>类型</th>
						<th>Base URL</th>
						<th>Key</th>
					</tr>
				</thead>
				<tbody>
					{#each providers as p}
						<tr>
							<td>{p.name}</td>
							<td>{p.kind}</td>
							<td>{p.base_url || '-'}</td>
							<td>
								<input
									type="password"
									placeholder="输入 API Key"
									onblur={(e) => handleSaveKey(p.id, (e.target as HTMLInputElement).value)}
								/>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}

		{#if models.length > 0}
			<h3 style="margin-top: var(--space-4);">已添加模型</h3>
			<table>
				<thead>
					<tr>
						<th>模型</th>
						<th>Provider</th>
						<th>默认</th>
					</tr>
				</thead>
				<tbody>
					{#each models as m}
						<tr>
							<td>{m.display_name || m.model_id}</td>
							<td>{m.provider_id}</td>
							<td>{m.is_default ? '✓' : ''}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</section>
</div>

<style>
	.settings-page {
		padding: var(--space-8);
		max-width: 700px;
		margin: 0 auto;
	}
	h1 {
		font-size: var(--text-2xl);
		font-weight: 700;
		margin: 0 0 var(--space-6);
	}
	h2 {
		font-size: var(--text-lg);
		font-weight: 600;
		margin: 0 0 var(--space-3);
	}
	h3 {
		font-size: var(--text-base);
		font-weight: 600;
		margin: 0 0 var(--space-2);
	}
	.card {
		background: var(--color-bg-secondary);
		border-radius: var(--radius-lg);
		padding: var(--space-6);
		margin-bottom: var(--space-6);
	}
	.hint {
		color: var(--color-fg-secondary);
		font-size: var(--text-sm);
		margin: 0 0 var(--space-3);
	}
	.steps {
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}
	.step {
		display: flex;
		gap: var(--space-4);
	}
	.step-num {
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
	.step-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}
	.form-row {
		display: flex;
		gap: var(--space-2);
	}
	input, select, textarea {
		padding: var(--space-2) var(--space-3);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-md);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: var(--text-sm);
		font-family: var(--font-sans);
	}
	textarea { resize: vertical; }
	.btn {
		padding: var(--space-2) var(--space-4);
		border-radius: var(--radius-md);
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: var(--text-sm);
		cursor: pointer;
		align-self: flex-start;
	}
	.btn:hover { background: var(--color-accent-hover); }
	.checkbox-label {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}
	table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--text-sm);
	}
	th, td {
		text-align: left;
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--color-separator);
	}
	th { font-weight: 600; color: var(--color-fg-secondary); }
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
	}
</style>
