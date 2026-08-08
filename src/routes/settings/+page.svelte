<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke, listen } from '$lib/api/client';
	import { agentApi, asrApi, mcpApi, memoryApi, settingsApi, skillApi } from '$lib/api';
	import SkillMarket from '$lib/components/market/SkillMarket.svelte';

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
	let mcpServers = $state<any[]>([]);
	let skills = $state<any[]>([]);
	let msg = $state('');

	// Provider/Model
	let pName = $state('');
	let pKind = $state('openai');
	let pUrl = $state('');
	let pKey = $state('');
	let editKeyProviderId = $state<string | null>(null);
	let editKeyValue = $state('');
	let keySaving = $state(false);
	let mProvider = $state('');
	let mModelId = $state('');
	let availableModels = $state<string[]>([]);
	let loadingModels = $state(false);

	// ASR 语音识别（从会议页移入）
	let asrBackends = $state<any[]>([]);
	let asrCatalog = $state<any[]>([]);
	let asrInstalled = $state<any[]>([]);
	let asrConfigs = $state<any[]>([]);
	let asrDownloadProgress = $state<Record<string, number>>({});
	let asrShowAddConfig = $state(false);
	let asrNewConfig = $state<any>({
		name: '本地 SenseVoice', kind: 'SherpaOnnx', is_default: false,
		model_path: '', lang: 'zh'
	});
	let asrModelPathInput = $state('');

	// MCP
	let mcName = $state('');
	let mcType = $state('stdio');
	let mcCommand = $state('');
	let mcArgs = $state('');
	let mcUrl = $state('');

	// Skill
	let skillPath = $state('');

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
		try { mcpServers = await mcpApi.list(); } catch (e) {}
		try { skills = await skillApi.list(); } catch (e) {}
		try { loadAsr(); } catch (e) {}
	}

	// ── ASR 语音识别 ─────────────────────────────────────
	async function loadAsr() {
		try {
			[asrBackends, asrCatalog, asrInstalled, asrConfigs] = await Promise.all([
				asrApi.backends(), asrApi.modelCatalog(), asrApi.modelInstalled(), asrApi.listConfigs()
			]);
		} catch (e) { console.error(e); }
	}

	async function asrDownloadModel(id: string) {
		try { await asrApi.modelDownload(id); } catch (e) { console.error(e); }
	}

	async function asrRemoveModel(id: string) {
		if (!confirm('删除模型？')) return;
		try { await asrApi.modelRemove(id); await loadAsr(); } catch (e) { console.error(e); }
	}

	async function asrTestConfig() {
		try {
			const res = await asrApi.test({ ...asrNewConfig, model_path: asrModelPathInput || undefined });
			alert(res.ok ? `连接正常（${res.latency_ms}ms）` : `失败：${res.error}`);
		} catch (e) { console.error(e); }
	}

	async function asrSaveConfig() {
		try {
			await asrApi.saveConfig({ ...asrNewConfig, model_path: asrModelPathInput || undefined, api_key: asrNewConfig.api_key || undefined });
			asrShowAddConfig = false;
			await loadAsr();
		} catch (e) { console.error(e); }
	}

	async function asrDeleteConfig(id: string) {
		try { await asrApi.deleteConfig(id); await loadAsr(); } catch (e) { console.error(e); }
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

	function startEditKey(providerId: string) {
		editKeyProviderId = providerId;
		editKeyValue = '';
	}

	function cancelEditKey() {
		editKeyProviderId = null;
		editKeyValue = '';
	}

	async function saveProviderKey(providerId: string) {
		if (!editKeyValue.trim() || keySaving) return;
		keySaving = true;
		try {
			await settingsApi.saveProviderKey(providerId, editKeyValue.trim());
			cancelEditKey();
			msg = '✓ Key 已保存';
		} catch (e) { msg = '错误: ' + String(e); }
		finally { keySaving = false; }
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

	// MCP
	async function addMcp() {
		if (!mcName.trim()) { msg = '请输入 MCP 名称'; return; }
		try {
			const args = mcArgs.trim() ? mcArgs.split(/\s+/) : [];
			if (mcType === 'stdio') {
				await mcpApi.add({ name: mcName.trim(), type: 'stdio', command: mcCommand.trim(), args });
			} else {
				await mcpApi.add({ name: mcName.trim(), type: 'http', base_url: mcUrl.trim(), args });
			}
			mcName = ''; mcCommand = ''; mcArgs = ''; mcUrl = '';
			await load();
			msg = '✓ MCP 服务器已添加';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function removeMcp(id: string) {
		try {
			await mcpApi.remove(id);
			await load();
			msg = '✓ MCP 已删除';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function testMcp(id: string) {
		try {
			const result = await mcpApi.test(id);
			msg = result.ok ? `✓ 连接成功 (${result.tools_count} 个工具, ${result.latency_ms}ms)` : `✗ 连接失败: ${result.error}`;
		} catch (e) { msg = '错误: ' + String(e); }
	}

	// Skill
	async function installSkill() {
		if (!skillPath.trim()) { msg = '请输入技能目录路径'; return; }
		try {
			await skillApi.install(skillPath.trim());
			skillPath = '';
			await load();
			msg = '✓ 技能已安装';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function uninstallSkill(id: string) {
		try {
			await skillApi.uninstall(id);
			await load();
			msg = '✓ 技能已卸载';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	// Memory
	let reconciling = $state(false);

	async function reconcileMemory() {
		if (reconciling) return;
		reconciling = true;
		try {
			const count = await memoryApi.reconcile();
			msg = `已索引 ${count} 个文件`;
		} catch (e) { msg = '错误: ' + String(e); }
		finally { reconciling = false; }
	}

	$effect(() => { load(); });

	let asrListenerCleanup: (() => void) | null = null;

	onMount(async () => {
		// ASR 模型下载进度事件
		const un = await listen<{ model_id: string; progress: number; message: string }>('asr:model-download-progress', (e) => {
			asrDownloadProgress[e.model_id] = e.progress;
			if (e.progress >= 1) loadAsr();
		});
		asrListenerCleanup = un;
	});
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
						<div class="config-actions">
							{#if editKeyProviderId === p.id}
								<input
									class="key-input"
									bind:value={editKeyValue}
									type="password"
									placeholder="新 API Key"
									onkeydown={(e) => { if (e.key === 'Enter') saveProviderKey(p.id); }}
									disabled={keySaving}
								/>
								<button class="btn-sm" onclick={() => saveProviderKey(p.id)} disabled={keySaving || !editKeyValue.trim()}>
									{keySaving ? '保存中…' : '保存'}
								</button>
								<button class="btn-sm" onclick={cancelEditKey} disabled={keySaving}>取消</button>
							{:else}
								<button class="btn-sm" onclick={() => startEditKey(p.id)}>编辑 Key</button>
							{/if}
						</div>
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

	<!-- 语音识别（ASR） -->
	<div class="group">
		<div class="group-header">语音识别 (ASR)</div>
		<div class="group-body">
			<div class="section-title">可用后端</div>
			{#each asrBackends as b}
				<div class="config-row">
					<div class="config-info">
						<span class="config-name">{b.name}</span>
						<span class="config-badge">{b.languages.join(', ')}</span>
					</div>
				</div>
			{/each}

			<div class="divider"></div>
			<div class="section-title">模型管理</div>
			{#each asrCatalog as m}
				<div class="config-row">
					<div class="config-info">
						<span class="config-name">{m.name}</span>
						<span class="config-badge">{m.backend} · {m.size_mb}MB</span>
					</div>
					<div class="config-actions">
						{#if asrDownloadProgress[m.id] !== undefined && asrDownloadProgress[m.id] < 1}
							<span class="config-badge default">{(asrDownloadProgress[m.id] * 100).toFixed(0)}%</span>
						{:else if asrInstalled.some(i => i.id === m.id)}
							<button class="btn-sm danger" onclick={() => asrRemoveModel(m.id)}>删除</button>
						{:else}
							<button class="btn-sm" onclick={() => asrDownloadModel(m.id)}>下载</button>
						{/if}
					</div>
				</div>
			{/each}

			<div class="divider"></div>
			<div class="section-title">ASR 配置</div>
			<button class="btn-secondary" onclick={() => asrShowAddConfig = !asrShowAddConfig}>+ 新建配置</button>
			{#if asrShowAddConfig}
				<div class="asr-form">
					<input bind:value={asrNewConfig.name} placeholder="名称（如 本地 SenseVoice）" />
					<select bind:value={asrNewConfig.kind}>
						{#each asrBackends as b}<option value={b.kind}>{b.name}</option>{/each}
					</select>
					<input bind:value={asrModelPathInput} placeholder="模型路径（本地后端，如 asr_models/sherpa-sensevoice-small）" />
					{#if asrNewConfig.kind.includes('Http') || asrNewConfig.kind === 'Custom' || asrNewConfig.kind === 'WhisperApi'}
						<input bind:value={asrNewConfig.api_key} placeholder="API Key" />
					{/if}
					<div class="form-row">
						<button class="btn-sm" onclick={asrTestConfig}>测试连接</button>
						<button class="btn-primary" onclick={asrSaveConfig}>保存</button>
					</div>
				</div>
			{/if}
			{#each asrConfigs as c}
				<div class="config-row">
					<div class="config-info">
						<span class="config-name">{c.name}</span>
						<span class="config-badge">{c.kind}</span>
						{#if c.model_path}<span class="config-badge">{c.model_path}</span>{/if}
					</div>
					<button class="btn-sm danger" onclick={() => asrDeleteConfig(c.id)}>删</button>
				</div>
			{/each}
			{#if asrConfigs.length === 0}<p class="hint">暂无配置</p>{/if}
		</div>
	</div>

	<!-- Agent -->
	<div class="group">
		<div class="group-header">Agent</div>
		<div class="group-body">
			<button class="btn-green" onclick={createAgent}>创建默认 Agent</button>
		</div>
	</div>

	<!-- MCP -->
	<div class="group">
		<div class="group-header">MCP 服务器</div>
		<div class="group-body">
			<div class="form-row">
				<input bind:value={mcName} placeholder="名称" />
				<select bind:value={mcType}>
					<option value="stdio">Stdio</option>
					<option value="http">HTTP</option>
				</select>
			</div>
			{#if mcType === 'stdio'}
				<div class="form-row">
					<input bind:value={mcCommand} placeholder="命令，如 npx" />
				</div>
				<div class="form-row">
					<input bind:value={mcArgs} placeholder="参数（空格分隔），如 -y @modelcontextprotocol/server-filesystem" />
				</div>
			{:else}
				<div class="form-row">
					<input bind:value={mcUrl} placeholder="URL，如 http://localhost:3000/sse" />
				</div>
			{/if}
			<button class="btn-primary" onclick={addMcp}>添加 MCP</button>

			{#if mcpServers.length > 0}
				<div class="divider"></div>
				{#each mcpServers as mc}
					<div class="config-row">
						<div class="config-info">
							<span class="config-name">{mc.name}</span>
							<span class="config-badge">{mc.type}</span>
						</div>
						<div class="config-actions">
							<button class="btn-sm" onclick={() => testMcp(mc.id)}>测试</button>
							<button class="btn-sm danger" onclick={() => removeMcp(mc.id)}>删除</button>
						</div>
					</div>
				{/each}
			{/if}
		</div>
	</div>

	<!-- Skill -->
	<div class="group">
		<div class="group-header">技能</div>
		<div class="group-body">
			<div class="form-row">
				<input bind:value={skillPath} placeholder="技能目录路径，如 /path/to/my-skill" />
			</div>
			<button class="btn-primary" onclick={installSkill}>安装技能</button>

			{#if skills.length > 0}
				<div class="divider"></div>
				{#each skills as skill}
					<div class="config-row">
						<div class="config-info">
							<span class="config-name">{skill.name}</span>
							<span class="config-badge">{skill.source}</span>
							{#if skill.is_enabled}<span class="config-badge default">已启用</span>{/if}
						</div>
						<button class="btn-sm danger" onclick={() => uninstallSkill(skill.id)}>卸载</button>
					</div>
				{/each}
			{/if}
		</div>
	</div>

	<!-- Skill Market -->
	<div class="group">
		<div class="group-header">Market</div>
		<div class="group-body">
			<SkillMarket />
		</div>
	</div>

	<!-- Memory -->
	<div class="group">
		<div class="group-header">记忆管理</div>
		<div class="group-body">
			<p class="hint">记忆存储于 global/projects/sessions 目录的 .md 文件，重建索引可回填全文搜索（memory_fts）。</p>
			<button class="btn-primary" onclick={reconcileMemory} disabled={reconciling}>
				{reconciling ? '索引中…' : '重建索引'}
			</button>
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
		background: var(--color-glass);
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
		color: var(--color-accent);
		font-size: 17px;
		cursor: pointer;
		border-radius: 8px;
	}
	.nav-back:hover { background: var(--color-bg-hover); }
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
		background: var(--color-green);
		color: #fff;
		font-size: 15px;
	}
	.toast.error { background: var(--color-red); }

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
	.form-row select:focus { border-color: var(--color-accent); }

	.hint {
		font-size: 14px;
		color: var(--color-fg-tertiary);
		margin: 0;
		padding: 8px 0;
	}

	.section-title {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		margin: 4px 0 8px;
	}

	.asr-form {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin: 10px 0;
	}
	.asr-form input,
	.asr-form select {
		width: 100%;
		padding: 10px 12px;
		border-radius: 10px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		font-size: 14px;
		outline: none;
		box-sizing: border-box;
	}
	.asr-form input:focus,
	.asr-form select:focus { border-color: var(--color-accent); }
	.asr-form .btn-primary,
	.asr-form .btn-secondary {
		width: auto;
		padding: 8px 16px;
		font-size: 14px;
		margin-bottom: 0;
	}
	.asr-form .form-row { align-items: center; }

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
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
		color: var(--color-accent);
	}
	.key-input {
		width: 180px;
		padding: 6px 10px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		font-size: 13px;
		outline: none;
	}
	.key-input:focus { border-color: var(--color-accent); }

	/* ── Buttons ────────────────────────────────── */
	.btn-primary {
		width: 100%;
		padding: 12px;
		border-radius: 12px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: 17px;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.15s ease;
	}
	.btn-primary:hover { background: var(--color-accent-hover); }
	.btn-primary:active { transform: scale(0.98); }

	.btn-secondary {
		width: 100%;
		padding: 10px;
		border-radius: 10px;
		border: 1px solid var(--color-accent);
		background: transparent;
		color: var(--color-accent);
		font-size: 15px;
		font-weight: 500;
		cursor: pointer;
		margin-bottom: 10px;
	}
	.btn-secondary:hover { background: color-mix(in srgb, var(--color-accent) 8%, transparent); }
	.btn-secondary:disabled { opacity: 0.5; cursor: not-allowed; }

	.btn-green {
		width: 100%;
		padding: 12px;
		border-radius: 12px;
		border: none;
		background: var(--color-green);
		color: #fff;
		font-size: 17px;
		font-weight: 600;
		cursor: pointer;
	}
	.btn-green:hover { background: color-mix(in srgb, var(--color-green) 85%, #000); }
	.btn-green:active { transform: scale(0.98); }

	/* ── Small Buttons ────────────────────────── */
	.config-actions {
		display: flex;
		gap: 8px;
	}
	.btn-sm {
		padding: 4px 12px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		font-size: 13px;
		cursor: pointer;
		transition: all 0.15s ease;
	}
	.btn-sm:hover { background: var(--color-bg-hover); }
	.btn-sm.danger { color: var(--color-red); border-color: var(--color-red); }
	.btn-sm.danger:hover { background: color-mix(in srgb, var(--color-red) 8%, transparent); }
</style>
