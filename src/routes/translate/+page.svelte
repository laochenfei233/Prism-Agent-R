<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { invoke } from '$lib/api/client';
	import { translateApi, glossaryApi, ocrApi, type TranslateHistoryDto, type GlossaryTermDto } from '$lib/api';

	// 翻译
	let sourceText = $state('');
	let sourceLang = $state('auto');
	let targetLang = $state('en');
	let translatedText = $state('');
	let translating = $state(false);

	// 模型配置
	let models = $state<{ id: string; display_name: string | null; model_id: string }[]>([]);
	let translateModelId = $state('');
	let translateModelLoaded = $state(false);

	// 历史
	let history = $state<TranslateHistoryDto[]>([]);
	let historyQuery = $state('');
	let showHistory = $state(false);

	// 术语表
	let terms = $state<GlossaryTermDto[]>([]);
	let showGlossary = $state(false);
	let newTerm = $state({ source_lang: 'zh', target_lang: 'en', source_term: '', target_term: '', category: '' });

	// 内置词库一键导入
	let builtinGlossaries = $state<{ file: string; label: string; description: string }[]>([]);
	let importingFile = $state('');
	let importMessage = $state<{ ok: boolean; text: string } | null>(null);

	// OCR
	let ocrText = $state('');
	let ocrImage = $state('');
	let ocrLang = $state('');

	const LANGUAGES = [
		{ code: 'zh', name: '中文' },
		{ code: 'en', name: 'English' },
		{ code: 'ja', name: '日本語' },
		{ code: 'ko', name: '한국어' },
		{ code: 'fr', name: 'Français' },
		{ code: 'de', name: 'Deutsch' },
		{ code: 'es', name: 'Español' },
	];

	onMount(async () => {
		try {
			models = await invoke<any[]>('model_list');
			const status = await translateApi.modelStatus();
			translateModelId = status.model_id ?? '';
		} catch (e) { console.error(e); }
		translateModelLoaded = true;
	});

	async function saveTranslateModel() {
		try {
			await translateApi.modelConfig(translateModelId || undefined);
		} catch (e) { console.error(e); }
	}

	async function doTranslate() {
		if (!sourceText.trim()) return;
		translating = true;
		try {
			const result = await translateApi.translate(sourceText, targetLang, sourceLang === 'auto' ? undefined : sourceLang, translateModelId || undefined);
			translatedText = result.translated;
		} catch (e) { console.error(e); translatedText = '翻译失败：' + e; }
		translating = false;
	}

	function exchangeLangs() {
		if (sourceLang === 'auto') return;
		const tmp = sourceLang;
		sourceLang = targetLang;
		targetLang = tmp;
		const tmpText = sourceText;
		sourceText = translatedText;
		translatedText = tmpText;
	}

	async function loadHistory() {
		try {
			const res = await translateApi.history(historyQuery || undefined, 20);
			history = res.items;
		} catch (e) { console.error(e); }
	}

	async function loadGlossary() {
		try { terms = await glossaryApi.list(); } catch (e) { console.error(e); }
	}

	async function addTerm() {
		if (!newTerm.source_term || !newTerm.target_term) return;
		try {
			await glossaryApi.add(newTerm);
			newTerm = { ...newTerm, source_term: '', target_term: '', category: '' };
			await loadGlossary();
		} catch (e) { console.error(e); }
	}

	async function removeTerm(id: string) {
		try { await glossaryApi.remove(id); terms = terms.filter(t => t.id !== id); } catch (e) { console.error(e); }
	}

	async function handleOcrImage(event: Event) {
		const input = event.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;
		const reader = new FileReader();
		reader.onload = async () => {
			ocrImage = reader.result as string;
			try {
				// 传 data URL（WebView 的 file.name 不是磁盘路径，后端无法读取）
				const result = await ocrApi.recognize(undefined, reader.result as string);
				ocrText = result.text;
				ocrLang = result.lang;
			} catch (e) { console.error(e); ocrText = '识别失败'; }
		};
		reader.readAsDataURL(file);
	}

	function toggleHistory() { showHistory = !showHistory; if (showHistory) loadHistory(); }
	function toggleGlossary() { showGlossary = !showGlossary; if (showGlossary) { loadGlossary(); loadBuiltin(); } }

	// ── 内置词库一键导入 ──────────────────────────────
	async function loadBuiltin() {
		try {
			builtinGlossaries = await glossaryApi.builtinList();
		} catch (e) { console.error(e); }
	}

	async function importBuiltin(file: string) {
		if (importingFile) return;
		importingFile = file;
		importMessage = null;
		try {
			const res = await glossaryApi.importBuiltin(file);
			importMessage = { ok: true, text: `「${file}」导入完成：成功 ${res.imported} 条` };
			await loadGlossary();
		} catch (e) {
			importMessage = { ok: false, text: `导入失败：${e}` };
		}
		importingFile = '';
	}
</script>

<div class="page">
	<header class="page-header">
		<div class="header-left">
			<button class="nav-back" onclick={() => goto('/')} title="返回聊天" aria-label="返回聊天">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
			</button>
			<h1 class="page-title">翻译工具</h1>
		</div>
		<div class="header-actions">
			<button class="btn-ghost" onclick={toggleHistory} aria-pressed={showHistory}>历史</button>
			<button class="btn-ghost" onclick={toggleGlossary} aria-pressed={showGlossary}>术语表</button>
		</div>
	</header>

	<!-- 顶部工具栏（Cherry 风格：语言栏 + 翻译 + 模型） -->
	<div class="toolbar">
		<div class="lang-bar">
			<select class="lang-select" bind:value={sourceLang} aria-label="源语言">
				<option value="auto">自动检测</option>
				{#each LANGUAGES as lang}
					<option value={lang.code}>{lang.name}</option>
				{/each}
			</select>
			<button class="exchange-btn" onclick={exchangeLangs} title="交换语言" aria-label="交换语言">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m7 16-4-4 4-4"/><path d="M3 12h17"/><path d="m17 8 4 4-4 4"/><path d="M21 12H4"/></svg>
			</button>
			<select class="lang-select" bind:value={targetLang} aria-label="目标语言">
				{#each LANGUAGES as lang}
					<option value={lang.code}>{lang.name}</option>
				{/each}
			</select>
		</div>
		<div class="toolbar-right">
			<select class="model-select" id="translate-model" bind:value={translateModelId} onchange={saveTranslateModel} aria-label="翻译模型">
				<option value="">默认模型</option>
				{#each models as m}
					<option value={m.id}>{m.display_name ?? m.model_id}</option>
				{/each}
			</select>
			<button class="btn-primary" onclick={doTranslate} disabled={translating || !sourceText.trim()}>
				{translating ? '翻译中...' : '翻译'}
			</button>
		</div>
	</div>

	<!-- 双栏翻译区 -->
	<div class="translate-area">
		<div class="translate-columns">
			<div class="col">
				<div class="col-head">
					<span class="col-label">原文</span>
					<span class="char-count">{sourceText.length} 字</span>
				</div>
				<textarea placeholder="输入要翻译的文本..." bind:value={sourceText} rows="10" aria-label="源文本"></textarea>
			</div>
			<div class="col">
				<div class="col-head">
					<span class="col-label">译文</span>
					<span class="char-count">{translatedText.length} 字</span>
				</div>
				<div class="result-box">
					{#if translating}
						<span class="loading">翻译中...</span>
					{:else}
						{translatedText || '翻译结果将显示在这里'}
					{/if}
				</div>
			</div>
		</div>
	</div>

	<!-- OCR 区 -->
	<div class="section">
		<h3>OCR 识别</h3>
		<div class="ocr-area">
			<input type="file" accept="image/*" onchange={handleOcrImage} aria-label="选择图片进行 OCR 识别" />
			{#if ocrImage}
				<img src={ocrImage} alt="uploaded" class="ocr-preview" />
			{/if}
			{#if ocrText}
				<div class="content-box">{ocrText}</div>
			{/if}
		</div>
	</div>

	<!-- 历史面板 -->
	{#if showHistory}
		<div class="panel">
			<h3>翻译历史</h3>
			<input placeholder="搜索历史..." bind:value={historyQuery} onkeydown={(e) => e.key === 'Enter' && loadHistory()} class="search-input" aria-label="搜索历史" />
			<div class="history-list">
				{#each history as h}
					<div class="history-item">
						<div class="history-source">{h.source_text.slice(0, 60)}...</div>
						<div class="history-target">{h.translated.slice(0, 60)}...</div>
						<span class="lang-badge">{h.source_lang}→{h.target_lang}</span>
					</div>
				{/each}
				{#if history.length === 0}<div class="empty">暂无历史</div>{/if}
			</div>
		</div>
	{/if}

	<!-- 术语表面板 -->
	{#if showGlossary}
		<div class="panel">
			<h3>术语表</h3>
			<div class="term-form">
				<input placeholder="原文" bind:value={newTerm.source_term} aria-label="原文" />
				<input placeholder="译文" bind:value={newTerm.target_term} aria-label="译文" />
				<button class="btn-primary" onclick={addTerm}>添加</button>
			</div>

			<!-- 内置词库一键导入 -->
			<div class="builtin-section">
				<div class="builtin-title">内置词库一键导入</div>
				{#if builtinGlossaries.length === 0}
					<div class="empty">未发现内置词库（打包资源缺失）</div>
				{:else}
					<div class="builtin-list">
						{#each builtinGlossaries as g}
							<div class="builtin-item">
								<div class="builtin-info">
									<span class="builtin-label">{g.label}</span>
									<span class="builtin-desc">{g.description}</span>
								</div>
								<button
									class="btn-import"
									onclick={() => importBuiltin(g.file)}
									disabled={importingFile !== ''}
									aria-label={`导入${g.label}`}
								>
									{importingFile === g.file ? '导入中...' : '一键导入'}
								</button>
							</div>
						{/each}
					</div>
					{#if importMessage}
						<div class="import-msg" class:error={!importMessage.ok}>{importMessage.text}</div>
					{/if}
				{/if}
			</div>

			<div class="term-list">
				{#each terms as t}
					<div class="term-item">
						<span>{t.source_term} → {t.target_term}</span>
						<button class="btn-danger-sm" onclick={() => removeTerm(t.id)}>删除</button>
					</div>
				{/each}
				{#if terms.length === 0}<div class="empty">暂无术语</div>{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.page { padding: 24px 32px; max-width: 1400px; margin: 0 auto; }
	.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px; }
	.header-left { display: flex; align-items: center; gap: 10px; }
	.header-actions { display: flex; gap: 8px; }
	.nav-back {
		display: flex; align-items: center; justify-content: center;
		width: 32px; height: 32px; border: none; border-radius: 8px;
		background: var(--color-bg-secondary); color: var(--color-fg-secondary);
		cursor: pointer; transition: background 0.15s ease;
	}
	.nav-back:hover { background: var(--color-bg-tertiary); color: var(--color-fg); }
	.page-title { font-size: 22px; font-weight: 600; color: var(--color-fg); margin: 0; letter-spacing: -0.41px; }
	.btn-primary { padding: 8px 16px; border-radius: 8px; border: none; background: var(--color-accent); color: #fff; font-size: 14px; font-weight: 500; cursor: pointer; }
	.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
	.btn-ghost { padding: 8px 16px; border-radius: 8px; border: 1px solid var(--color-separator); background: transparent; color: var(--color-fg-secondary); font-size: 14px; cursor: pointer; }
	.btn-ghost[aria-pressed="true"] { background: var(--color-bg-tertiary); color: var(--color-fg); }
	.btn-danger-sm { padding: 2px 6px; border-radius: 4px; border: none; background: var(--color-red); color: #fff; font-size: 11px; cursor: pointer; }

	/* 顶部工具栏（Cherry 风格） */
	.toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; padding: 10px 14px; margin-bottom: 16px; flex-wrap: wrap; }
	.lang-bar { display: flex; align-items: center; gap: 8px; }
	.lang-select { padding: 7px 12px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 13px; outline: none; }
	.lang-select:focus { border-color: var(--color-accent); }
	.exchange-btn {
		display: flex; align-items: center; justify-content: center;
		width: 32px; height: 32px; border: none; border-radius: 8px;
		background: transparent; color: var(--color-accent); cursor: pointer;
		transition: background 0.15s ease;
	}
	.exchange-btn:hover { background: var(--color-bg-tertiary); }
	.toolbar-right { display: flex; align-items: center; gap: 8px; }
	.model-select { padding: 7px 12px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 13px; outline: none; max-width: 220px; }
	.model-select:focus { border-color: var(--color-accent); }

	/* 双栏翻译区 */
	.translate-area { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; padding: 16px; margin-bottom: 24px; }
	.translate-columns { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
	.col { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
	.col-head { display: flex; align-items: center; justify-content: space-between; }
	.col-label { font-size: 12px; font-weight: 600; color: var(--color-fg-secondary); text-transform: uppercase; letter-spacing: 0.5px; }
	.char-count { font-size: 11px; color: var(--color-fg-tertiary); }
	.col textarea { width: 100%; padding: 12px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 14px; resize: vertical; font-family: inherit; outline: none; box-sizing: border-box; min-height: 200px; }
	.col textarea:focus { border-color: var(--color-accent); }
	.result-box { padding: 12px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); min-height: 200px; font-size: 14px; color: var(--color-fg); white-space: pre-wrap; }
	.loading { color: var(--color-fg-secondary); }
	.section { margin-bottom: 24px; }
	.section h3 { font-size: 14px; color: var(--color-fg-secondary); margin: 0 0 8px; text-transform: uppercase; letter-spacing: 0.5px; }
	.ocr-area { display: flex; flex-direction: column; gap: 8px; }
	.ocr-preview { max-width: 200px; border-radius: 8px; }
	.content-box { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 10px; padding: 12px; font-size: 14px; color: var(--color-fg); white-space: pre-wrap; }
	.panel { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; padding: 16px; margin-bottom: 16px; }
	.panel h3 { font-size: 14px; color: var(--color-fg-secondary); margin: 0 0 12px; text-transform: uppercase; letter-spacing: 0.5px; }
	.search-input { width: 100%; padding: 8px 12px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 13px; margin-bottom: 12px; outline: none; }
	.history-list, .term-list { display: flex; flex-direction: column; gap: 4px; max-height: 300px; overflow-y: auto; }
	.history-item, .term-item { display: flex; align-items: center; gap: 8px; padding: 8px; border-radius: 6px; background: var(--color-bg); font-size: 13px; }
	.history-source { color: var(--color-fg-secondary); flex: 1; }
	.history-target { color: var(--color-fg); flex: 1; }
	.lang-badge { font-size: 10px; background: var(--color-accent); color: #fff; padding: 2px 4px; border-radius: 3px; }
	.term-form { display: flex; gap: 8px; margin-bottom: 12px; }
	.term-form input { flex: 1; padding: 6px 10px; border-radius: 6px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 13px; outline: none; }

	/* 内置词库一键导入 */
	.builtin-section { border: 1px dashed var(--color-separator); border-radius: 8px; padding: 10px; margin-bottom: 12px; background: var(--color-bg); }
	.builtin-title { font-size: 12px; font-weight: 600; color: var(--color-fg-secondary); margin-bottom: 8px; }
	.builtin-list { display: flex; flex-direction: column; gap: 6px; max-height: 220px; overflow-y: auto; }
	.builtin-item { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 6px 8px; border-radius: 6px; background: var(--color-bg-secondary); }
	.builtin-info { display: flex; flex-direction: column; min-width: 0; }
	.builtin-label { font-size: 13px; color: var(--color-fg); font-weight: 500; }
	.builtin-desc { font-size: 11px; color: var(--color-fg-tertiary); }
	.btn-import { flex-shrink: 0; padding: 4px 10px; border-radius: 6px; border: none; background: var(--color-accent); color: #fff; font-size: 12px; cursor: pointer; }
	.btn-import:disabled { opacity: 0.5; cursor: not-allowed; }
	.import-msg { margin-top: 8px; font-size: 12px; color: var(--color-accent); }
	.import-msg.error { color: var(--color-red); }
	.term-item span { flex: 1; font-size: 13px; color: var(--color-fg); }
	.empty { text-align: center; padding: 24px; color: var(--color-fg-secondary); font-size: 13px; }

	/* ── 窄视口响应式 ─────────────────────────── */
	@media (max-width: 720px) {
		.page { padding: 16px; }
		.translate-columns { grid-template-columns: 1fr; }
		.toolbar { flex-direction: column; align-items: stretch; }
		.toolbar-right { justify-content: space-between; }
	}
</style>
