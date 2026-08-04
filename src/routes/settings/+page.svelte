<script lang="ts">
	import { invoke } from '$lib/api/client';
	import { agentApi } from '$lib/api';

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
	let message = $state('');

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
		if (!pName.trim()) return;
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
		if (!mProvider || !mModelId.trim()) return;
		await invoke('settings_add_model', {
			provider_id: mProvider, model_id: mModelId.trim(),
			display_name: null, is_default: true
		});
		mModelId = '';
		await load();
		message = '✓ 模型已添加';
		setTimeout(() => message = '', 2000);
	}

	async function saveKey(id: string) {
		const el = document.getElementById('k-' + id) as HTMLInputElement;
		if (!el?.value.trim()) return;
		await invoke('settings_save_provider_key', { provider_id: id, api_key: el.value.trim() });
		el.value = '';
		message = '✓ Key 已保存';
		setTimeout(() => message = '', 2000);
	}

	$effect(() => { load(); });
</script>

<div class="page">
	<h1>设置</h1>
	{#if message}<div class="toast">{message}</div>{/if}

	<!-- Provider -->
	<div class="card">
		<h2>Provider</h2>
		<div class="row">
			<select bind:value={pKind}>
				<option value="openai">OpenAI 兼容</option>
				<option value="ollama">Ollama</option>
			</select>
			<input bind:value={pName} placeholder="名称" />
			<input bind:value={pUrl} placeholder="Base URL" />
			<input bind:value={pKey} type="password" placeholder="API Key" />
			<button class="btn" onclick={saveProvider}>添加</button>
		</div>
		{#if providers.length > 0}
			<div class="list">
				{#each providers as p}
					<div class="item">
						<span>{p.name}</span>
						<span class="dim">{p.kind}</span>
						<span class="dim">{p.base_url || '-'}</span>
						<input id="k-{p.id}" type="password" placeholder="更新 Key" style="width:150px" />
						<button class="btn-sm" onclick={() => saveKey(p.id)}>保存</button>
					</div>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Model -->
	<div class="card">
		<h2>模型</h2>
		<div class="row">
			<select bind:value={mProvider}>
				<option value="">选择 Provider</option>
				{#each providers as p}<option value={p.id}>{p.name}</option>{/each}
			</select>
			<input bind:value={mModelId} placeholder="模型 ID，如 gpt-4o" />
			<button class="btn" onclick={saveModel}>添加</button>
		</div>
		{#if models.length > 0}
			<div class="list">
				{#each models as m}
					<div class="item">
						<span>{m.display_name || m.model_id}</span>
						{#if m.is_default}<span class="tag">默认</span>{/if}
					</div>
				{/each}
			</div>
		{/if}
	</div>

	<!-- Agent -->
	<div class="card">
		<h2>Agent</h2>
		<div class="row">
			<button class="btn" onclick={async () => { await agentApi.create('助手'); await load(); message = '✓ Agent 已创建'; setTimeout(() => message = '', 2000); }}>创建默认 Agent</button>
		</div>
	</div>
</div>

<style>
	.page { padding: 32px; max-width: 600px; overflow-y: auto; height: 100%; }
	h1 { font-size: 24px; font-weight: 700; margin: 0 0 20px; }
	h2 { font-size: 16px; font-weight: 600; margin: 0 0 12px; }

	.card {
		background: var(--color-bg-secondary); border-radius: 12px;
		padding: 20px; margin-bottom: 16px;
	}
	.row { display: flex; gap: 8px; flex-wrap: wrap; }
	.list { margin-top: 12px; display: flex; flex-direction: column; gap: 6px; }
	.item {
		display: flex; align-items: center; gap: 8px; padding: 8px 12px;
		background: var(--color-bg); border-radius: 8px; font-size: 14px;
	}
	.dim { color: var(--color-fg-secondary); font-size: 12px; }

	input, select {
		padding: 8px 12px; border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg); color: var(--color-fg);
		font-size: 14px; outline: none; flex: 1; min-width: 100px;
	}
	.btn {
		padding: 8px 16px; border-radius: 8px; border: none;
		background: var(--color-accent); color: #fff;
		font-size: 14px; font-weight: 600; cursor: pointer;
	}
	.btn-sm {
		padding: 4px 10px; border-radius: 6px; border: 1px solid var(--color-separator);
		background: var(--color-bg); color: var(--color-fg);
		font-size: 12px; cursor: pointer;
	}
	.tag {
		padding: 2px 8px; border-radius: 10px;
		background: rgba(0,113,227,0.15); color: var(--color-accent);
		font-size: 12px;
	}
	.toast {
		padding: 8px 16px; border-radius: 8px;
		background: #34C759; color: #fff; font-size: 14px;
		margin-bottom: 16px; display: inline-block;
	}
</style>
