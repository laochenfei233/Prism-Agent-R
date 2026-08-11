<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke, listen } from '$lib/api/client';
	import { asrApi, mcpApi, memoryApi, projectIndexApi, ragApi, settingsApi, skillApi, translateApi, workspaceApi } from '$lib/api';
	import type { SettingSpecDto } from '$lib/api';
	import Switch from '$lib/components/base/Switch.svelte';
	import SkillMarket from '$lib/components/market/SkillMarket.svelte';
	import ProviderLogo from '$lib/components/icons/ProviderLogo.svelte';

	// ── 注册表设置项（系统分组） ─────────────────────────
	let specs = $state<SettingSpecDto[]>([]);
	let savingKey = $state('');

	async function loadSpecs() {
		try { specs = await settingsApi.getAll(); } catch (e) {}
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
			msg = `${spec.label} 已保存`;
		} catch (e) {
			msg = '保存失败: ' + String(e);
		} finally {
			savingKey = '';
		}
	}

	// ── RAG / 工作区 / 项目索引 / 翻译模型（系统分组） ──
	let embedMode = $state('local');
	let embedProvider = $state('');
	let embedModel = $state('');
	let embedDim = $state(256);
	let ragContextual = $state(true);
	let ragRerank = $state(false);
	let wsPath = $state('');
	let wsCurrent = $state('');
	let wsSaving = $state(false);
	let projectIndex = $state({ enabled: true, workdir: null as string | null, indexed_files: 0, in_progress: false, last_indexed_at: null as number | null });
	let translateModelId = $state('');
	let translateLoading = $state(false);

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
			msg = '嵌入配置已保存';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function toggleContextual(v: boolean) {
		try { await ragApi.contextualConfig(v); msg = 'Contextual Retrieval 已更新'; }
		catch (e) { msg = '错误: ' + String(e); }
	}

	async function toggleRerank(v: boolean) {
		try { await ragApi.rerankConfig(v); msg = 'Reranker 已更新'; }
		catch (e) { msg = '错误: ' + String(e); }
	}

	async function loadWorkspace() {
		try {
			const info = await workspaceApi.get();
			wsCurrent = info.current_dir || '';
			wsPath = wsCurrent;
		} catch (e) {}
	}

	async function saveWorkspace() {
		if (!wsPath.trim()) { msg = '请输入工作区路径'; return; }
		wsSaving = true;
		try {
			const info = await workspaceApi.set(wsPath.trim());
			wsCurrent = info.current_dir;
			msg = '工作区已更新';
		} catch (e) { msg = '错误: ' + String(e); }
		finally { wsSaving = false; }
	}

	async function loadProjectIndex() {
		try { projectIndex = await projectIndexApi.status(); } catch (e) {}
	}

	async function toggleProjectIndex(v: boolean) {
		try { projectIndex = await projectIndexApi.toggle(v); msg = '项目索引已更新'; }
		catch (e) { msg = '错误: ' + String(e); }
	}

	async function reindexProject() {
		try { projectIndex = await projectIndexApi.reindex(); msg = '项目已重新索引'; }
		catch (e) { msg = '错误: ' + String(e); }
	}

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
			msg = '翻译模型已保存';
		} catch (e) { msg = '错误: ' + String(e); }
		finally { translateLoading = false; }
	}

	let providers = $state<any[]>([]);
	let models = $state<any[]>([]);
	let mcpServers = $state<any[]>([]);
	let skills = $state<any[]>([]);
	let msg = $state('');
	// 是否已完成首载（区分"加载中"与"暂无"）
	let loaded = $state(false);
	// 当前激活的设置分类（Cherry Studio 风格左导航）
	let section = $state<'providers' | 'asr' | 'tts' | 'agents' | 'mcp' | 'skills' | 'market' | 'memory' | 'rag' | 'security' | 'advanced'>('providers');
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
	let editBaseUrl = $state('');
	let connSaving = $state(false);
	let mProvider = $state('');
	let mModelId = $state('');
	let availableModels = $state<string[]>([]);
	let loadingModels = $state(false);

	function kindLabel(kind: string): string {
		switch (kind) {
			case 'chat': return '对话';
			case 'embedding': return '嵌入';
			case 'vision': return '视觉';
			case 'asr': return '语音';
			default: return kind;
		}
	}

	// 供应商预设库（Cherry Studio 风格：选中自动填充名称/Base URL）
	const PROVIDER_PRESETS = [
		{ kind: 'openai', name: 'OpenAI', baseUrl: 'https://api.openai.com/v1' },
		{ kind: 'anthropic', name: 'Anthropic', baseUrl: 'https://api.anthropic.com' },
		{ kind: 'google', name: 'Google Gemini', baseUrl: 'https://generativelanguage.googleapis.com/v1beta' },
		{ kind: 'deepseek', name: 'DeepSeek', baseUrl: 'https://api.deepseek.com' },
		{ kind: 'zhipu', name: '智谱', baseUrl: 'https://open.bigmodel.cn/api/paas/v4' },
		{ kind: 'moonshot', name: 'Moonshot AI', baseUrl: 'https://api.moonshot.cn' },
		{ kind: 'dashscope', name: '阿里云百炼', baseUrl: 'https://dashscope.aliyuncs.com/compatible-mode/v1' },
		{ kind: 'doubao', name: '豆包', baseUrl: 'https://ark.cn-beijing.volces.com/api/v3' },
		{ kind: 'minimax', name: 'MiniMax', baseUrl: 'https://api.minimaxi.com/v1' },
		{ kind: 'baichuan', name: '百川', baseUrl: 'https://api.baichuan-ai.com' },
		{ kind: 'silicon', name: '硅基流动', baseUrl: 'https://api.siliconflow.cn/v1' },
		{ kind: 'mimo', name: 'Xiaomi MiMo', baseUrl: 'https://api.xiaomimimo.com/v1' },
		{ kind: 'groq', name: 'Groq', baseUrl: 'https://api.groq.com/openai' },
		{ kind: 'openrouter', name: 'OpenRouter', baseUrl: 'https://openrouter.ai/api/v1' },
		{ kind: 'mistral', name: 'Mistral', baseUrl: 'https://api.mistral.ai' },
		{ kind: 'ollama', name: 'Ollama', baseUrl: 'http://localhost:11434/v1' },
		{ kind: 'custom', name: '自定义', baseUrl: '' },
	] as const;
	function applyProviderPreset(preset: { kind: string; name: string; baseUrl: string }) {
		pKind = preset.kind;
		pName = preset.name;
		pUrl = preset.baseUrl;
	}
	function providerColor(kind: string): string {
		switch (kind) {
			case 'openai': return 'var(--color-green)';
			case 'anthropic': return 'var(--color-orange)';
			case 'google': return 'var(--color-accent)';
			case 'dashscope': return 'var(--color-purple)';
			case 'mimo': return 'var(--color-red)';
			case 'ollama': return 'var(--color-fg-secondary)';
			default: return 'var(--color-fg-secondary)';
		}
	}
	function providerInitial(name: string): string {
		return (name || '?').trim()[0]?.toUpperCase() ?? '?';
	}
	// Provider 列表搜索过滤
	let providerFilter = $state('');

	// 中间栏条目 = 预置供应商（未添加显示为可配置项）+ 已添加的自定义 Provider
	type PaneProvider = {
		kind: string;
		name: string;
		existing: (typeof providers)[number] | null;
	};
	const paneProviders = $derived.by(() => {
		const rows: PaneProvider[] = PROVIDER_PRESETS
			.filter((pr) => pr.kind !== 'custom')
			.map((pr) => ({
				kind: pr.kind,
				name: pr.name,
				existing: providers.find((p) => p.kind === pr.kind) ?? null,
			}));
		// 已添加但不在预置库中的 Provider（自定义 kind）追加在末尾
		for (const p of providers) {
			if (!PROVIDER_PRESETS.some((pr) => pr.kind === p.kind)) {
				rows.push({ kind: p.kind, name: p.name, existing: p });
			}
		}
		return rows;
	});

	function selectPaneProvider(item: PaneProvider) {
		if (item.existing) {
			selectedProviderId = item.existing.id;
			addingProvider = false;
		} else {
			// 未添加 → 进入添加模式并预填充预设（name/base_url）
			const preset = PROVIDER_PRESETS.find((pr) => pr.kind === item.kind);
			if (preset) applyProviderPreset(preset);
			else { pKind = item.kind; pName = item.name; pUrl = ''; }
			pKey = '';
			selectedProviderId = null;
			addingProvider = true;
		}
	}

	// 切换 Provider 分类时自动选中第一个（已添加的）
	$effect(() => {
		if (section === 'providers') {
			if (!selectedProviderId && providers.length > 0) {
				selectedProviderId = providers[0].id;
			}
		}
	});

	// 选中 Provider 变化时自动拉取可用模型
	$effect(() => {
		const pid = selectedProviderId;
		if (pid) { mProvider = pid; fetchModels(); }
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
			const result = await invoke<{models: string[]}>('model_fetch_available', { providerId: mProvider });
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
		try { loadSpecs(); } catch (e) {}
		try { loadRagStatus(); } catch (e) {}
		try { loadWorkspace(); } catch (e) {}
		try { loadProjectIndex(); } catch (e) {}
		try { loadTranslateModel(); } catch (e) {}
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
		const savedKind = pKind;
		try {
			await invoke('settings_add_provider', {
				name: pName.trim(), kind: pKind,
				baseUrl: pUrl.trim() || null, apiKey: pKey.trim() || null
			});
			pName = ''; pUrl = ''; pKey = '';
			addingProvider = false;
			await load();
			// 选中新添加的 Provider（按 kind 匹配，找不到则回退列表首位）
			const added = providers.find((p) => p.kind === savedKind);
			selectedProviderId = added?.id ?? providers[0]?.id ?? null;
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

	async function saveProviderConn(providerId: string) {
		if (connSaving) return;
		connSaving = true;
		try {
			await invoke('settings_update_provider', { providerId, baseUrl: editBaseUrl.trim() });
			await load();
			msg = '✓ 连接已保存';
		} catch (e) { msg = '错误: ' + String(e); }
		finally { connSaving = false; }
	}

	function cancelEditConn() {
		const sel2 = providers.find((p) => p.id === selectedProviderId);
		editBaseUrl = sel2?.base_url ?? '';
	}

	// 选中 Provider 变化时同步编辑缓冲
	$effect(() => {
		if (selectedProviderId) {
			const p = providers.find((x) => x.id === selectedProviderId);
			editBaseUrl = p?.base_url ?? '';
			cancelEditKey();
		}
	});

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

	async function deleteModel(id: string) {
		if (!confirm('删除该模型？')) return;
		try {
			await invoke('model_delete', { id });
			await load();
			msg = '✓ 模型已删除';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function setDefaultModel(id: string) {
		try {
			await invoke('model_set_default', { id });
			await load();
			msg = '✓ 已设为默认';
		} catch (e) { msg = '错误: ' + String(e); }
	}

	async function quickAddModel(modelId: string) {
		const providerId = selectedProviderId;
		if (!providerId) { msg = '请先选择服务商'; return; }
		try {
			await invoke('settings_add_model', { providerId, modelId, displayName: null, isDefault: false });
			await load();
			msg = '✓ 模型已添加';
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
			<h1 class="nav-title">设置</h1>
		</div>
		<div class="nav-scroll">
			<div class="nav-group-title">模型管理</div>
			<button class="nav-item" class:active={section === 'providers'} onclick={() => section = 'providers'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2 2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/></svg>
				<span>LLM 模型管理</span>
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

			<div class="nav-group-title">系统</div>
			<button class="nav-item" class:active={section === 'rag'} onclick={() => section = 'rag'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/><path d="M8 11h6M11 8v6"/></svg>
				<span>RAG 检索</span>
			</button>
			<button class="nav-item" class:active={section === 'security'} onclick={() => section = 'security'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
				<span>安全</span>
			</button>
			<button class="nav-item" class:active={section === 'advanced'} onclick={() => section = 'advanced'}>
				<svg class="nav-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h.01a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51h.01a1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v.01a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>
				<span>高级</span>
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
					<div class="pane-search">
						<svg class="pane-search-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>
						<input bind:value={providerFilter} placeholder="搜索服务商" aria-label="搜索服务商" />
					</div>
					<div class="pane-list">
						{#each paneProviders.filter(pp => !providerFilter.trim() || pp.name.toLowerCase().includes(providerFilter.trim().toLowerCase()) || pp.kind.toLowerCase().includes(providerFilter.trim().toLowerCase())) as pp}
							<button
								class="pane-item"
								class:active={pp.existing ? selectedProviderId === pp.existing.id : false}
								onclick={() => selectPaneProvider(pp)}
							>
								<span class="pane-avatar"><ProviderLogo kind={pp.kind} /></span>
								<span class="pane-item-name">{pp.name}</span>
								<span
									class="pane-status"
									class:on={pp.existing?.is_enabled ?? false}
									title={pp.existing ? (pp.existing.is_enabled ? '可用' : '未启用') : '未配置'}
								></span>
							</button>
						{/each}
						{#if paneProviders.length === 0}
							<div class="pane-empty">{loaded ? '暂无 Provider' : '加载中...'}</div>
						{:else if paneProviders.filter(pp => !providerFilter.trim() || pp.name.toLowerCase().includes(providerFilter.trim().toLowerCase()) || pp.kind.toLowerCase().includes(providerFilter.trim().toLowerCase())).length === 0}
							<div class="pane-empty">无匹配服务商</div>
						{/if}
					</div>
					<button class="add-provider-btn" onclick={() => { pName = ''; pUrl = ''; pKey = ''; pKind = 'openai'; addingProvider = true; }}>
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
						添加服务商
					</button>
				</div>

				<!-- Provider 详情（连接 + 模型管理） -->
				<div class="provider-detail-pane">
					{#if sel && !addingProvider}
						<div class="content-header">
							<h2 class="content-title">{sel.name}</h2>
							<p class="content-desc">配置连接参数并管理该服务商的模型</p>
						</div>

						<!-- 连接设置（Cherry Studio 风格：Base URL / API Key 均可编辑） -->
						<div class="card detail-card">
							<div class="section-title">连接设置</div>
							<div class="config-row">
								<div class="config-info">
									<span class="config-name">API 地址 (Base URL)</span>
								</div>
								<div class="config-input-group">
									<input class="conn-input" bind:value={editBaseUrl} placeholder="https://api.example.com/v1" aria-label="Base URL" />
									{#if editBaseUrl !== (sel.base_url ?? '')}
										<button class="btn-sm" onclick={() => saveProviderConn(sel.id)}>保存</button>
										<button class="btn-sm" onclick={cancelEditConn}>取消</button>
									{/if}
								</div>
							</div>
							<div class="config-row">
								<div class="config-info">
									<span class="config-name">API Key</span>
								</div>
								<div class="config-input-group">
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

						<!-- 模型管理（Cherry Studio 风格：列表展示） -->
						<div class="card detail-card">
							<div class="section-title">模型</div>
							<!-- 快速添加行 -->
							<div class="form-row quick-add-row">
								<input bind:value={mModelId} placeholder="输入模型 ID（如 gpt-4o）" aria-label="模型 ID" />
								<button class="btn-primary" onclick={saveModel}>添加</button>
							</div>

							<!-- 已添加的模型列表 -->
							{#if models.filter(m => m.provider_id === sel.id).length > 0}
								<div class="model-section">
									<div class="model-section-label">已添加</div>
									<div class="model-list-box">
										{#each models.filter(m => m.provider_id === sel.id) as m}
											<div class="model-row added">
												<span class="model-name">{m.display_name || m.model_id}</span>
												<span class="config-badge kind-badge kind-{m.kind || 'chat'}">{kindLabel(m.kind || 'chat')}</span>
												{#if m.is_default}<span class="config-badge default">默认</span>{/if}
												<div class="model-actions">
													{#if !m.is_default}
														<button class="btn-sm" onclick={() => setDefaultModel(m.id)}>设默认</button>
													{/if}
													<button class="btn-sm danger" onclick={() => deleteModel(m.id)}>删除</button>
												</div>
											</div>
										{/each}
									</div>
								</div>
							{/if}

							<!-- 可用模型列表（API 拉取） -->
							{#if loadingModels}
								<div class="model-loading">拉取模型列表中...</div>
							{:else if availableModels.length > 0}
								<div class="model-section">
									<div class="model-section-label">可用模型 <span class="model-section-hint">（点击添加）</span></div>
									<div class="model-list-box">
										{#each availableModels as modelId}
											{@const isAdded = models.some(m => m.provider_id === sel.id && m.model_id === modelId)}
											<div class="model-row available" class:added={isAdded}>
												<span class="model-name">{modelId}</span>
												{#if isAdded}
													<span class="config-badge default">已添加</span>
												{:else}
													<button class="btn-sm" onclick={() => quickAddModel(modelId)}>添加</button>
												{/if}
											</div>
										{/each}
									</div>
								</div>
							{:else if models.filter(m => m.provider_id === sel.id).length === 0}
								<p class="hint">该服务商暂无模型，输入模型 ID 添加或等待自动拉取</p>
							{/if}
						</div>
					{:else}
						<!-- 添加 Provider 模式 -->
						<div class="content-header">
							<h2 class="content-title">LLM 模型管理</h2>
							<p class="content-desc">添加模型服务商并配置其模型</p>
						</div>
						<div class="card detail-card">
							<div class="section-title">添加 Provider</div>
							<div class="form-row">
								<select bind:value={pKind}>
									{#each PROVIDER_PRESETS as pr}<option value={pr.kind}>{pr.name}</option>{/each}
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
								<span class="pane-avatar" style={`background: ${providerColor('asr')}`}>{providerInitial(b.name)}</span>
								<span class="pane-item-name">{b.name}</span>
								<span class="pane-item-kind">{b.languages.join(', ')}</span>
								<span class="pane-status on" title="可用"></span>
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
				<p class="content-desc">内置 OPC Agent 自动预置，可在左侧 Agent 列表中管理</p>
			</div>
			<div class="card">
				<p class="hint">首次进入 Agent 页面时自动加载内置 OPC Agent（短视频脚本师、文案优化师、品牌定位顾问等），也可在 Agent 页面手动创建自定义 Agent。</p>
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

		{:else if section === 'rag'}
			<div class="content-header">
				<h2 class="content-title">RAG 检索</h2>
				<p class="content-desc">嵌入、检索与混合权重配置</p>
			</div>
			<div class="card">
				<div class="section-title">嵌入配置</div>
				<div class="form-row">
					<select bind:value={embedMode} aria-label="嵌入模式">
						<option value="local">本地嵌入</option>
						<option value="api">API 嵌入</option>
					</select>
				</div>
				{#if embedMode === 'api'}
					<div class="form-row">
						<input bind:value={embedProvider} placeholder="Provider ID" aria-label="Provider ID" />
					</div>
					<div class="form-row">
						<input bind:value={embedModel} placeholder="嵌入模型，如 text-embedding-3-small" aria-label="嵌入模型" />
					</div>
				{/if}
				<div class="form-row">
					<input bind:value={embedDim} type="number" placeholder="维度" aria-label="维度" />
				</div>
				<button class="btn-primary" onclick={saveEmbedding}>保存嵌入配置</button>
			</div>
			<div class="card">
				<div class="section-title">检索增强</div>
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
			{#each specsOf('rag') as spec}
				{@render SettingRow(spec, (v) => saveSpec(spec, v))}
			{/each}

		{:else if section === 'security'}
			<div class="content-header">
				<h2 class="content-title">安全</h2>
				<p class="content-desc">输入护栏与安全策略</p>
			</div>
			{#each specsOf('security') as spec}
				{@render SettingRow(spec, (v) => saveSpec(spec, v))}
			{/each}

		{:else if section === 'advanced'}
			<div class="content-header">
				<h2 class="content-title">高级</h2>
				<p class="content-desc">工作区、项目索引与高级参数</p>
			</div>
			<div class="card">
				<div class="section-title">工作区</div>
				<p class="hint">当前工作区目录（用于项目索引与上下文注入）。</p>
				<div class="form-row">
					<input bind:value={wsPath} placeholder="工作区目录路径" aria-label="工作区目录路径" />
				</div>
				<button class="btn-primary" onclick={saveWorkspace} disabled={wsSaving}>
					{wsSaving ? '保存中…' : '设置工作区'}
				</button>
				{#if wsCurrent}
					<p class="hint">当前：{wsCurrent}</p>
				{/if}
			</div>
			<div class="card">
				<div class="section-title">项目自动索引</div>
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
			{#each specsOf('advanced') as spec}
				{@render SettingRow(spec, (v) => saveSpec(spec, v))}
			{/each}
		{/if}
	</main>
</div>

<!-- 注册表设置项卡片（系统分组通用渲染） -->
{#snippet SettingRow(spec: SettingSpecDto, onsave: (v: unknown) => void)}
	<div class="card">
		{#if spec.kind === 'bool'}
			<div class="config-row">
				<div class="config-info stack">
					<span class="config-name">{spec.label}</span>
					{#if spec.description}<p class="config-desc">{spec.description}</p>{/if}
				</div>
				<Switch checked={!!spec.value} onchange={(v) => { spec.value = v; onsave(v); }} />
			</div>
		{:else}
			<div class="section-title">{spec.label}</div>
			{#if spec.description}<p class="hint">{spec.description}</p>{/if}
			{#if spec.kind === 'select'}
				<select
					class="field-input"
					value={spec.value as string}
					onchange={(e) => onsave((e.currentTarget as HTMLSelectElement).value)}
					aria-label={spec.label}
				>
					{#each spec.options || [] as opt}<option value={opt}>{opt}</option>{/each}
				</select>
			{:else if spec.kind === 'int' || spec.kind === 'float'}
				<input
					class="field-input"
					type="number"
					value={spec.value as number}
					min={spec.min ?? undefined}
					max={spec.max ?? undefined}
					step={spec.step ?? (spec.kind === 'int' ? 1 : 0.1)}
					onchange={(e) => {
						const raw = (e.currentTarget as HTMLInputElement).value;
						if (raw === '') return;
						const v = Number(raw);
						if (!Number.isNaN(v)) onsave(v);
					}}
					aria-label={spec.label}
				/>
			{:else}
				<input
					class="field-input"
					type="text"
					value={spec.value as string}
					onchange={(e) => onsave((e.currentTarget as HTMLInputElement).value)}
					aria-label={spec.label}
				/>
			{/if}
		{/if}
	</div>
{/snippet}

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
		padding: 8px;
	}
	.settings-content > :is(.content-header, .card) {
		max-width: 720px;
	}
	/* Provider 两栏铺满右侧（Cherry Studio：详情贴近窗口右缘） */
	.settings-content > .provider-shell {
		max-width: none;
		width: 100%;
	}
	.content-header { margin-bottom: var(--space-4); }
	.content-title {
		font-size: 18px;
		font-weight: 600;
		color: var(--color-fg);
		margin: 0 0 6px;
	}
	.content-desc {
		font-size: 13px;
		color: var(--color-fg-secondary);
		margin: 0;
	}

	/* ── Provider 两栏（Cherry Studio 风格） ──── */
	.provider-shell {
		display: flex;
		gap: 8px;
		height: 100%;
		min-height: 0;
	}
	.provider-list-pane {
		width: 200px;
		min-width: 200px;
		display: flex;
		flex-direction: column;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-separator);
		border-radius: 12px;
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
	.pane-search {
		display: flex;
		align-items: center;
		gap: 6px;
		margin: 0 8px 8px;
		padding: 6px 10px;
		border: 1px solid var(--color-separator);
		border-radius: 8px;
		background: var(--color-bg);
	}
	.pane-search-icon { flex-shrink: 0; color: var(--color-fg-tertiary); }
	.pane-search input {
		flex: 1;
		min-width: 0;
		border: none;
		outline: none;
		background: transparent;
		color: var(--color-fg);
		font-size: 13px;
	}
	.pane-search input::placeholder { color: var(--color-fg-tertiary); }
	.pane-list { flex: 1; overflow-y: auto; padding: 0 8px 8px; }
	.pane-item {
		display: flex;
		align-items: center;
		gap: 8px;
		width: 100%;
		padding: 8px 10px;
		border: none;
		border-radius: 8px;
		background: transparent;
		color: var(--color-fg);
		font-size: 13px;
		cursor: pointer;
		text-align: left;
		position: relative;
		transition: background 0.15s ease;
	}
	.pane-item:hover { background: var(--color-bg-tertiary); }
	.pane-item.active { background: var(--color-bg-tertiary); font-weight: 500; }
	.pane-item.active::before {
		content: '';
		position: absolute;
		left: 0;
		top: 8px;
		bottom: 8px;
		width: 3px;
		border-radius: 2px;
		background: var(--color-accent);
	}
	.pane-avatar {
		width: 24px;
		height: 24px;
		min-width: 24px;
		border-radius: 6px;
		display: inline-flex;
		align-items: center;
		justify-content: center;
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-separator);
		overflow: hidden;
	}
	.pane-avatar :global(svg) { display: block; }
	.pane-item-name { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
	.pane-item-kind { font-size: 11px; color: var(--color-fg-secondary); flex-shrink: 0; }
	.pane-status {
		width: 8px;
		height: 8px;
		min-width: 8px;
		border-radius: 50%;
		background: var(--color-fg-tertiary);
		flex-shrink: 0;
	}
	.pane-status.on { background: var(--color-green); box-shadow: 0 0 0 2px color-mix(in srgb, var(--color-green) 20%, transparent); }
	.pane-empty { padding: 16px 10px; font-size: 13px; color: var(--color-fg-tertiary); }
	.add-provider-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 6px;
		margin: 0 8px 10px;
		padding: 8px 10px;
		border: 1px dashed var(--color-separator);
		border-radius: 8px;
		background: transparent;
		color: var(--color-accent);
		font-size: 13px;
		cursor: pointer;
		text-align: center;
		transition: background 0.15s ease, border-color 0.15s ease;
	}
	.add-provider-btn:hover { background: color-mix(in srgb, var(--color-accent) 8%, transparent); border-color: var(--color-border-strong); }
	.provider-detail-pane { flex: 1; min-width: 0; overflow-y: auto; display: flex; flex-direction: column; }
	.detail-card { margin-bottom: 16px; }
	.detail-card .form-row .btn-primary { flex-shrink: 0; }
	.card {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-separator);
		border-radius: 12px;
		padding: var(--space-4);
		margin-bottom: var(--space-4);
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}
	.card:last-child { margin-bottom: 0; }
	.card-head { display: flex; flex-direction: column; gap: var(--space-3); }

	/* ── 表单 ─────────────────────────────── */
	.form-row {
		display: flex;
		gap: var(--space-2);
	}
	.form-row input,
	.form-row select {
		flex: 1;
		padding: 10px 12px;
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

	.field-input {
		width: 100%;
		padding: 10px 12px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 14px;
		outline: none;
		box-sizing: border-box;
	}
	.field-input:focus { border-color: var(--color-accent); }

	.hint {
		font-size: 13px;
		color: var(--color-fg-tertiary);
		margin: 0;
		padding: 4px 0;
		line-height: 1.6;
	}

	.section-title {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		margin: 0 0 6px;
	}

	.divider {
		height: 0.5px;
		background: var(--color-separator);
		margin: 8px 0;
	}

	/* ── 配置列表（分隔线节奏：行间留白 + 细分割线） ── */
	.config-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-3) 0;
		gap: var(--space-2);
		border-bottom: 1px solid var(--color-separator);
	}
	.config-row:last-child { border-bottom: none; }
	.config-row + .config-row { padding-top: var(--space-3); }
	.config-info {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		min-width: 0;
	}
	.config-info.stack {
		flex-direction: column;
		align-items: flex-start;
		gap: 2px;
	}
	.config-name {
		font-size: 14px;
		font-weight: 500;
		color: var(--color-fg);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.config-desc {
		font-size: 12px;
		color: var(--color-fg-secondary);
		margin: 0;
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
	.config-badge.kind-badge { font-size: 11px; padding: 2px 6px; }
	.config-badge.kind-chat {
		background: color-mix(in srgb, var(--color-green) 14%, transparent);
		color: var(--color-green);
	}
	.config-badge.kind-embedding {
		background: color-mix(in srgb, var(--color-accent) 14%, transparent);
		color: var(--color-accent);
	}
	.config-badge.kind-vision {
		background: color-mix(in srgb, var(--color-purple, var(--color-accent)) 14%, transparent);
		color: var(--color-purple, var(--color-accent));
	}
	.config-badge.kind-asr {
		background: color-mix(in srgb, var(--color-orange) 14%, transparent);
		color: var(--color-orange);
	}
	.preset-hint { font-size: 12px; color: var(--color-fg-tertiary); }

	/* ── 模型列表（Cherry Studio 风格） ──── */
	.quick-add-row { margin-bottom: 12px; }
	.quick-add-row input { flex: 1; }
	.quick-add-row .btn-primary { flex-shrink: 0; }
	.model-section { margin-top: 12px; }
	.model-section-label {
		font-size: 12px;
		font-weight: 500;
		color: var(--color-fg-secondary);
		margin: 0 0 6px;
	}
	.model-section-hint { font-weight: 400; color: var(--color-fg-tertiary); }
	.model-list-box {
		border: 1px solid var(--color-border-strong);
		border-radius: 10px;
		background: var(--color-bg);
		overflow: hidden;
	}
	.model-list-box .model-row + .model-row {
		border-top: 1px solid var(--color-separator);
	}
	.model-loading {
		padding: 12px 0;
		font-size: 13px;
		color: var(--color-fg-tertiary);
	}
	.model-row {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 8px 12px;
		border-radius: 8px;
		font-size: 13px;
		transition: background 0.12s ease;
	}
	.model-row.available { cursor: pointer; }
	.model-row.available:hover { background: var(--color-bg-hover); }
	.model-row.added { background: color-mix(in srgb, var(--color-green) 6%, transparent); }
	.model-name { flex: 1; min-width: 0; word-break: break-all; }
	.model-actions { display: flex; gap: 6px; flex-shrink: 0; }
	.config-actions { display: flex; align-items: center; gap: var(--space-2); flex-shrink: 0; }
	.config-input-group {
		display: flex;
		align-items: center;
		gap: 8px;
		flex: 1;
		min-width: 0;
	}
	.conn-input {
		flex: 1;
		min-width: 0;
		padding: 7px 10px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 13px;
		outline: none;
		transition: border-color 0.15s ease;
	}
	.conn-input:focus { border-color: var(--color-accent); }
	.conn-input::placeholder { color: var(--color-fg-tertiary); }
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
	.btn-sm:hover { background: var(--color-bg-hover); }
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
		.provider-list-pane { width: 170px; min-width: 170px; }
	}
	@media (max-width: 720px) {
		.provider-shell { flex-direction: column; gap: 8px; }
		.provider-list-pane { width: 100%; min-width: 0; border: 1px solid var(--color-separator); }
		.pane-list { display: flex; gap: 4px; overflow-x: auto; }
		.pane-item { width: auto; flex-shrink: 0; }
		.settings-content { padding: 8px; }
	}
</style>
