<script lang="ts">
	import { invoke } from '$lib/api/client';
	import { agentApi } from '$lib/api';

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
	let msg = $state('');

	let pName = $state('');
	let pKind = $state('openai');
	let pUrl = $state('');
	let pKey = $state('');
	let mProvider = $state('');
	let mModelId = $state('');
	let availableModels = $state<string[]>([]);
	let loadingModels = $state(false);

	async function fetchModels() {
		if (!mProvider) return;
		loadingModels = true;
		availableModels = [];
		try {
			const result = await invoke<{models: string[]}>('model_fetch_available', { providerId: mProvider });
			availableModels = result.models || [];
		} catch (e) {
			console.error('Fetch models error:', e);
			msg = '拉取模型失败: ' + String(e);
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
			console.error('Error:', e);
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
			console.error('Error:', e);
			msg = '错误: ' + String(e);
		}
	}

	async function createAgent() {
		try {
			await agentApi.create('助手', 'AI 助手', '你是一个有用的 AI 助手。请用中文回答。');
			msg = '✓ Agent 已创建，可以开始对话了';
			await load();
		} catch (e) {
			console.error('Error:', e);
			msg = '错误: ' + String(e);
		}
	}

	$effect(() => { load(); });
</script>

<div style="padding:32px; max-width:500px;">
	<h1 style="font-size:24px; margin:0 0 20px;">Prism Agent - 快速配置</h1>

	{#if msg}
		<div style="padding:10px 16px; background:#34C759; color:#fff; border-radius:8px; margin-bottom:16px; font-size:14px;">
			{msg}
		</div>
	{/if}

	<!-- Provider -->
	<div style="background:var(--color-bg-secondary); border-radius:12px; padding:20px; margin-bottom:16px;">
		<h2 style="font-size:16px; margin:0 0 12px;">第 1 步：添加 Provider</h2>
		<div style="display:flex; flex-direction:column; gap:8px;">
			<select bind:value={pKind} style="padding:10px; border-radius:8px; border:1px solid #ccc; font-size:14px;">
				<option value="openai">OpenAI 兼容</option>
				<option value="ollama">Ollama（本地）</option>
			</select>
			<input bind:value={pName} placeholder="名称，如 OpenAI" style="padding:10px; border-radius:8px; border:1px solid #ccc; font-size:14px;" />
			<input bind:value={pUrl} placeholder="Base URL" style="padding:10px; border-radius:8px; border:1px solid #ccc; font-size:14px;" />
			<input bind:value={pKey} type="password" placeholder="API Key" style="padding:10px; border-radius:8px; border:1px solid #ccc; font-size:14px;" />
			<button onclick={saveProvider} style="padding:12px; border-radius:8px; border:none; background:#0071E3; color:#fff; font-size:16px; font-weight:600; cursor:pointer; margin-top:4px;">
				保存 Provider
			</button>
		</div>
		{#if providers.length > 0}
			<div style="margin-top:12px; font-size:13px; color:#666;">
				已添加：{providers.map(p => p.name).join(', ')}
			</div>
		{/if}
	</div>

	<!-- Model -->
	<div style="background:var(--color-bg-secondary); border-radius:12px; padding:20px; margin-bottom:16px;">
		<h2 style="font-size:16px; margin:0 0 12px;">第 2 步：添加模型</h2>
		{#if providers.length === 0}
			<p style="color:#999; font-size:14px;">请先完成第 1 步</p>
		{:else}
			<div style="display:flex; flex-direction:column; gap:8px;">
				<select bind:value={mProvider} onchange={() => { availableModels = []; mModelId = ''; }} style="padding:10px; border-radius:8px; border:1px solid #ccc; font-size:14px;">
					<option value="">选择 Provider</option>
					{#each providers as p}<option value={p.id}>{p.name}</option>{/each}
				</select>

				{#if mProvider}
					<button onclick={fetchModels} disabled={loadingModels} style="padding:8px; border-radius:8px; border:1px solid #0071E3; background:transparent; color:#0071E3; font-size:14px; cursor:pointer;">
						{loadingModels ? '拉取中...' : '拉取可用模型列表'}
					</button>
				{/if}

				{#if availableModels.length > 0}
					<select bind:value={mModelId} style="padding:10px; border-radius:8px; border:1px solid #ccc; font-size:14px;">
						<option value="">-- 选择模型 --</option>
						{#each availableModels as m}<option value={m}>{m}</option>{/each}
					</select>
				{:else}
					<input bind:value={mModelId} placeholder="模型 ID，如 gpt-4o、qwen2.5" style="padding:10px; border-radius:8px; border:1px solid #ccc; font-size:14px;" />
				{/if}

				<button onclick={saveModel} style="padding:12px; border-radius:8px; border:none; background:#0071E3; color:#fff; font-size:16px; font-weight:600; cursor:pointer; margin-top:4px;">
					保存模型
				</button>
			</div>
		{/if}
		{#if models.length > 0}
			<div style="margin-top:12px; font-size:13px; color:#666;">
				已添加：{models.map(m => m.display_name || m.model_id).join(', ')}
			</div>
		{/if}
	</div>

	<!-- Agent -->
	<div style="background:var(--color-bg-secondary); border-radius:12px; padding:20px; margin-bottom:16px;">
		<h2 style="font-size:16px; margin:0 0 12px;">第 3 步：创建 Agent</h2>
		{#if models.length === 0}
			<p style="color:#999; font-size:14px;">请先完成第 2 步</p>
		{:else}
			<button onclick={createAgent} style="padding:12px; border-radius:8px; border:none; background:#34C759; color:#fff; font-size:16px; font-weight:600; cursor:pointer;">
				创建默认 Agent
			</button>
		{/if}
	</div>
</div>
