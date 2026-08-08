<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '$lib/api/client';
	import { translateApi, glossaryApi, ocrApi, type TranslateHistoryDto, type GlossaryTermDto } from '$lib/api';

	// 翻译
	let sourceText = $state('');
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
			const result = await translateApi.translate(sourceText, targetLang, undefined, translateModelId || undefined);
			translatedText = result.translated;
		} catch (e) { console.error(e); translatedText = '翻译失败：' + e; }
		translating = false;
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
				const result = await ocrApi.recognize(file.name);
				ocrText = result.text;
				ocrLang = result.lang;
			} catch (e) { console.error(e); ocrText = '识别失败'; }
		};
		reader.readAsDataURL(file);
	}

	function toggleHistory() { showHistory = !showHistory; if (showHistory) loadHistory(); }
	function toggleGlossary() { showGlossary = !showGlossary; if (showGlossary) loadGlossary(); }
</script>

<div class="page">
	<header class="page-header">
		<h1>翻译工具</h1>
		<div class="header-actions">
			<button class="btn-ghost" onclick={toggleHistory}>历史</button>
			<button class="btn-ghost" onclick={toggleGlossary}>术语表</button>
		</div>
	</header>

	<!-- 翻译模型配置 -->
	<div class="model-panel">
		<div class="model-row">
			<label>翻译模型</label>
			<select bind:value={translateModelId}>
				<option value="">默认模型（设置中的默认）</option>
				{#each models as m}
					<option value={m.id}>{m.display_name ?? m.model_id}</option>
				{/each}
			</select>
			<button class="btn-ghost" onclick={saveTranslateModel} disabled={!translateModelLoaded}>保存</button>
		</div>
	</div>

	<!-- 翻译区 -->
	<div class="translate-area">
		<div class="lang-bar">
			<select bind:value={targetLang}>
				{#each LANGUAGES as lang}
					<option value={lang.code}>{lang.name}</option>
				{/each}
			</select>
		</div>
		<div class="translate-columns">
			<div class="col">
				<textarea placeholder="输入要翻译的文本..." bind:value={sourceText} rows="8"></textarea>
			</div>
			<div class="col">
				<div class="result-box">
					{#if translating}
						<span class="loading">翻译中...</span>
					{:else}
						{translatedText || '翻译结果将显示在这里'}
					{/if}
				</div>
			</div>
		</div>
		<div class="translate-actions">
			<button class="btn-primary" onclick={doTranslate} disabled={translating}>
				{translating ? '翻译中...' : '翻译'}
			</button>
		</div>
	</div>

	<!-- OCR 区 -->
	<div class="section">
		<h3>OCR 识别</h3>
		<div class="ocr-area">
			<input type="file" accept="image/*" onchange={handleOcrImage} />
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
	.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px; }
	.page-header h1 { font-size: 24px; font-weight: 600; color: var(--color-fg); margin: 0; }
	.header-actions { display: flex; gap: 8px; }
	.btn-primary { padding: 8px 16px; border-radius: 8px; border: none; background: var(--color-accent); color: #fff; font-size: 14px; font-weight: 500; cursor: pointer; }
	.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
	.btn-ghost { padding: 8px 16px; border-radius: 8px; border: 1px solid var(--color-separator); background: transparent; color: var(--color-fg-secondary); font-size: 14px; cursor: pointer; }
	.btn-danger-sm { padding: 2px 6px; border-radius: 4px; border: none; background: var(--color-red); color: #fff; font-size: 11px; cursor: pointer; }
	.model-panel { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; padding: 12px 16px; margin-bottom: 16px; }
	.model-row { display: flex; align-items: center; gap: 12px; }
	.model-row label { font-size: 13px; color: var(--color-fg-secondary); flex-shrink: 0; }
	.model-row select { flex: 1; padding: 7px 10px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 13px; outline: none; }
	.model-row select:focus { border-color: var(--color-accent); }
	.translate-area { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; padding: 16px; margin-bottom: 24px; }
	.lang-bar { margin-bottom: 12px; }
	.lang-bar select { padding: 6px 12px; border-radius: 6px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 13px; }
	.translate-columns { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
	.col textarea { width: 100%; padding: 12px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 14px; resize: vertical; font-family: inherit; outline: none; }
	.col textarea:focus { border-color: var(--color-accent); }
	.result-box { padding: 12px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); min-height: 180px; font-size: 14px; color: var(--color-fg); white-space: pre-wrap; }
	.loading { color: var(--color-fg-secondary); }
	.translate-actions { display: flex; justify-content: center; margin-top: 12px; }
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
	.term-item span { flex: 1; font-size: 13px; color: var(--color-fg); }
	.empty { text-align: center; padding: 24px; color: var(--color-fg-secondary); font-size: 13px; }

	/* ── 窄视口响应式 ─────────────────────────── */
	@media (max-width: 720px) {
		.page { padding: 16px; }
		.translate-columns { grid-template-columns: 1fr; }
		.model-row { flex-direction: column; align-items: stretch; }
	}
</style>
