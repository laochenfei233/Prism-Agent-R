<script lang="ts">
	import { invoke } from '$lib/api/client';
	import { agentApi, mcpApi, memoryApi, settingsApi, skillApi } from '$lib/api';
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
		color: #FF6900;
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
	.form-row select:focus { border-color: #FF6900; }

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
		color: #FF6900;
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
	.key-input:focus { border-color: #FF6900; }

	/* ── Buttons ────────────────────────────────── */
	.btn-primary {
		width: 100%;
		padding: 12px;
		border-radius: 12px;
		border: none;
		background: #FF6900;
		color: #fff;
		font-size: 17px;
		font-weight: 600;
		cursor: pointer;
		transition: all 0.15s ease;
	}
	.btn-primary:hover { background: #E85D00; }
	.btn-primary:active { transform: scale(0.98); }

	.btn-secondary {
		width: 100%;
		padding: 10px;
		border-radius: 10px;
		border: 1px solid #FF6900;
		background: transparent;
		color: #FF6900;
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
	.btn-sm:hover { background: var(--color-bg); }
	.btn-sm.danger { color: #FF3B30; border-color: #FF3B30; }
	.btn-sm.danger:hover { background: rgba(255, 59, 48, 0.08); }
</style>
