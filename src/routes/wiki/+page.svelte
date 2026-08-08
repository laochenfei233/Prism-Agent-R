<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { invoke } from '$lib/api/client';
	import { wikiApi, ragApi, type WikiDto, type WikiPageDto, type WikiPageHitDto, type EmbeddingStatusDto, type WikiWritePlan, type WikiOp, type RagDocumentDto, type RagHitDto } from '$lib/api';

	let wikis = $state<WikiDto[]>([]);
	let selectedWiki = $state<WikiDto | null>(null);
	let showCreate = $state(false);
	let newName = $state('');
	let newDesc = $state('');
	let loading = $state(false);

	// 页面
	let pages = $state<WikiPageDto[]>([]);
	let selectedPath = $state<string | null>(null);
	let pageContent = $state('');
	let editing = $state(false);
	let editContent = $state('');
	let categories = $derived(groupByCategory(pages));

	// 搜索
	let searchQuery = $state('');
	let searchResults = $state<WikiPageHitDto[]>([]);
	let searching = $state(false);
	// 底部查询栏模式
	let queryMode = $state<'ai' | 'rag' | 'docs'>('ai');

	// AI 写入（§10.1.1）
	let aiInput = $state('');
	let aiGenerating = $state(false);
	let aiPlan = $state<WikiWritePlan | null>(null);
	let aiApplying = $state(false);
	let aiResult = $state('');

	// RAG 文档
	let ragDocs = $state<RagDocumentDto[]>([]);
	let ragQuery = $state('');
	let ragHits = $state<RagHitDto[]>([]);

	// 嵌入器配置
	let embStatus = $state<EmbeddingStatusDto | null>(null);
	let embMode = $state<'local' | 'api'>('local');
	let embProvider = $state('');
	let embModel = $state('');
	let providers = $state<{ id: string; name: string; kind: string }[]>([]);
	let rerankEnabled = $state(false);

	onMount(async () => {
		loadWikis();
		await loadEmbStatus();
	});

	async function loadEmbStatus() {
		try {
			embStatus = await ragApi.embeddingStatus();
			embMode = embStatus.mode === 'api' ? 'api' : 'local';
			embProvider = embStatus.provider_id ?? '';
			embModel = embStatus.model ?? '';
			providers = await invoke<any[]>('model_providers');
			const rr = await ragApi.rerankStatus();
			rerankEnabled = rr.enabled;
		} catch (e) { console.error(e); }
	}

	async function toggleRerank() {
		try { await ragApi.rerankConfig(rerankEnabled); } catch (e) { console.error(e); }
	}

	async function saveEmbConfig() {
		try {
			embStatus = await ragApi.embeddingConfig(
				embMode,
				embMode === 'api' ? embProvider || undefined : undefined,
				embMode === 'api' ? embModel || undefined : undefined
			);
		} catch (e) { console.error(e); }
	}

	async function loadWikis() {
		loading = true;
		try { wikis = await wikiApi.list(); } catch (e) { console.error(e); }
		loading = false;
	}

	async function createWiki() {
		if (!newName.trim()) return;
		try {
			await wikiApi.create(newName.trim(), newDesc.trim() || undefined);
			newName = ''; newDesc = ''; showCreate = false;
			await loadWikis();
		} catch (e) { console.error(e); }
	}

	async function selectWiki(wiki: WikiDto) {
		selectedWiki = wiki;
		selectedPath = null;
		aiPlan = null; aiResult = ''; searchResults = []; ragHits = [];
		try {
			[pages, ragDocs] = await Promise.all([
				wikiApi.listPages(wiki.id),
				ragApi.listDocuments(wiki.id)
			]);
		} catch (e) { console.error(e); }
	}

	async function deleteWiki(id: string) {
		if (!confirm('确定删除此知识库？')) return;
		try { await wikiApi.delete(id); selectedWiki = null; await loadWikis(); } catch (e) { console.error(e); }
	}

	async function openPage(path: string) {
		if (!selectedWiki) return;
		selectedPath = path;
		editing = false;
		try { pageContent = await wikiApi.readPage(selectedWiki.id, path); } catch (e) { console.error(e); }
	}

	function startEdit() { editing = true; editContent = pageContent; }

	async function saveEdit() {
		if (!selectedWiki || !selectedPath) return;
		try {
			await wikiApi.writePage(selectedWiki.id, selectedPath, editContent);
			pageContent = editContent;
			editing = false;
			pages = await wikiApi.listPages(selectedWiki.id);
		} catch (e) { console.error(e); }
	}

	async function handleSearch() {
		if (!selectedWiki || !searchQuery.trim()) { searchResults = []; return; }
		searching = true;
		try { searchResults = await wikiApi.search(selectedWiki.id, searchQuery); } catch (e) { console.error(e); }
		searching = false;
	}

	// ── AI 写入 ─────────────────────────────────────────
	async function generateAiPlan() {
		if (!selectedWiki || !aiInput.trim()) return;
		aiGenerating = true; aiResult = '';
		try {
			const res = await wikiApi.writeAi(selectedWiki.id, aiInput, true);
			aiPlan = res.plan;
		} catch (e) { console.error(e); aiResult = '生成失败：' + e; }
		aiGenerating = false;
	}

	async function confirmAiPlan() {
		if (!selectedWiki || !aiPlan) return;
		aiApplying = true;
		try {
			const result = await wikiApi.applyPlan(selectedWiki.id, aiPlan);
			aiResult = `已执行：${result.summary}（log.md ${result.log_appended ? '已更新' : '未更新'}）`;
			aiPlan = null; aiInput = '';
			pages = await wikiApi.listPages(selectedWiki.id);
		} catch (e) { console.error(e); aiResult = '执行失败：' + e; }
		aiApplying = false;
	}

	// ── RAG ─────────────────────────────────────────────
	async function ragSearch() {
		if (!selectedWiki || !ragQuery.trim()) { ragHits = []; return; }
		try { ragHits = await ragApi.search(selectedWiki.id, ragQuery, 5); } catch (e) { console.error(e); }
	}

	async function ingestDocument() {
		if (!selectedWiki) return;
		try {
			const path = await invoke<string>('file_pick');
			if (!path) return;
			const res = await ragApi.ingest(selectedWiki.id, path);
			ragDocs = await ragApi.listDocuments(selectedWiki.id);
			alert(`已导入：${res.chunk_count} 个分块（${res.status}）`);
		} catch (e) { console.error(e); alert('导入失败：' + e); }
	}

	async function deleteDocument(docId: string) {
		if (!confirm('删除此文档及其分块？')) return;
		try {
			await ragApi.deleteDocument(docId);
			ragDocs = await ragApi.listDocuments(selectedWiki!.id);
		} catch (e) { console.error(e); }
	}

	function opLabel(op: WikiOp): string {
		switch (op.op) {
			case 'create_page': return `新建 ${op.path}`;
			case 'update_page': return `更新 ${op.path}`;
			case 'delete_page': return `删除 ${op.path}`;
			case 'update_index': return `追加索引 ${(op.entries ?? []).length} 条`;
			case 'noop': return `跳过：${op.reason ?? ''}`;
			default: return op.op;
		}
	}

	function groupByCategory(pages: WikiPageDto[]): Record<string, WikiPageDto[]> {
		const out: Record<string, WikiPageDto[]> = {};
		for (const p of pages) {
			const cat = p.path.includes('/') ? p.path.split('/')[0] : '根目录';
			(out[cat] ??= []).push(p);
		}
		return out;
	}
</script>

<div class="page">
	<header class="page-header">
		<div class="header-left">
			<button class="nav-back" onclick={() => goto('/')} title="返回聊天" aria-label="返回聊天">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
			</button>
			<h1 class="page-title">知识库</h1>
		</div>
		<div class="header-right">
			{#if selectedWiki}
				<button class="btn-ghost" onclick={() => { selectedWiki = null; }}>← 返回列表</button>
			{:else}
				<button class="btn-primary" onclick={() => showCreate = true}>新建知识库</button>
			{/if}
		</div>
	</header>

	{#if showCreate}
		<div class="create-form">
			<input placeholder="知识库名称" bind:value={newName} aria-label="知识库名称" />
			<input placeholder="描述（可选）" bind:value={newDesc} aria-label="描述可选" />
			<div class="form-actions">
				<button class="btn-ghost" onclick={() => showCreate = false}>取消</button>
				<button class="btn-primary" onclick={createWiki}>创建</button>
			</div>
		</div>
	{/if}

	{#if !selectedWiki}
		<!-- 知识库列表 -->
		{#if loading}
			<div class="empty">加载中...</div>
		{:else if wikis.length === 0}
			<div class="empty">
				<p>暂无知识库</p>
				<button class="btn-primary" onclick={() => showCreate = true}>创建第一个</button>
			</div>
		{:else}
			<div class="grid">
				{#each wikis as wiki}
					<div
						class="card"
						onclick={() => selectWiki(wiki)}
						role="button"
						tabindex="0"
						onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selectWiki(wiki); } }}
					>
						<div class="card-head">
							<div class="card-icon">
								<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
							</div>
							<button
								class="card-delete"
								onclick={(e) => { e.stopPropagation(); deleteWiki(wiki.id); }}
								title="删除知识库"
								aria-label="删除知识库"
							>
								<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
							</button>
						</div>
						<h3>{wiki.name}</h3>
						{#if wiki.description}<p>{wiki.description}</p>{/if}
					</div>
				{/each}
			</div>
		{/if}

		<!-- RAG 嵌入器配置（Cherry Studio 卡片风格） -->
		<div class="emb-card">
			<div class="emb-card-head">
				<div>
					<h2 class="emb-title">RAG 嵌入器</h2>
					<p class="emb-desc">配置知识库检索的向量化模式</p>
				</div>
				{#if embStatus}
					<span class="emb-status">
						{embStatus.is_local ? '本地' : embStatus.model || 'API'} · {embStatus.dim} 维
					</span>
				{/if}
			</div>
			<div class="emb-body">
				<div class="emb-row">
					<label for="emb-mode">模式</label>
					<select id="emb-mode" bind:value={embMode}>
						<option value="local">本地（离线特征哈希，无网络）</option>
						<option value="api">API（OpenAI 兼容 /embeddings）</option>
					</select>
				</div>
				{#if embMode === 'api'}
					<div class="emb-row">
						<label for="emb-provider">Provider</label>
						<select id="emb-provider" bind:value={embProvider}>
							<option value="">选择 Provider</option>
							{#each providers as p}
								<option value={p.id}>{p.name} ({p.kind})</option>
							{/each}
						</select>
					</div>
					<div class="emb-row">
						<label for="emb-model">模型</label>
						<input id="emb-model" placeholder="如 text-embedding-3-small / nomic-embed-text" bind:value={embModel} aria-label="嵌入模型" />
					</div>
				{/if}
				<div class="emb-row">
					<label for="emb-rerank">重排序</label>
					<label class="switch-label" for="emb-rerank">
						<input id="emb-rerank" type="checkbox" bind:checked={rerankEnabled} onchange={toggleRerank} />
						<span>LLM 重排序（初检 top-150 → 重排 → top-k，有成本）</span>
					</label>
				</div>
				<div class="emb-actions">
					<button class="btn-primary" onclick={saveEmbConfig}>保存配置</button>
				</div>
			</div>
		</div>
	{:else}
		<!-- 知识库详情：左栏切换器+页面树，右栏内容（Cherry Studio 风格） -->
		<div class="detail">
			<!-- 左栏：知识库切换 + 页面树 -->
			<aside class="tree-pane">
				<!-- 知识库切换器 -->
				<div class="wiki-switcher">
					<div class="switcher-info">
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/></svg>
						<span class="switcher-name">{selectedWiki!.name}</span>
					</div>
					<button class="switcher-back" onclick={() => { selectedWiki = null; }} title="返回知识库列表" aria-label="返回知识库列表">
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
					</button>
				</div>

				<div class="pane-header">
					<h3>页面</h3>
					<button class="icon-btn-sm" title="刷新" aria-label="刷新页面列表" onclick={() => selectWiki(selectedWiki!)}>
						<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 1 1-2.64-6.36"/><polyline points="21 3 21 9 15 9"/></svg>
					</button>
				</div>

				<div class="search-bar">
					<input placeholder="搜索页面..." bind:value={searchQuery} onkeydown={(e) => e.key === 'Enter' && handleSearch()} aria-label="搜索页面" />
				</div>
				{#if searching}<div class="hint">搜索中...</div>{/if}
				{#if searchResults.length > 0}
					<div class="search-results">
						{#each searchResults as hit}
							<div
								class="result-item"
								onclick={() => openPage(hit.path)}
								role="button"
								tabindex="0"
								onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openPage(hit.path); } }}
							>
								<strong>{hit.title}</strong>
								<span class="snippet">{hit.snippet}</span>
							</div>
						{/each}
					</div>
				{:else if searchQuery.trim() && !searching}
					<div class="hint">无结果</div>
				{/if}

				<div class="tree">
					{#each Object.entries(categories) as [cat, items]}
						{#if items.length > 0}
							<div class="tree-cat">{cat || '根目录'}</div>
							{#each items as page}
								<div
									class="tree-item"
									class:active={selectedPath === page.path}
									onclick={() => openPage(page.path)}
									role="button"
									tabindex="0"
									onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openPage(page.path); } }}
								>{page.title}</div>
							{/each}
						{/if}
					{/each}
					{#if pages.length === 0}
						<div class="hint">暂无页面，可用下方 AI 写入或直接导入</div>
					{/if}
				</div>
			</aside>

			<!-- 右侧：内容区 -->
			<main class="content-pane">
				{#if selectedPath}
					<div class="editor-header">
						<span class="editor-path">{selectedPath}</span>
						{#if editing}
							<button class="btn-primary btn-sm" onclick={saveEdit}>保存</button>
							<button class="btn-ghost btn-sm" onclick={() => editing = false}>取消</button>
						{:else}
							<button class="btn-ghost btn-sm" onclick={startEdit}>编辑</button>
						{/if}
					</div>
					{#if editing}
						<textarea class="editor" bind:value={editContent} rows="20" aria-label="编辑页面内容"></textarea>
					{:else}
						<pre class="markdown-view">{pageContent}</pre>
					{/if}
				{:else}
					<div class="empty">← 选择左侧页面，或使用 AI 写入 / RAG 检索</div>
				{/if}
			</main>

			<!-- 底部查询栏（Cherry WikiQueryBar 风格）：AI 写入 + RAG 检索 -->
			<div class="query-bar">
				<div class="query-tabs" role="tablist" tabindex="0" aria-label="知识库工具" onkeydown={(e) => {
					const keys = ['ai', 'rag', 'docs'] as const;
					const idx = keys.indexOf(queryMode);
					const dir = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
					if (dir) { e.preventDefault(); queryMode = keys[(idx + dir + 3) % 3]; }
				}}>
					<button class="query-tab" class:active={queryMode === 'ai'} onclick={() => queryMode = 'ai'} role="tab" aria-selected={queryMode === 'ai'} tabindex={queryMode === 'ai' ? 0 : -1}>AI 写入</button>
					<button class="query-tab" class:active={queryMode === 'rag'} onclick={() => queryMode = 'rag'} role="tab" aria-selected={queryMode === 'rag'} tabindex={queryMode === 'rag' ? 0 : -1}>RAG 检索</button>
					<button class="query-tab" class:active={queryMode === 'docs'} onclick={() => queryMode = 'docs'} role="tab" aria-selected={queryMode === 'docs'} tabindex={queryMode === 'docs' ? 0 : -1}>文档管理</button>
				</div>

				{#if queryMode === 'ai'}
					<div class="query-body">
						<textarea
							placeholder="输入新知识或粘贴文档片段，如：Kubernetes 1.30 引入了 ..."
							bind:value={aiInput}
							rows="3"
							aria-label="AI 写入内容"
						></textarea>
						<div class="query-actions">
							<button class="btn-primary btn-sm" onclick={generateAiPlan} disabled={aiGenerating || !aiInput.trim()}>
								{aiGenerating ? '生成计划中...' : '让 AI 入库'}
							</button>
							{#if aiResult}<span class="ai-result">{aiResult}</span>{/if}
						</div>
						{#if aiPlan}
							<div class="plan-preview">
								<h4>操作计划（确认后执行）</h4>
								{#each aiPlan.operations as op}
									<div class="plan-op" class:noop={op.op === 'noop'}>
										<span class="op-badge">{op.op === 'noop' ? '⚠' : '✓'}</span>
										{opLabel(op)}
									</div>
								{/each}
								<div class="plan-actions">
									<button class="btn-primary btn-sm" onclick={confirmAiPlan} disabled={aiApplying}>
										{aiApplying ? '执行中...' : '确认执行'}
									</button>
									<button class="btn-ghost btn-sm" onclick={() => aiPlan = null}>取消</button>
								</div>
							</div>
						{/if}
					</div>

				{:else if queryMode === 'rag'}
					<div class="query-body">
						<div class="rag-row">
							<input placeholder="问知识库..." bind:value={ragQuery} onkeydown={(e) => e.key === 'Enter' && ragSearch()} aria-label="问知识库" />
							<button class="btn-primary btn-sm" onclick={ragSearch}>检索</button>
						</div>
						{#if ragHits.length > 0}
							<div class="rag-hits">
								{#each ragHits as hit}
									<div class="rag-hit">
										<div class="rag-hit-meta">
											<strong>{hit.document_title}</strong>
											{#if hit.section}<span>· {hit.section}</span>{/if}
											{#if hit.page_start}<span>· 第 {hit.page_start} 页</span>{/if}
											<span class="score">{(hit.score * 100).toFixed(0)}</span>
										</div>
										<div class="rag-quote">"{hit.quote.slice(0, 200)}..."</div>
									</div>
								{/each}
							</div>
						{/if}
					</div>

				{:else}
					<div class="query-body">
						<div class="doc-toolbar">
							<button class="btn-ghost btn-sm" onclick={ingestDocument}>导入文档</button>
						</div>
						<div class="doc-list">
							{#each ragDocs as doc}
								<div class="doc-item">
									<span class="doc-name">{doc.name}</span>
									<span class="doc-status" class:ready={doc.status === 'ready'}>{doc.status}</span>
									<span class="doc-count">{doc.chunk_count} 块</span>
									<button class="btn-danger-sm" onclick={() => deleteDocument(doc.id)}>删除</button>
								</div>
							{/each}
							{#if ragDocs.length === 0}<div class="hint">暂无文档</div>{/if}
						</div>
					</div>
				{/if}
			</div>
		</div>
	{/if}
</div>

<style>
	.page { padding: 24px 32px; max-width: 1400px; margin: 0 auto; }
	.page-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 24px; }
	.header-left { display: flex; align-items: center; gap: 10px; }
	.header-right { display: flex; align-items: center; gap: 8px; }
	.nav-back {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border: none;
		border-radius: 8px;
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
		cursor: pointer;
		transition: background 0.15s ease, color 0.15s ease;
	}
	.nav-back:hover { background: var(--color-bg-tertiary); color: var(--color-fg); }
	.page-title { font-size: 22px; font-weight: 600; color: var(--color-fg); margin: 0; letter-spacing: -0.41px; }
	.btn-primary { padding: 8px 16px; border-radius: 8px; border: none; background: var(--color-accent); color: #fff; font-size: 14px; font-weight: 500; cursor: pointer; }
	.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
	.btn-sm { padding: 5px 12px; font-size: 13px; }
	.btn-ghost { padding: 8px 16px; border-radius: 8px; border: 1px solid var(--color-separator); background: transparent; color: var(--color-fg-secondary); font-size: 14px; cursor: pointer; }
	.btn-danger-sm { padding: 4px 8px; border-radius: 6px; border: none; background: var(--color-red); color: #fff; font-size: 12px; cursor: pointer; }
	.icon-btn-sm { width: 26px; height: 26px; border-radius: 6px; border: none; background: transparent; color: var(--color-fg-secondary); cursor: pointer; }
	.icon-btn-sm:hover { background: var(--color-bg-tertiary); }
	.create-form { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; padding: 16px; margin-bottom: 24px; display: flex; flex-direction: column; gap: 8px; }
	.create-form input { padding: 8px 12px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 14px; outline: none; }
	.create-form input:focus { border-color: var(--color-accent); }
	.form-actions { display: flex; gap: 8px; justify-content: flex-end; }

	/* 列表 */
	.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 14px; }
	.card { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; padding: 16px; cursor: pointer; transition: border-color 0.15s, transform 0.15s; position: relative; }
	.card:hover { border-color: var(--color-accent); }
	.card-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 10px; }
	.card-icon {
		width: 32px;
		height: 32px;
		border-radius: 8px;
		display: flex;
		align-items: center;
		justify-content: center;
		background: color-mix(in srgb, var(--color-accent) 10%, transparent);
		color: var(--color-accent);
	}
	.card h3 { margin: 0 0 6px; font-size: 15px; color: var(--color-fg); }
	.card p { margin: 0; font-size: 13px; color: var(--color-fg-secondary); line-height: 1.5; }
	.card-delete {
		position: absolute;
		top: 10px;
		right: 10px;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		opacity: 0;
		transition: opacity 0.15s ease, background 0.15s ease;
	}
	.card:hover .card-delete,
	.card-delete:focus-visible { opacity: 1; }
	.card-delete:hover { background: var(--color-red); color: #fff; }
	.empty { text-align: center; padding: 48px; color: var(--color-fg-secondary); }

	/* RAG 嵌入器卡片 */
	.emb-card { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; overflow: hidden; margin-top: 20px; }
	.emb-card-head { display: flex; align-items: center; justify-content: space-between; padding: 16px 20px; border-bottom: 1px solid var(--color-separator); }
	.emb-title { margin: 0; font-size: 16px; font-weight: 600; color: var(--color-fg); }
	.emb-desc { margin: 2px 0 0; font-size: 12px; color: var(--color-fg-secondary); }
	.emb-status { font-size: 12px; color: var(--color-accent); }
	.emb-body { padding: 16px 20px; display: flex; flex-direction: column; gap: 12px; }
	.emb-row { display: flex; align-items: center; gap: 12px; }
	.emb-row label { width: 80px; font-size: 13px; color: var(--color-fg-secondary); flex-shrink: 0; }
	.emb-row select, .emb-row input { flex: 1; padding: 7px 10px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 13px; outline: none; }
	.emb-row select:focus, .emb-row input:focus { border-color: var(--color-accent); }
	.emb-actions { display: flex; align-items: center; gap: 12px; margin-top: 4px; }
	.emb-actions .btn-primary { width: fit-content; }

	/* 详情双栏（Cherry 风格：左栏切换器+树 + 右栏内容 + 底部查询栏） */
	.detail { display: flex; flex-direction: column; gap: 16px; min-height: 0; }
	.tree-pane { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; padding: 12px; max-height: 280px; overflow-y: auto; }
	.wiki-switcher { display: flex; align-items: center; justify-content: space-between; padding: 2px 4px 10px; margin-bottom: 10px; border-bottom: 1px solid var(--color-separator); }
	.switcher-info { display: flex; align-items: center; gap: 8px; min-width: 0; }
	.switcher-info svg { color: var(--color-accent); flex-shrink: 0; }
	.switcher-name { font-size: 14px; font-weight: 600; color: var(--color-fg); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
	.switcher-back {
		display: flex; align-items: center; justify-content: center;
		width: 26px; height: 26px; border: none; border-radius: 6px;
		background: transparent; color: var(--color-fg-secondary); cursor: pointer;
	}
	.switcher-back:hover { background: var(--color-bg-tertiary); color: var(--color-fg); }
	.pane-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
	.pane-header h3 { margin: 0; font-size: 13px; color: var(--color-fg-secondary); text-transform: uppercase; letter-spacing: 0.5px; }
	.search-bar { display: flex; gap: 8px; margin-bottom: 10px; }
	.search-bar input { flex: 1; padding: 7px 10px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 13px; outline: none; }
	.search-bar input:focus { border-color: var(--color-accent); }
	.search-results { margin-bottom: 10px; }
	.result-item { padding: 6px 8px; border: 1px solid var(--color-separator); border-radius: 6px; margin-bottom: 4px; cursor: pointer; }
	.result-item:hover { border-color: var(--color-accent); }
	.result-item strong { font-size: 13px; color: var(--color-fg); }
	.snippet { display: block; font-size: 11px; color: var(--color-fg-secondary); margin-top: 2px; }
	.tree-cat { font-size: 11px; color: var(--color-fg-secondary); margin: 10px 0 4px; font-weight: 600; }
	.tree-item { padding: 6px 10px; border-radius: 6px; font-size: 13px; color: var(--color-fg); cursor: pointer; }
	.tree-item:hover { background: var(--color-bg-tertiary); }
	.tree-item.active { background: var(--color-accent); color: #fff; }
	.hint { font-size: 12px; color: var(--color-fg-secondary); padding: 4px 0; }

	/* 内容区 */
	.content-pane { min-width: 0; display: flex; flex-direction: column; gap: 16px; }
	.editor-header { display: flex; align-items: center; gap: 8px; }
	.editor-path { font-size: 13px; color: var(--color-fg-secondary); flex: 1; font-family: monospace; }
	.editor, .markdown-view { width: 100%; border-radius: 10px; border: 1px solid var(--color-separator); background: var(--color-bg-secondary); color: var(--color-fg); font-size: 14px; }
	.editor { padding: 12px; resize: vertical; font-family: monospace; outline: none; }
	.editor:focus { border-color: var(--color-accent); }
	.markdown-view { padding: 16px; white-space: pre-wrap; line-height: 1.6; min-height: 120px; overflow-x: auto; }

	/* 底部查询栏（Cherry WikiQueryBar 风格） */
	.query-bar { background: var(--color-bg-secondary); border: 1px solid var(--color-separator); border-radius: 12px; overflow: hidden; }
	.query-tabs { display: flex; gap: 2px; padding: 6px 10px 0; }
	.query-tab { padding: 6px 12px; border: none; border-radius: 8px 8px 0 0; background: transparent; color: var(--color-fg-secondary); font-size: 13px; cursor: pointer; position: relative; }
	.query-tab:hover { background: var(--color-bg-tertiary); }
	.query-tab.active { color: var(--color-fg); font-weight: 500; background: var(--color-bg); }
	.query-tab.active::after { content: ''; position: absolute; inset-inline: 0; bottom: 0; height: 2px; background: var(--color-accent); }
	.query-body { padding: 12px; display: flex; flex-direction: column; gap: 10px; }
	.query-body textarea { width: 100%; padding: 10px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 13px; resize: vertical; outline: none; font-family: inherit; box-sizing: border-box; }
	.query-body textarea:focus { border-color: var(--color-accent); }
	.query-actions { display: flex; align-items: center; gap: 12px; }
	.doc-toolbar { display: flex; align-items: center; justify-content: space-between; }
	.plan-preview { margin-top: 4px; border-top: 1px solid var(--color-separator); padding-top: 10px; }
	.plan-preview h4 { margin: 0 0 8px; font-size: 13px; color: var(--color-fg); }
	.plan-op { display: flex; align-items: center; gap: 8px; padding: 5px 0; font-size: 13px; color: var(--color-fg); }
	.plan-op.noop { color: var(--color-fg-secondary); }
	.op-badge { width: 18px; height: 18px; border-radius: 50%; background: var(--color-accent); color: #fff; display: flex; align-items: center; justify-content: center; font-size: 11px; flex-shrink: 0; }
	.plan-op.noop .op-badge { background: var(--color-warning, #e6a23c); }
	.plan-actions { display: flex; gap: 8px; margin-top: 8px; }
	.ai-result { font-size: 13px; color: var(--color-accent); }

	/* RAG */
	.rag-row { display: flex; gap: 8px; margin-bottom: 10px; }
	.rag-row input { flex: 1; padding: 7px 10px; border-radius: 8px; border: 1px solid var(--color-separator); background: var(--color-bg); color: var(--color-fg); font-size: 13px; outline: none; }
	.rag-row input:focus { border-color: var(--color-accent); }
	.rag-hits { display: flex; flex-direction: column; gap: 6px; }
	.rag-hit { border: 1px solid var(--color-separator); border-radius: 8px; padding: 8px 10px; }
	.rag-hit-meta { display: flex; gap: 8px; font-size: 12px; color: var(--color-fg-secondary); }
	.rag-hit-meta strong { color: var(--color-fg); }
	.score { margin-left: auto; font-size: 11px; color: var(--color-accent); }
	.rag-quote { font-size: 12px; color: var(--color-fg-secondary); margin-top: 4px; }
	.doc-list { display: flex; flex-direction: column; gap: 4px; margin-top: 4px; }
	.doc-item { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 6px; background: var(--color-bg); font-size: 12px; }
	.doc-name { flex: 1; color: var(--color-fg); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
	.doc-status { font-size: 10px; padding: 2px 6px; border-radius: 4px; background: var(--color-separator); color: var(--color-fg-secondary); }
	.doc-status.ready { background: var(--color-accent); color: #fff; }
	.doc-count { font-size: 11px; color: var(--color-fg-secondary); }

	@media (max-width: 900px) {
		.detail { flex-direction: column; }
		.tree-pane { max-height: 220px; }
	}
</style>
