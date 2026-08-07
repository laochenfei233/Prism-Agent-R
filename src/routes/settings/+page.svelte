<script lang="ts">
	import { invoke } from '$lib/api/client';
	import { agentApi, asrApi, mcpApi, memoryApi, projectIndexApi, ragApi, settingsApi, skillApi, translateApi, ttsApi } from '$lib/api';
	import type { SettingSpecDto } from '$lib/api';
	import SkillMarket from '$lib/components/market/SkillMarket.svelte';
	import Switch from '$lib/components/base/Switch.svelte';

	// ── 分组导航 ─────────────────────────────────────────
	const groups = [
		{ id: 'model_service', label: '模型服务', icon: 'model' },
		{ id: 'agent', label: 'Agent', icon: 'agent' },
		{ id: 'memory', label: '记忆', icon: 'memory' },
		{ id: 'tools', label: '工具', icon: 'tools' },
		{ id: 'rag', label: 'RAG', icon: 'rag' },
		{ id: 'meeting', label: '会议', icon: 'meeting' },
		{ id: 'security', label: '安全', icon: 'shield' },
		{ id: 'advanced', label: '高级', icon: 'gear' },
	];
	let activeGroup = $state('model_service');

	// ── 注册表设置项 ─────────────────────────────────────
	let specs = $state<SettingSpecDto[]>([]);
	let msg = $state('');
	let savingKey = $state('');

	async function loadSpecs() {
		try {
			specs = await settingsApi.getAll();
		} catch (e) {
			msg = '加载设置失败: ' + String(e);
		}
	}

	function specsOf(group: string) {
		return specs.filter((s) => s.group === group);
	}

	async function saveSpec(spec: SettingSpecDto, value: unknown) {
		if (savingKey === spec.key) return;
		savingKey = spec.key;
		try {
			const updated = await settingsApi.set(spec.key, value);
			const idx = specs.findIndex((s) => s.key === spec.key);
			if (idx >= 0) specs[idx] = updated;
			msg = `✓ ${spec.label} 已保存`;
		} catch (e) {
			msg = '保存失败: ' + String(e);
		} finally {
			savingKey = '';
		}
	}

	// ── Provider/Model ───────────────────────────────────
	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
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

	async function fetchModels() {
		if (!mProvider) return;
		loadingModels = true;
		availableModels = [];
		try {
			const result = await invoke<{ models: string[] }>('model_fetch_available', { provider_id: mProvider });
			availableModels = result.models || [];
		} catch (e) {
			msg = '拉取失败: ' + String(e);
		} finally {
			loadingModels = false;
		}
	}

	async function loadModels() {
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
			await loadModels();
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
			await loadModels();
			msg = '✓ 模型已添加';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	// ── 翻译模型 ─────────────────────────────────────────
	let translateModelId = $state('');
	let translateLoading = $state(false);
	async function loadTranslateModel() {
		try {
			const st = await translateApi.modelStatus();
			translateModelId = st.model_id || '';
		} catch (e) {}
	}
	async function saveTranslateModel() {
		translateLoading = true;
		try {
			await translateApi.modelConfig(translateModelId.trim() || undefined);
			msg = '✓ 翻译模型已保存';
		} catch (e) { msg = '错误: ' + String(e); }
		finally { translateLoading = false; }
	}

	// ── MCP ──────────────────────────────────────────────
	let mcpServers = $state<any[]>([]);
	let mcName = $state('');
	let mcType = $state('stdio');
	let mcCommand = $state('');
	let mcArgs = $state('');
	let mcUrl = $state('');

	async function loadMcp() {
		try { mcpServers = await mcpApi.list(); } catch (e) {}
	}

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
			await loadMcp();
			msg = '✓ MCP 服务器已添加';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function removeMcp(id: string) {
		try {
			await mcpApi.remove(id);
			await loadMcp();
			msg = '✓ MCP 已删除';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function testMcp(id: string) {
		try {
			const result = await mcpApi.test(id);
			msg = result.ok ? `✓ 连接成功 (${result.tools_count} 个工具, ${result.latency_ms}ms)` : `✗ 连接失败: ${result.error}`;
		} catch (e) { msg = '错误: ' + String(e); }
	}

	// ── Skill ────────────────────────────────────────────
	let skills = $state<any[]>([]);
	let skillPath = $state('');

	async function loadSkills() {
		try { skills = await skillApi.list(); } catch (e) {}
	}

	async function installSkill() {
		if (!skillPath.trim()) { msg = '请输入技能目录路径'; return; }
		try {
			await skillApi.install(skillPath.trim());
			skillPath = '';
			await loadSkills();
			msg = '✓ 技能已安装';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function uninstallSkill(id: string) {
		try {
			await skillApi.uninstall(id);
			await loadSkills();
			msg = '✓ 技能已卸载';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	// ── ASR 配置 ─────────────────────────────────────────
	let asrConfigs = $state<any[]>([]);
	let acName = $state('');
	let acKind = $state('DashScopeFunasr');
	let acBaseUrl = $state('');
	let acApiKey = $state('');
	let acModel = $state('');
	let acLang = $state('zh');
	let asrBackends = $state<any[]>([]);

	async function loadAsr() {
		try { asrConfigs = await asrApi.listConfigs(); } catch (e) {}
		try { asrBackends = await asrApi.backends(); } catch (e) {}
	}

	async function saveAsr() {
		if (!acName.trim()) { msg = '请输入 ASR 配置名称'; return; }
		try {
			await asrApi.saveConfig({
				name: acName.trim(), kind: acKind, base_url: acBaseUrl.trim() || undefined,
				api_key: acApiKey.trim() || undefined, model: acModel.trim() || undefined,
				lang: acLang.trim() || undefined, is_default: asrConfigs.length === 0,
			});
			acName = ''; acBaseUrl = ''; acApiKey = ''; acModel = '';
			await loadAsr();
			msg = '✓ ASR 配置已保存';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function deleteAsr(id: string) {
		try {
			await asrApi.deleteConfig(id);
			await loadAsr();
			msg = '✓ ASR 配置已删除';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	// ── RAG 嵌入 / contextual / rerank ──────────────────
	let embedMode = $state('local');
	let embedProvider = $state('');
	let embedModel = $state('');
	let embedDim = $state(256);
	let ragContextual = $state(true);
	let ragRerank = $state(false);

	async function loadRagStatus() {
		try {
			const st = await ragApi.embeddingStatus();
			embedMode = st.mode || 'local';
			embedProvider = st.provider_id || '';
			embedModel = st.model || '';
			embedDim = st.dim || 256;
		} catch (e) {}
		try { ragContextual = (await ragApi.contextualStatus()).enabled; } catch (e) {}
		try { ragRerank = (await ragApi.rerankStatus()).enabled; } catch (e) {}
	}

	async function saveEmbedding() {
		try {
			await ragApi.embeddingConfig(embedMode as 'local' | 'api', embedProvider || undefined, embedModel || undefined, embedDim);
			msg = '✓ 嵌入配置已保存';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function toggleContextual(v: boolean) {
		try { await ragApi.contextualConfig(v); msg = '✓ Contextual Retrieval 已更新'; }
		catch (e) { msg = '错误: ' + String(e); }
	}

	async function toggleRerank(v: boolean) {
		try { await ragApi.rerankConfig(v); msg = '✓ Reranker 已更新'; }
		catch (e) { msg = '错误: ' + String(e); }
	}

	// ── 项目索引 / 记忆 ─────────────────────────────────
	let projectIndex = $state({ enabled: true, workdir: null as string | null, indexed_files: 0, in_progress: false, last_indexed_at: null as number | null });
	let reconciling = $state(false);

	async function loadProjectIndex() {
		try { projectIndex = await projectIndexApi.status(); } catch (e) {}
	}

	async function toggleProjectIndex(v: boolean) {
		try { projectIndex = await projectIndexApi.toggle(v); msg = '✓ 项目索引已更新'; }
		catch (e) { msg = '错误: ' + String(e); }
	}

	async function reindexProject() {
		try { projectIndex = await projectIndexApi.reindex(); msg = '✓ 项目已重新索引'; }
		catch (e) { msg = '错误: ' + String(e); }
	}

	async function reconcileMemory() {
		if (reconciling) return;
		reconciling = true;
		try {
			const count = await memoryApi.reconcile();
			msg = `已索引 ${count} 个文件`;
		} catch (e) { msg = '错误: ' + String(e); }
		finally { reconciling = false; }
	}

	// ── 初始化 ───────────────────────────────────────────
	$effect(() => {
		loadSpecs();
		loadModels();
		loadMcp();
		loadSkills();
		loadAsr();
		loadRagStatus();
		loadProjectIndex();
		loadTranslateModel();
	});

	// ── 图标 ─────────────────────────────────────────────
	const icons: Record<string, string> = {
		model: '<circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18"/>',
		agent: '<circle cx="12" cy="7" r="4"/><path d="M5 21v-2a7 7 0 0 1 14 0v2"/>',
		memory: '<ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"/>',
		tools: '<path d="M14.7 6.3a4.5 4.5 0 0 0-6 6L3 18l3 3 5.7-5.7a4.5 4.5 0 0 0 6-6L14 13l-3-3z"/>',
		rag: '<path d="M4 6h16M4 12h16M4 18h10"/><circle cx="19" cy="18" r="2"/>',
		meeting: '<path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M23 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75"/>',
		shield: '<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>',
		gear: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h.01a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h.01a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v.01a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>',
	};
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
		<div class="toast" class:error={msg.startsWith('错误') || msg.startsWith('保存失败')}>{msg}</div>
	{/if}

	<div class="layout">
		<!-- 左侧分组导航 -->
		<nav class="side-nav">
			{#each groups as g}
				<button
					class="nav-item"
					class:active={activeGroup === g.id}
					onclick={() => (activeGroup = g.id)}
				>
					<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
						{@html icons[g.icon]}
					</svg>
					<span>{g.label}</span>
				</button>
			{/each}
		</nav>

		<!-- 右侧内容区 -->
		<div class="content">
			{#if activeGroup === 'model_service'}
				<!-- 模型服务：Provider + 模型 + 翻译模型 -->
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

				<div class="group">
					<div class="group-header">翻译专用模型</div>
					<div class="group-body">
						<p class="hint">留空则使用默认模型。</p>
						<div class="form-row">
							<input bind:value={translateModelId} placeholder="模型 ID，如 gpt-4o-mini" />
						</div>
						<button class="btn-primary" onclick={saveTranslateModel} disabled={translateLoading}>
							{translateLoading ? '保存中…' : '保存'}
						</button>
					</div>
				</div>

				<!-- 注册表项 -->
				{#each specsOf('model_service') as spec}
					{@render SettingRow(spec, (v) => saveSpec(spec, v))}
				{/each}
			{:else if activeGroup === 'agent'}
				<div class="group">
					<div class="group-header">Agent 默认参数</div>
					<div class="group-body">
						<p class="hint">以下参数应用于新建 Agent。</p>
					</div>
				</div>
				{#each specsOf('agent') as spec}
					{@render SettingRow(spec, (v) => saveSpec(spec, v))}
				{/each}
			{:else if activeGroup === 'memory'}
				<div class="group">
					<div class="group-header">记忆管理</div>
					<div class="group-body">
						<p class="hint">记忆存储于 global/projects/sessions 目录的 .md 文件，重建索引可回填全文搜索（memory_fts）。</p>
						<button class="btn-primary" onclick={reconcileMemory} disabled={reconciling}>
							{reconciling ? '索引中…' : '重建索引'}
						</button>
					</div>
				</div>
				{#each specsOf('memory') as spec}
					{@render SettingRow(spec, (v) => saveSpec(spec, v))}
				{/each}
			{:else if activeGroup === 'tools'}
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
			{:else if activeGroup === 'rag'}
				<!-- 嵌入配置 -->
				<div class="group">
					<div class="group-header">嵌入配置</div>
					<div class="group-body">
						<div class="form-row">
							<select bind:value={embedMode}>
								<option value="local">本地嵌入</option>
								<option value="api">API 嵌入</option>
							</select>
						</div>
						{#if embedMode === 'api'}
							<div class="form-row">
								<input bind:value={embedProvider} placeholder="Provider ID" />
							</div>
							<div class="form-row">
								<input bind:value={embedModel} placeholder="嵌入模型，如 text-embedding-3-small" />
							</div>
						{/if}
						<div class="form-row">
							<input bind:value={embedDim} type="number" placeholder="维度" />
						</div>
						<button class="btn-primary" onclick={saveEmbedding}>保存嵌入配置</button>
					</div>
				</div>

				<!-- Contextual / Rerank -->
				<div class="group">
					<div class="group-header">检索增强</div>
					<div class="group-body">
						<div class="config-row">
							<div class="config-info">
								<span class="config-name">Contextual Retrieval</span>
							</div>
							<Switch checked={ragContextual} onchange={(v) => { ragContextual = v; toggleContextual(v); }} />
						</div>
						<div class="config-row">
							<div class="config-info">
								<span class="config-name">Reranker 重排序</span>
							</div>
							<Switch checked={ragRerank} onchange={(v) => { ragRerank = v; toggleRerank(v); }} />
						</div>
					</div>
				</div>

				{#each specsOf('rag') as spec}
					{@render SettingRow(spec, (v) => saveSpec(spec, v))}
				{/each}
			{:else if activeGroup === 'meeting'}
				<!-- ASR 配置 -->
				<div class="group">
					<div class="group-header">ASR 语音识别配置</div>
					<div class="group-body">
						<div class="form-row">
							<input bind:value={acName} placeholder="配置名称" />
							<select bind:value={acKind}>
								{#each asrBackends as b}<option value={b.kind}>{b.name}</option>{/each}
							</select>
						</div>
						<div class="form-row">
							<input bind:value={acBaseUrl} placeholder="Base URL（云端后端）" />
							<input bind:value={acApiKey} type="password" placeholder="API Key" />
						</div>
						<div class="form-row">
							<input bind:value={acModel} placeholder="模型（如 paraformer-realtime-v2）" />
							<input bind:value={acLang} placeholder="语言（zh/en）" />
						</div>
						<button class="btn-primary" onclick={saveAsr}>保存 ASR 配置</button>

						{#if asrConfigs.length > 0}
							<div class="divider"></div>
							{#each asrConfigs as cfg}
								<div class="config-row">
									<div class="config-info">
										<span class="config-name">{cfg.name}</span>
										<span class="config-badge">{cfg.kind}</span>
										{#if cfg.is_default}<span class="config-badge default">默认</span>{/if}
									</div>
									<button class="btn-sm danger" onclick={() => deleteAsr(cfg.id)}>删除</button>
								</div>
							{/each}
						{/if}
					</div>
				</div>

				{#each specsOf('meeting') as spec}
					{@render SettingRow(spec, (v) => saveSpec(spec, v))}
				{/each}
			{:else if activeGroup === 'security'}
				{#each specsOf('security') as spec}
					{@render SettingRow(spec, (v) => saveSpec(spec, v))}
				{/each}
			{:else if activeGroup === 'advanced'}
				<!-- 项目索引 -->
				<div class="group">
					<div class="group-header">项目自动索引</div>
					<div class="group-body">
						<div class="config-row">
							<div class="config-info">
								<span class="config-name">启用自动索引</span>
								{#if projectIndex.workdir}<span class="config-badge">{projectIndex.workdir}</span>{/if}
							</div>
							<Switch checked={projectIndex.enabled} onchange={(v) => { projectIndex.enabled = v; toggleProjectIndex(v); }} />
						</div>
						{#if projectIndex.enabled}
							<button class="btn-secondary" onclick={reindexProject} disabled={projectIndex.in_progress}>
								{projectIndex.in_progress ? '索引中…' : `重新索引（${projectIndex.indexed_files} 文件）`}
							</button>
						{/if}
					</div>
				</div>

				{#each specsOf('advanced') as spec}
					{@render SettingRow(spec, (v) => saveSpec(spec, v))}
				{/each}
			{/if}
		</div>
	</div>
</div>

<!-- 注册表设置项行（通用渲染） -->
{#snippet SettingRow(spec: SettingSpecDto, onsave: (v: unknown) => void)}
	<div class="group">
		<div class="group-header">{spec.label}</div>
		<div class="group-body">
			<p class="hint">{spec.description}</p>
			<div class="form-row">
				{#if spec.kind === 'bool'}
					<Switch checked={!!spec.value} onchange={(v) => { spec.value = v; onsave(v); }} />
				{:else if spec.kind === 'select'}
					<select
						value={spec.value as string}
						onchange={(e) => onsave((e.currentTarget as HTMLSelectElement).value)}
					>
						{#each spec.options || [] as opt}<option value={opt}>{opt}</option>{/each}
					</select>
				{:else if spec.kind === 'int' || spec.kind === 'float'}
					<div class="num-row">
						<input
							type="number"
							value={spec.value as number}
							min={spec.min ?? undefined}
							max={spec.max ?? undefined}
							step={spec.step ?? (spec.kind === 'int' ? 1 : 0.1)}
							onchange={(e) => onsave(Number((e.currentTarget as HTMLInputElement).value))}
						/>
						<button class="btn-sm" onclick={() => onsave(spec.value)}>应用</button>
					</div>
				{:else}
					<input
						type="text"
						value={spec.value as string}
						onchange={(e) => onsave((e.currentTarget as HTMLInputElement).value)}
					/>
				{/if}
			</div>
		</div>
	</div>
{/snippet}

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

	/* ── Layout ─────────────────────────────────── */
	.layout {
		display: flex;
		gap: 0;
		min-height: calc(100% - 52px);
	}

	.side-nav {
		width: 200px;
		flex-shrink: 0;
		padding: 16px 8px;
		display: flex;
		flex-direction: column;
		gap: 2px;
		border-right: 0.5px solid var(--color-separator);
		background: var(--color-glass);
		backdrop-filter: saturate(180%) blur(20px);
	}
	.nav-item {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 9px 12px;
		border: none;
		border-radius: 10px;
		background: transparent;
		color: var(--color-fg);
		font-size: 14px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.15s ease;
	}
	.nav-item:hover { background: var(--color-bg-hover); }
	.nav-item.active {
		background: color-mix(in srgb, var(--color-accent) 14%, transparent);
		color: var(--color-accent);
		font-weight: 600;
	}

	.content {
		flex: 1;
		padding: 16px;
		overflow-y: auto;
		min-width: 0;
	}

	/* ── Group ──────────────────────────────────── */
	.group { margin-bottom: 16px; }
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

	.num-row {
		display: flex;
		gap: 8px;
		flex: 1;
	}
	.num-row input { flex: 1; }

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
		flex-wrap: wrap;
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

	.config-actions { display: flex; gap: 8px; }
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

	/* ── Responsive ─────────────────────────────── */
	@media (max-width: 640px) {
		.layout { flex-direction: column; }
		.side-nav {
			width: 100%;
			flex-direction: row;
			overflow-x: auto;
			border-right: none;
			border-bottom: 0.5px solid var(--color-separator);
			padding: 8px;
		}
		.nav-item { white-space: nowrap; }
	}
</style>

