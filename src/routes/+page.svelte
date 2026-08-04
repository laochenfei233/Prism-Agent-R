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

	async function load() {
		providers = await invoke<any[]>('model-providers');
		models = await invoke<any[]>('model-list');
	}

	async function saveProvider() {
		if (!pName.trim()) { msg = '请输入名称'; return; }
		await invoke('settings-add-provider', {
			name: pName.trim(), kind: pKind,
			base_url: pUrl.trim() || null, api_key: pKey.trim() || null
		});
		pName = ''; pUrl = ''; pKey = '';
		await load();
		msg = '✓ Provider 已添加';
	}

	async function saveModel() {
		if (!mProvider || !mModelId.trim()) { msg = '请选择 Provider 并输入模型 ID'; return; }
		await invoke('settings-add-model', {
			provider_id: mProvider, model_id: mModelId.trim(),
			display_name: null, is_default: true
		});
		mModelId = '';
		await load();
		msg = '✓ 模型已添加';
	}

	async function createAgent() {
		await agentApi.create('助手', 'AI 助手', '你是一个有用的 AI 助手。请用中文回答。');
		msg = '✓ Agent 已创建，可以开始对话了';
		await load();
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
				<select bind:value={mProvider} style="padding:10px; border-radius:8px; border:1px solid #ccc; font-size:14px;">
					<option value="">选择 Provider</option>
					{#each providers as p}<option value={p.id}>{p.name}</option>{/each}
				</select>
				<input bind:value={mModelId} placeholder="模型 ID，如 gpt-4o、qwen2.5" style="padding:10px; border-radius:8px; border:1px solid #ccc; font-size:14px;" />
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
