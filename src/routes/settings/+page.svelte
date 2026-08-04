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
				baseUrl: pUrl.trim() || null, apiKey: pKey.trim() || null
			});
			pName = ''; pUrl = ''; pKey = '';
			await load();
			msg = '✓ Provider 已添加';
		} catch (e) { msg = '错误: ' + String(e); }
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
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function createAgent() {
		try {
			await agentApi.create('助手', 'AI 助手', '你是一个有用的 AI 助手。请用中文回答。');
			msg = '✓ Agent 已创建';
			await load();
		} catch (e) { msg = '错误: ' + String(e); }
	}

	$effect(() => { load(); });
</script>

<div class="page">
	<div class="nav">
		<button class="nav-back" onclick={() => history.back()}>
			<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<polyline points="15 18 9 12 15 6"/>
			</svg>
			返回
		</button>
		<h1 class="nav-title">设置</h1>
		<div></div>
	</div>

	{#if msg}
		<div class="toast" class:error={msg.startsWith('错误')}>{msg}</div>
	{/if}

	<!-- Provider -->
	<div class="group">
		<div class="group-header">Provider</div>
		<div class="group-body">
			<div class="form-row">
				<select bind:value={pKind}>
					<option value="openai">OpenAI 兼容</option>
					<option value="ollama">Ollama</option>
				</select>
				<input bind:value={pName} placeholder="名称" />
			</div>
			<div class="form-row">
				<input bind:value={pUrl} placeholder="Base URL" />
				<input bind:value={pKey} type="password" placeholder="API Key" />
			</div>
			<button class="btn-primary" onclick={saveProvider}>添加 Provider</button>

			{#if providers.length > 0}
				<div class="divider"></div>
				{#each providers as p}
					<div class="config-row">
						<div class="config-info">
							<span class="config-name">{p.name}</span>
							<span class="config-badge">{p.kind}</span>
						</div>
						<span class="config-url">{p.base_url || '-'}</span>
					</div>
				{/each}
			{/if}
		</div>
	</div>

	<!-- Model -->
	<div class="group">
		<div class="group-header">模型</div>
		<div class="group-body">
			{#if providers.length === 0}
				<p class="hint">请先添加 Provider</p>
			{:else}
				<div class="form-row">
					<select bind:value={mProvider} onchange={() => { availableModels = []; mModelId = ''; }}>
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
					<div class="form-row">
						<select bind:value={mModelId}>
							<option value="">选择模型</option>
							{#each availableModels as m}<option value={m}>{m}</option>{/each}
						</select>
					</div>
				{:else}
					<div class="form-row">
						<input bind:value={mModelId} placeholder="模型 ID，如 gpt-4o" />
					</div>
				{/if}
				<button class="btn-primary" onclick={saveModel}>添加模型</button>
			{/if}

			{#if models.length > 0}
				<div class="divider"></div>
				{#each models as m}
					<div class="config-row">
						<div class="config-info">
							<span class="config-name">{m.display_name || m.model_id}</span>
							{#if m.is_default}<span class="config-badge default">默认</span>{/if}
						</div>
					</div>
				{/each}
			{/if}
		</div>
	</div>

	<!-- Agent -->
	<div class="group">
		<div class="group-header">Agent</div>
		<div class="group-body">
			<button class="btn-green" onclick={createAgent}>创建默认 Agent</button>
		</div>
	</div>
</div>

<style>
	.page {
		padding: 0;
		overflow-y: auto;
		height: 100%;
		background: var(--color-bg-secondary);
	}

	/* ── Nav ────────────────────────────────────── */
	.nav {
		position: sticky;
		top: 0;
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 12px 16px;
		min-height: 52px;
		background: rgba(242, 242, 247, 0.94);
		backdrop-filter: saturate(180%) blur(20px);
		border-bottom: 0.5px solid var(--color-separator);
		z-index: 100;
	}
	.nav-back {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 4px 8px;
		border: none;
		background: transparent;
		color: #007AFF;
		font-size: 17px;
		cursor: pointer;
		border-radius: 8px;
	}
	.nav-back:hover { background: rgba(0, 122, 255, 0.08); }
	.nav-title {
		font-size: 17px;
		font-weight: 600;
		color: var(--color-fg);
		letter-spacing: -0.41px;
		margin: 0;
	}

	/* ── Toast ──────────────────────────────────── */
	.toast {
		padding: 10px 16px;
		margin: 16px 16px 0;
		border-radius: 10px;
		background: #34C759;
		color: #fff;
		font-size: 15px;
	}
	.toast.error { background: #FF3B30; }

	/* ── Group ──────────────────────────────────── */
	.group {
		margin: 16px;
	}
	.group-header {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		padding: 0 0 8px;
	}
	.group-body {
		background: var(--color-bg);
		border-radius: 12px;
		padding: 12px;
	}

	/* ── Form ───────────────────────────────────── */
	.form-row {
		display: flex;
		gap: 8px;
		margin-bottom: 10px;
	}
	.form-row input,
	.form-row select {
		flex: 1;
		padding: 10px 12px;
		border-radius: 10px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		font-size: 15px;
		outline: none;
	}
	.form-row input:focus,
	.form-row select:focus { border-color: #007AFF; }

	.hint {
		font-size: 14px;
		color: var(--color-fg-tertiary);
		margin: 0;
		padding: 8px 0;
	}

	.divider {
		height: 0.5px;
		background: var(--color-separator);
		margin: 12px 0;
	}

	/* ── Config List ────────────────────────────── */
	.config-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 8px 0;
	}
	.config-info {
		display: flex;
		align-items: center;
		gap: 8px;
	}
	.config-name {
		font-size: 15px;
		color: var(--color-fg);
	}
	.config-badge {
		padding: 2px 8px;
		border-radius: 6px;
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
		font-size: 12px;
	}
	.config-badge.default {
		background: rgba(0, 122, 255, 0.12);
		color: #007AFF;
	}
	.config-url {
		font-size: 13px;
		color: var(--color-fg-tertiary);
	}

	/* ── Buttons ────────────────────────────────── */
	.btn-primary {
		width: 100%;
		padding: 12px;
		border-radius: 12px;
		border: none;
		background: #007AFF;
		color: #fff;
		font-size: 17px;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.15s ease;
	}
	.btn-primary:hover { background: #0066D6; }
	.btn-primary:active { transform: scale(0.98); }

	.btn-secondary {
		width: 100%;
		padding: 10px;
		border-radius: 10px;
		border: 1px solid #007AFF;
		background: transparent;
		color: #007AFF;
		font-size: 15px;
		font-weight: 500;
		cursor: pointer;
		margin-bottom: 10px;
	}
	.btn-secondary:hover { background: rgba(0, 122, 255, 0.08); }
	.btn-secondary:disabled { opacity: 0.5; cursor: not-allowed; }

	.btn-green {
		width: 100%;
		padding: 12px;
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
</style>
