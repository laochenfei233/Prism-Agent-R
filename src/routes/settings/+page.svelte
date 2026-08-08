<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { invoke, listen } from '$lib/api/client';
	import { agentApi, asrApi, mcpApi, memoryApi, settingsApi, skillApi } from '$lib/api';
	import SkillMarket from '$lib/components/market/SkillMarket.svelte';

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
	let mcpServers = $state<any[]>([]);
	let skills = $state<any[]>([]);
	let msg = $state('');
	// 是否已完成首载（区分"加载中"与"暂无"）
	let loaded = $state(false);
	// 当前激活的设置分类（Cherry Studio 风格左导航）
	let section = $state<'providers' | 'asr' | 'tts' | 'agents' | 'mcp' | 'skills' | 'market' | 'memory'>('providers');
	// 当前选中的 Provider（Cherry Studio 风格：左侧列表 + 右侧详情）
	let selectedProviderId = $state<string | null>(null);
	// 是否处于"添加 Provider"模式（右侧显示表单）
	let addingProvider = $state(false);

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

	// 切换 Provider 分类时自动选中第一个
	$effect(() => {
		if (section === 'providers') {
			if (!selectedProviderId && providers.length > 0) {
				selectedProviderId = providers[0].id;
			}
		}
	});

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
		loaded = false;
		providers = await invoke<any[]>('model_providers');
		models = await invoke<any[]>('model_list');
		try { mcpServers = await mcpApi.list(); } catch (e) {}
		try { skills = await skillApi.list(); } catch (e) {}
		try { loadAsr(); } catch (e) {}
		loaded = true;
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
			addingProvider = false;
			await load();
			// 显式选中新添加的 Provider（位于列表首位）
			if (providers.length > 0) {
				selectedProviderId = providers[0].id;
			}
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
		// 优先用当前选中的 Provider，其次用下拉框选择
		const providerId = mProvider || selectedProviderId;
		if (!providerId || !mModelId.trim()) { msg = '请选择 Provider 并输入模型 ID'; return; }
		try {
			await invoke('settings_add_model', {
				providerId, modelId: mModelId.trim(),
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

	// toast 4 秒后自动消失
	let toastTimer: ReturnType<typeof setTimeout> | null = null;
	$effect(() => {
		if (!msg) return;
		if (toastTimer) clearTimeout(toastTimer);
		toastTimer = setTimeout(() => { msg = ''; }, 4000);
		return () => { if (toastTimer) clearTimeout(toastTimer); };
	});

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

{#if msg}
	<div class="toast" class:error={msg.startsWith('错误')} role="status" aria-live="polite">{msg}</div>
{/if}

<div class="settings-shell">
	<!-- 左侧分类导航 -->
	<aside class="settings-nav">
		<div class="nav-header">
			<button class="nav-back" onclick={() => goto('/')} title="返回聊天" aria-label="返回聊天">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
			</button>
			<h1 class="nav-title">设置</h1>
		</div>
		<div class="nav-scroll">
			<div class="nav-group-title">模型管理</div>
			<button class="nav-item" class:active={section === 'providers'} onclick={() => section = 'providers'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
				<span>Provider & 模型</span>
			</button>
			<button class="nav-item" class:active={section === 'asr'} onclick={() => section = 'asr'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" y1="19" x2="12" y2="23"/></svg>
				<span>语音识别</span>
			</button>
			<button class="nav-item" class:active={section === 'tts'} onclick={() => section = 'tts'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 5 6 9H2v6h4l5 4V5z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/></svg>
				<span>语音合成</span>
			</button>

			<div class="nav-group-title">能力</div>
			<button class="nav-item" class:active={section === 'agents'} onclick={() => section = 'agents'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z"/></svg>
				<span>Agent</span>
			</button>
			<button class="nav-item" class:active={section === 'mcp'} onclick={() => section = 'mcp'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="2" width="20" height="8" rx="2"/><rect x="2" y="14" width="20" height="8" rx="2"/><line x1="6" y1="6" x2="6.01" y2="6"/><line x1="6" y1="18" x2="6.01" y2="18"/></svg>
				<span>MCP 服务器</span>
			</button>
			<button class="nav-item" class:active={section === 'skills'} onclick={() => section = 'skills'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m12 2 2.4 4.9 5.4.8-3.9 3.8.9 5.4-4.8-2.5-4.8 2.5.9-5.4L4.2 7.7l5.4-.8z"/></svg>
				<span>技能</span>
			</button>
			<button class="nav-item" class:active={section === 'market'} onclick={() => section = 'market'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 8l-9-5-9 5v8l9 5 9-5V8z"/><path d="M3 8l9 5 9-5"/><path d="M12 13v8"/></svg>
				<span>Market</span>
			</button>

			<div class="nav-group-title">数据</div>
			<button class="nav-item" class:active={section === 'memory'} onclick={() => section = 'memory'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>
				<span>记忆管理</span>
			</button>
		</div>
	</aside>

	<!-- 右侧内容 -->
	<main class="settings-content">
		{#if section === 'providers'}
			{@const sel = providers.find(p => p.id === selectedProviderId) ?? null}
			<div class="provider-shell">
				<!-- Provider 列表（Cherry Studio 风格左子栏） -->
				<div class="provider-list-pane">
					<div class="pane-title">Provider</div>
					<button class="add-provider-btn" onclick={() => { pName = ''; pUrl = ''; pKey = ''; pKind = 'openai'; addingProvider = true; }}>
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
						添加 Provider
					</button>
					<div class="pane-list">
						{#each providers as p}
							<button
								class="pane-item"
								class:active={selectedProviderId === p.id}
								onclick={() => selectedProviderId = p.id}
							>
								<span class="pane-item-name">{p.name}</span>
								<span class="pane-item-kind">{p.kind}</span>
							</button>
						{/each}
						{#if providers.length === 0}
							<div class="pane-empty">{loaded ? '暂无 Provider' : '加载中...'}</div>
						{/if}
					</div>
				</div>

				<!-- Provider 详情（连接 + 模型管理） -->
				<div class="provider-detail-pane">
					{#if sel && !addingProvider}
						<div class="content-header">
							<h2 class="content-title">{sel.name}</h2>
							<p class="content-desc">配置连接参数并管理该服务商的模型</p>
						</div>

						<!-- 连接设置 -->
						<div class="card detail-card">
							<div class="section-title">连接设置</div>
							<div class="form-row">
								<input value={sel.base_url || ''} disabled placeholder="Base URL" aria-label="Base URL" />
							</div>
							<div class="config-row">
								<div class="config-info">
									<span class="config-name">API Key</span>
									{#if editKeyProviderId === sel.id}
										<input
											class="key-input"
											bind:value={editKeyValue}
											type="password"
											placeholder="新 API Key"
											onkeydown={(e) => { if (e.key === 'Enter') saveProviderKey(sel.id); }}
											disabled={keySaving}
											aria-label="新 API Key"
										/>
										<button class="btn-sm" onclick={() => saveProviderKey(sel.id)} disabled={keySaving || !editKeyValue.trim()}>
											{keySaving ? '保存中…' : '保存'}
										</button>
										<button class="btn-sm" onclick={cancelEditKey} disabled={keySaving}>取消</button>
									{:else}
										<button class="btn-sm" onclick={() => startEditKey(sel.id)}>编辑 Key</button>
									{/if}
								</div>
							</div>
						</div>

						<!-- 模型管理 -->
						<div class="card detail-card">
							<div class="section-title">模型</div>
							<div class="form-row">
								<select bind:value={mProvider} onchange={() => { availableModels = []; mModelId = ''; }}>
									<option value="">选择 Provider</option>
									{#each providers as p}<option value={p.id}>{p.name}</option>{/each}
								</select>
								<input bind:value={mModelId} placeholder="模型 ID，如 gpt-4o" aria-label="模型 ID,如 gpt-4o" />
								<button class="btn-secondary" onclick={fetchModels} disabled={loadingModels}>
									{loadingModels ? '拉取中...' : '拉取'}
								</button>
								<button class="btn-primary" onclick={saveModel}>添加</button>
							</div>
							{#if availableModels.length > 0}
								<div class="form-row">
									<select bind:value={mModelId}>
										<option value="">选择模型</option>
										{#each availableModels as m}<option value={m}>{m}</option>{/each}
									</select>
								</div>
							{/if}

							<div class="divider"></div>
							{#if models.filter(m => m.provider_id === sel.id).length === 0}
								<p class="hint">该服务商暂无模型，请在上面添加</p>
							{:else}
								{#each models.filter(m => m.provider_id === sel.id) as m}
									<div class="config-row">
										<div class="config-info">
											<span class="config-name">{m.display_name || m.model_id}</span>
											{#if m.is_default}<span class="config-badge default">默认</span>{/if}
										</div>
									</div>
								{/each}
							{/if}
						</div>
					{:else}
						<!-- 添加 Provider 模式 -->
						<div class="content-header">
							<h2 class="content-title">Provider & 模型</h2>
							<p class="content-desc">添加模型服务商并配置其模型</p>
						</div>
						<div class="card detail-card">
							<div class="section-title">添加 Provider</div>
							<div class="form-row">
								<select bind:value={pKind}>
									<option value="openai">OpenAI 兼容</option>
									<option value="ollama">Ollama</option>
								</select>
								<input bind:value={pName} placeholder="名称" aria-label="名称" />
							</div>
							<div class="form-row">
								<input bind:value={pUrl} placeholder="Base URL" aria-label="Base URL" />
								<input bind:value={pKey} type="password" placeholder="API Key" aria-label="API Key" />
							</div>
							<div class="form-row">
								<button class="btn-primary" onclick={saveProvider}>添加 Provider</button>
								{#if sel}
									<button class="btn-sm" onclick={() => addingProvider = false}>取消</button>
								{/if}
							</div>
						</div>
					{/if}
				</div>
			</div>

	{:else if section === 'asr'}
			<div class="provider-shell">
				<!-- 后端列表 -->
				<div class="provider-list-pane">
					<div class="pane-title">ASR 后端</div>
					<div class="pane-list">
						{#each asrBackends as b}
							<div class="pane-item" class:active={true}>
								<span class="pane-item-name">{b.name}</span>
								<span class="pane-item-kind">{b.languages.join(', ')}</span>
							</div>
						{/each}
						{#if asrBackends.length === 0}
							<div class="pane-empty">{loaded ? '暂无后端' : '加载中...'}</div>
						{/if}
					</div>
				</div>

				<!-- 详情：模型管理 + 配置 -->
				<div class="provider-detail-pane">
					<div class="content-header">
						<h2 class="content-title">语音识别 (ASR)</h2>
						<p class="content-desc">配置会议录音的转写模型与后端连接</p>
					</div>

					<div class="card detail-card">
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
						{#if asrCatalog.length === 0}<p class="hint">{loaded ? '暂无可用模型' : '加载中...'}</p>{/if}
					</div>

					<div class="card detail-card">
						<div class="section-title">ASR 配置</div>
						<button class="btn-secondary" onclick={() => asrShowAddConfig = !asrShowAddConfig}>+ 新建配置</button>
						{#if asrShowAddConfig}
							<div class="asr-form">
								<input bind:value={asrNewConfig.name} placeholder="名称（如 本地 SenseVoice）" aria-label="名称如 本地 SenseVoice" />
								<select bind:value={asrNewConfig.kind}>
									{#each asrBackends as b}<option value={b.kind}>{b.name}</option>{/each}
								</select>
								<input bind:value={asrModelPathInput} placeholder="模型路径（本地后端，如 asr_models/sherpa-sensevoice-small）" aria-label="模型路径本地后端,如 asr_models/sherpa-sensevoice-small" />
								{#if asrNewConfig.kind.includes('Http') || asrNewConfig.kind === 'Custom' || asrNewConfig.kind === 'WhisperApi'}
									<input bind:value={asrNewConfig.api_key} placeholder="API Key" aria-label="API Key" />
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
								<button class="btn-sm danger" onclick={() => asrDeleteConfig(c.id)}>删除</button>
							</div>
						{/each}
						{#if asrConfigs.length === 0}<p class="hint">暂无配置</p>{/if}
					</div>
				</div>
			</div>

		{:else if section === 'tts'}
			<div class="content-header">
				<h2 class="content-title">语音合成 (TTS)</h2>
				<p class="content-desc">配置文本转语音的后端与音色</p>
			</div>
			<div class="card">
				<p class="hint">TTS 使用浏览器内置的 Web Speech API 播报（如会议待办播报），无需额外配置模型。</p>
			</div>

		{:else if section === 'agents'}
			<div class="content-header">
				<h2 class="content-title">Agent</h2>
				<p class="content-desc">创建与配置对话 Agent</p>
			</div>
			<div class="card">
				<p class="hint">创建一个使用默认模型的通用助手，可在左侧 Agent 列表中管理。</p>
				<button class="btn-green" onclick={createAgent}>创建默认 Agent</button>
			</div>

		{:else if section === 'mcp'}
			<div class="content-header">
				<h2 class="content-title">MCP 服务器</h2>
				<p class="content-desc">连接外部工具服务（Model Context Protocol）</p>
			</div>
			<div class="card">
				<div class="card-head">
					<div class="form-row">
						<input bind:value={mcName} placeholder="名称" aria-label="名称" />
						<select bind:value={mcType}>
							<option value="stdio">Stdio</option>
							<option value="http">HTTP</option>
						</select>
					</div>
					{#if mcType === 'stdio'}
						<div class="form-row">
							<input bind:value={mcCommand} placeholder="命令，如 npx" aria-label="命令,如 npx" />
						</div>
						<div class="form-row">
							<input bind:value={mcArgs} placeholder="参数（空格分隔），如 -y @modelcontextprotocol/server-filesystem" aria-label="参数空格分隔,如 -y @modelcontextprotocol/server-filesystem" />
						</div>
					{:else}
						<div class="form-row">
							<input bind:value={mcUrl} placeholder="URL，如 http://localhost:3000/sse" aria-label="URL,如 http://localhost:3000/sse" />
						</div>
					{/if}
					<button class="btn-primary" onclick={addMcp}>添加 MCP</button>
				</div>

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

		{:else if section === 'skills'}
			<div class="content-header">
				<h2 class="content-title">技能</h2>
				<p class="content-desc">安装与管理 Prompt 技能包</p>
			</div>
			<div class="card">
				<div class="card-head">
					<div class="form-row">
						<input bind:value={skillPath} placeholder="技能目录路径，如 /path/to/my-skill" aria-label="技能目录路径,如 /path/to/my-skill" />
					</div>
					<button class="btn-primary" onclick={installSkill}>安装技能</button>
				</div>

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

		{:else if section === 'market'}
			<div class="content-header">
				<h2 class="content-title">Market</h2>
				<p class="content-desc">浏览与安装技能市场</p>
			</div>
			<div class="card">
				<SkillMarket />
			</div>

		{:else if section === 'memory'}
			<div class="content-header">
				<h2 class="content-title">记忆管理</h2>
				<p class="content-desc">管理跨会话的持久化记忆</p>
			</div>
			<div class="card">
				<p class="hint">记忆存储于 global/projects/sessions 目录的 .md 文件，重建索引可回填全文搜索（memory_fts）。</p>
				<button class="btn-primary" onclick={reconcileMemory} disabled={reconciling}>
					{reconciling ? '索引中…' : '重建索引'}
				</button>
			</div>
		{/if}
	</main>
</div>

<style>
	.settings-shell {
		display: flex;
		height: 100%;
		background: var(--color-bg);
	}

	/* ── 左侧导航 ─────────────────────────── */
	.settings-nav {
		width: 220px;
		min-width: 220px;
		background: var(--color-bg-secondary);
		border-right: 1px solid var(--color-separator);
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}
	.nav-header {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 14px 16px 12px;
		font-size: 20px;
		font-weight: 600;
		color: var(--color-fg);
		letter-spacing: -0.41px;
		border-bottom: 1px solid var(--color-separator);
	}
	.nav-back {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border: none;
		border-radius: 8px;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		transition: background 0.15s ease;
	}
	.nav-back:hover { background: var(--color-bg-tertiary); color: var(--color-fg); }
	.nav-title { margin: 0; font-size: 20px; font-weight: 600; color: var(--color-fg); letter-spacing: -0.41px; }
	.nav-scroll {
		flex: 1;
		overflow-y: auto;
		padding: 8px;
	}
	.nav-group-title {
		padding: 12px 12px 4px;
		font-size: 12px;
		font-weight: 500;
		color: var(--color-fg-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.4px;
	}
	.nav-item {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 8px 12px;
		border: none;
		border-radius: 8px;
		background: transparent;
		color: var(--color-fg-secondary);
		font-size: 14px;
		cursor: pointer;
		text-align: left;
		transition: background 0.15s ease;
	}
	.nav-item:hover { background: var(--color-bg-tertiary); }
	.nav-item.active { background: var(--color-bg-tertiary); color: var(--color-fg); font-weight: 500; }
	.nav-icon { flex-shrink: 0; opacity: 0.85; }

	/* ── 右侧内容 ─────────────────────────── */
	.settings-content {
		flex: 1;
		min-width: 0;
		overflow-y: auto;
		padding: 28px 32px;
	}
	.content-header { margin-bottom: 20px; }
	.content-title {
		font-size: 18px;
		font-weight: 600;
		color: var(--color-fg);
		margin: 0 0 4px;
	}
	.content-desc {
		font-size: 13px;
		color: var(--color-fg-secondary);
		margin: 0;
	}

	/* ── Provider 两栏（Cherry Studio 风格） ──── */
	.provider-shell {
		display: flex;
		gap: 20px;
		height: 100%;
		min-height: 0;
	}
	.provider-list-pane {
		width: 160px;
		min-width: 160px;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-secondary);
		border-right: 1px solid var(--color-separator);
		overflow: hidden;
	}
	.pane-title {
		padding: 14px 16px 8px;
		font-size: 13px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		text-transform: uppercase;
		letter-spacing: 0.4px;
	}
	.add-provider-btn {
		display: flex;
		align-items: center;
		gap: 6px;
		margin: 0 8px 8px;
		padding: 8px 10px;
		border: none;
		border-radius: 8px;
		background: transparent;
		color: var(--color-accent);
		font-size: 13px;
		cursor: pointer;
		text-align: left;
		transition: background 0.15s ease;
	}
	.add-provider-btn:hover { background: color-mix(in srgb, var(--color-accent) 8%, transparent); }
	.pane-list { flex: 1; overflow-y: auto; padding: 0 8px 8px; }
	.pane-item {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		width: 100%;
		padding: 9px 10px;
		border: none;
		border-radius: 8px;
		background: transparent;
		color: var(--color-fg);
		font-size: 13px;
		cursor: pointer;
		text-align: left;
		transition: background 0.15s ease;
	}
	.pane-item:hover { background: var(--color-bg-tertiary); }
	.pane-item.active { background: var(--color-bg-tertiary); font-weight: 500; }
	.pane-item-name { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
	.pane-item-kind { font-size: 11px; color: var(--color-fg-secondary); flex-shrink: 0; }
	.pane-empty { padding: 16px 10px; font-size: 13px; color: var(--color-fg-tertiary); }
	.provider-detail-pane { flex: 1; min-width: 0; overflow-y: auto; display: flex; flex-direction: column; }
	.detail-card { margin-bottom: 16px; }
	.detail-card .form-row .btn-secondary { flex-shrink: 0; }
	.detail-card .form-row .btn-primary { flex-shrink: 0; }
	.card {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-separator);
		border-radius: 12px;
		padding: 20px;
	}
	.card-head { display: flex; flex-direction: column; gap: 10px; }

	/* ── 表单 ─────────────────────────────── */
	.form-row {
		display: flex;
		gap: 8px;
	}
	.form-row input,
	.form-row select {
		flex: 1;
		padding: 9px 12px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 14px;
		outline: none;
		box-sizing: border-box;
	}
	.form-row input:focus,
	.form-row select:focus { border-color: var(--color-accent); }

	.hint {
		font-size: 13px;
		color: var(--color-fg-tertiary);
		margin: 0;
		padding: 8px 0;
		line-height: 1.6;
	}

	.section-title {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		margin: 4px 0 8px;
	}

	.divider {
		height: 0.5px;
		background: var(--color-separator);
		margin: 16px 0;
	}

	/* ── 配置列表 ─────────────────────────── */
	.config-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 0;
		gap: 8px;
	}
	.config-info {
		display: flex;
		align-items: center;
		gap: 8px;
		min-width: 0;
	}
	.config-name {
		font-size: 14px;
		color: var(--color-fg);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.config-badge {
		padding: 2px 8px;
		border-radius: 6px;
		background: var(--color-bg-tertiary);
		color: var(--color-fg-secondary);
		font-size: 12px;
		flex-shrink: 0;
	}
	.config-badge.default {
		background: color-mix(in srgb, var(--color-accent) 12%, transparent);
		color: var(--color-accent);
	}
	.config-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }
	.key-input {
		width: 200px;
		padding: 6px 10px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 13px;
		outline: none;
	}
	.key-input:focus { border-color: var(--color-accent); }

	/* ── 按钮 ─────────────────────────────── */
	.btn-primary {
		padding: 9px 16px;
		border-radius: 8px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: 14px;
		font-weight: 500;
		cursor: pointer;
		transition: background 0.15s ease;
	}
	.btn-primary:hover { background: var(--color-accent-hover, var(--color-accent)); opacity: 0.92; }
	.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }

	.btn-secondary {
		width: fit-content;
		padding: 8px 16px;
		border-radius: 8px;
		border: 1px solid var(--color-accent);
		background: transparent;
		color: var(--color-accent);
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
	}
	.btn-secondary:hover { background: color-mix(in srgb, var(--color-accent) 8%, transparent); }
	.btn-secondary:disabled { opacity: 0.5; cursor: not-allowed; }

	.btn-green {
		padding: 9px 16px;
		border-radius: 8px;
		border: none;
		background: var(--color-green);
		color: #fff;
		font-size: 14px;
		font-weight: 500;
		cursor: pointer;
		transition: opacity 0.15s ease;
	}
	.btn-green:hover { opacity: 0.92; }
	.btn-green:active { transform: scale(0.98); }
	.btn-green:disabled { opacity: 0.5; cursor: not-allowed; }

	.btn-sm {
		padding: 4px 12px;
		border-radius: 6px;
		border: 1px solid var(--color-separator);
		background: transparent;
		color: var(--color-fg-secondary);
		font-size: 13px;
		cursor: pointer;
		transition: background 0.15s ease;
	}
	.btn-sm:hover { background: var(--color-bg-tertiary); }
	.btn-sm:disabled { opacity: 0.5; cursor: not-allowed; }
	.btn-sm.danger { color: var(--color-red, var(--color-red)); border-color: var(--color-red, var(--color-red)); }

	/* ── ASR 表单 ─────────────────────────── */
	.asr-form {
		display: flex;
		flex-direction: column;
		gap: 8px;
		margin: 10px 0;
	}
	.asr-form input,
	.asr-form select {
		width: 100%;
		padding: 9px 12px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 14px;
		outline: none;
		box-sizing: border-box;
	}
	.asr-form input:focus,
	.asr-form select:focus { border-color: var(--color-accent); }
	.asr-form .btn-primary { width: fit-content; }

	/* ── Toast ─────────────────────────────── */
	.toast {
		position: fixed;
		top: 16px;
		right: 16px;
		z-index: 200;
		padding: 10px 16px;
		border-radius: 10px;
		background: var(--color-green);
		color: #fff;
		font-size: 14px;
		box-shadow: 0 4px 16px rgba(0,0,0,0.2);
	}
	.toast.error { background: var(--color-red, var(--color-red)); }

	/* ── 窄视口响应式 ─────────────────────────── */
	@media (max-width: 900px) {
		.settings-nav { width: 56px; min-width: 56px; }
		.nav-item span { display: none; }
		.nav-item { justify-content: center; padding: 10px 0; }
		.nav-group-title { display: none; }
		.nav-back { width: 32px; }
		.provider-list-pane { width: 140px; min-width: 140px; }
	}
	@media (max-width: 720px) {
		.provider-shell { flex-direction: column; }
		.provider-list-pane { width: 100%; min-width: 0; border-right: none; border-bottom: 1px solid var(--color-separator); }
		.pane-list { display: flex; gap: 4px; overflow-x: auto; }
		.pane-item { width: auto; flex-shrink: 0; }
		.settings-content { padding: 20px; }
	}
</style>
