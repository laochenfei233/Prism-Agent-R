<script lang="ts">
	import { goto } from '$app/navigation';
	import { invoke } from '$lib/api/client';
	import { agentApi } from '$lib/api';

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
	let message = $state('');

	// Form state
	let pName = $state('');
	let pKind = $state('openai');
	let pUrl = $state('');
	let pKey = $state('');
	let mProvider = $state('');
	let mModelId = $state('');

	async function load() {
		providers = await invoke<any[]>('model_providers');
		models = await invoke<any[]>('model_list');
	}

	async function saveProvider() {
		if (!pName.trim()) return alert('请输入 Provider 名称');
		await invoke('settings_add_provider', {
			name: pName.trim(), kind: pKind,
			base_url: pUrl.trim() || null, api_key: pKey.trim() || null
		});
		pName = ''; pUrl = ''; pKey = '';
		await load();
		message = '✓ Provider 已添加';
		setTimeout(() => message = '', 2000);
	}

	async function saveModel() {
		if (!mProvider || !mModelId.trim()) return alert('请选择 Provider 并输入模型 ID');
		await invoke('settings_add_model', {
			provider_id: mProvider, model_id: mModelId.trim(),
			display_name: null, is_default: true
		});
		mModelId = '';
		await load();
		message = '✓ 模型已添加';
		setTimeout(() => message = '', 2000);
	}

	async function createAgent() {
		await agentApi.create('助手', 'AI 助手', '你是一个有用的 AI 助手。请用中文回答。');
		message = '✓ Agent 已创建';
		setTimeout(() => message = '', 2000);
		await load();
	}

	$effect(() => { load(); });
</script>

<div class="page">
	{#if providers.length > 0 && models.length > 0}
		<!-- 已配置：显示欢迎 -->
		<div class="center">
			<img src="/icon.svg" alt="" width="72" height="72" />
			<h1>Prism Agent</h1>
			<p>选择左侧 Agent 开始对话</p>
			<button class="link" onclick={() => goto('/settings')}>⚙ 设置</button>
		</div>
	{:else}
		<!-- 未配置：显示设置向导 -->
		<div class="center">
			<img src="/icon.svg" alt="" width="56" height="56" />
			<h1>Prism Agent</h1>
			<p>开始使用前，请先配置模型</p>

			{#if message}
				<div class="toast">{message}</div>
			{/if}

			<div class="wizard">
				<!-- Step 1 -->
				<div class="step" class:active={providers.length === 0}>
					<div class="step-title">
						<span class="badge">1</span> 添加 Provider
					</div>
					{#if providers.length === 0}
						<div class="fields">
							<select bind:value={pKind}>
								<option value="openai">OpenAI 兼容</option>
								<option value="ollama">Ollama（本地）</option>
							</select>
							<input bind:value={pName} placeholder="名称，如 OpenAI" />
							<input bind:value={pUrl} placeholder="Base URL" />
							<input bind:value={pKey} type="password" placeholder="API Key" />
							<button class="btn" onclick={saveProvider}>保存 Provider</button>
						</div>
					{:else}
						<div class="done">
							{#each providers as p}<span class="tag">{p.name}</span>{/each}
						</div>
					{/if}
				</div>

				<!-- Step 2 -->
				<div class="step" class:active={providers.length > 0 && models.length === 0} class:locked={providers.length === 0}>
					<div class="step-title">
						<span class="badge">{providers.length > 0 && models.length === 0 ? '2' : models.length > 0 ? '✓' : '2'}</span> 添加模型
					</div>
					{#if providers.length > 0 && models.length === 0}
						<div class="fields">
							<select bind:value={mProvider}>
								<option value="">选择 Provider</option>
								{#each providers as p}<option value={p.id}>{p.name}</option>{/each}
							</select>
							<input bind:value={mModelId} placeholder="模型 ID，如 gpt-4o、qwen2.5" />
							<button class="btn" onclick={saveModel}>保存模型</button>
						</div>
					{:else if models.length > 0}
						<div class="done">
							{#each models as m}<span class="tag">{m.display_name || m.model_id}</span>{/each}
						</div>
					{:else}
						<p class="lock-hint">请先完成步骤 1</p>
					{/if}
				</div>

				<!-- Step 3 -->
				<div class="step" class:active={models.length > 0} class:locked={models.length === 0}>
					<div class="step-title">
						<span class="badge">3</span> 创建 Agent
					</div>
					{#if models.length > 0}
						<div class="fields">
							<button class="btn" onclick={createAgent}>创建默认 Agent</button>
						</div>
					{:else}
						<p class="lock-hint">请先完成步骤 2</p>
					{/if}
				</div>
			</div>
		</div>
	{/if}
</div>

<style>
	.page { height: 100%; display: flex; align-items: center; justify-content: center; }
	.center {
		text-align: center;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 12px;
	}
	h1 { font-size: 28px; font-weight: 700; margin: 0; }
	p { color: var(--color-fg-secondary); margin: 0; font-size: 15px; }

	.link {
		background: none; border: none; color: var(--color-accent);
		cursor: pointer; font-size: 14px; margin-top: 8px;
	}

	.toast {
		padding: 8px 16px; border-radius: 8px;
		background: #34C759; color: #fff; font-size: 14px;
	}

	.wizard {
		width: 380px; text-align: left;
		display: flex; flex-direction: column; gap: 12px;
		margin-top: 8px;
	}

	.step {
		background: var(--color-bg-secondary);
		border-radius: 12px; padding: 16px;
		opacity: 0.5; transition: opacity 0.2s;
	}
	.step.active { opacity: 1; border: 2px solid var(--color-accent); }
	.step.locked { opacity: 0.3; }

	.step-title {
		display: flex; align-items: center; gap: 8px;
		font-weight: 600; font-size: 15px; margin-bottom: 12px;
	}

	.badge {
		width: 22px; height: 22px; border-radius: 50%;
		background: var(--color-accent); color: #fff;
		display: flex; align-items: center; justify-content: center;
		font-size: 12px; font-weight: 700; flex-shrink: 0;
	}

	.fields {
		display: flex; flex-direction: column; gap: 8px;
	}

	input, select {
		padding: 8px 12px; border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg); color: var(--color-fg);
		font-size: 14px; outline: none;
	}
	input:focus, select:focus { border-color: var(--color-accent); }

	.btn {
		padding: 10px 16px; border-radius: 8px; border: none;
		background: var(--color-accent); color: #fff;
		font-size: 14px; font-weight: 600; cursor: pointer;
		margin-top: 4px;
	}
	.btn:hover { opacity: 0.9; }

	.done { display: flex; flex-wrap: wrap; gap: 6px; }
	.tag {
		padding: 4px 10px; border-radius: 12px;
		background: rgba(52,199,89,0.15); color: #34C759;
		font-size: 13px;
	}

	.lock-hint { font-size: 13px; color: var(--color-fg-tertiary); margin: 0; }
</style>
