<script lang="ts">
	import type { WikiDto, WikiPageDto, WikiPageHitDto } from '$lib/api';
	import { wikiApi } from '$lib/api';
	import Input from '$lib/components/base/Input.svelte';
	import Button from '$lib/components/base/Button.svelte';
	import Textarea from '$lib/components/base/Textarea.svelte';
	import EmptyState from '$lib/components/base/EmptyState.svelte';
	import Skeleton from '$lib/components/base/Skeleton.svelte';

	let {
		wiki,
		onback
	}: {
		wiki: WikiDto;
		onback?: () => void;
	} = $props();

	let pages = $state<WikiPageDto[]>([]);
	let loading = $state(true);
	let searchQuery = $state('');
	let searchResults = $state<WikiPageHitDto[]>([]);
	let selectedPage = $state<string | null>(null);
	let pageContent = $state('');
	let saving = $state(false);
	let aiPrompt = $state('');
	let generating = $state(false);

	async function loadPages() {
		try {
			loading = true;
			pages = await wikiApi.listPages(wiki.id);
		} catch (e) {
			console.error('Failed to load pages:', e);
		} finally {
			loading = false;
		}
	}

	async function handleSearch() {
		if (!searchQuery.trim()) {
			searchResults = [];
			return;
		}
		try {
			searchResults = await wikiApi.search(wiki.id, searchQuery);
		} catch (e) {
			console.error('Search failed:', e);
		}
	}

	async function handleSelectPage(path: string) {
		try {
			selectedPage = path;
			pageContent = await wikiApi.readPage(wiki.id, path);
		} catch (e) {
			console.error('Failed to read page:', e);
			pageContent = '';
		}
	}

	async function handleSavePage() {
		if (!selectedPage) return;
		try {
			saving = true;
			await wikiApi.writePage(wiki.id, selectedPage, pageContent);
			await loadPages();
		} catch (e) {
			console.error('Failed to save page:', e);
		} finally {
			saving = false;
		}
	}

	async function handleNewPage() {
		const name = prompt('页面路径 (例如: notes/my-page):');
		if (!name) return;
		try {
			await wikiApi.writePage(wiki.id, name, '');
			await loadPages();
			await handleSelectPage(name);
		} catch (e) {
			console.error('Failed to create page:', e);
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if ((e.metaKey || e.ctrlKey) && e.key === 's') {
			e.preventDefault();
			handleSavePage();
		}
	}

	$effect(() => {
		loadPages();
	});

	$effect(() => {
		if (searchQuery) {
			const timer = setTimeout(handleSearch, 300);
			return () => clearTimeout(timer);
		} else {
			searchResults = [];
		}
	});
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="wiki-detail">
	<!-- Header -->
	<div class="detail-header">
		<button class="back-btn" onclick={onback}>
			<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
		</button>
		<div class="header-info">
			<h2>{wiki.name}</h2>
			{#if wiki.description}
				<span class="header-desc">{wiki.description}</span>
			{/if}
		</div>
		<div class="header-actions">
			<button class="btn-ghost" onclick={handleNewPage}>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
				新建页面
			</button>
		</div>
	</div>

	<div class="detail-body">
		<!-- Sidebar: pages list + search -->
		<aside class="page-sidebar">
			<div class="search-box">
				<Input bind:value={searchQuery} placeholder="搜索页面..." />
			</div>

			{#if searchResults.length > 0}
				<div class="page-section">
					<div class="section-label">搜索结果</div>
					{#each searchResults as hit}
						<button class="page-item" onclick={() => handleSelectPage(hit.path)}>
							<div class="page-title">{hit.title}</div>
							<div class="page-snippet">{hit.snippet}</div>
						</button>
					{/each}
				</div>
			{/if}

			<div class="page-section">
				<div class="section-label">所有页面</div>
				{#if loading}
					<Skeleton lines={4} />
				{:else if pages.length === 0}
					<div class="no-pages">暂无页面</div>
				{:else}
					{#each pages as page}
						<button
							class="page-item"
							class:active={selectedPage === page.path}
							onclick={() => handleSelectPage(page.path)}
						>
							<div class="page-title">{page.title}</div>
							<div class="page-meta">{page.size} bytes</div>
						</button>
					{/each}
				{/if}
			</div>
		</aside>

		<!-- Main: editor -->
		<div class="editor-area">
			{#if selectedPage}
				<div class="editor-header">
					<span class="editor-path">{selectedPage}</span>
					<Button variant="primary" size="sm" onclick={handleSavePage} disabled={saving}>
						{saving ? '保存中...' : '保存'}
					</Button>
				</div>
				<textarea class="editor" bind:value={pageContent} spellcheck="false"></textarea>

				<!-- AI Write Area -->
				<div class="ai-section">
					<div class="ai-label">AI 写入</div>
					<div class="ai-row">
						<Input bind:value={aiPrompt} placeholder="描述你想写入的内容..." />
						<Button variant="secondary" size="sm" disabled={generating || !aiPrompt.trim()}>
							{generating ? '生成中...' : '生成'}
						</Button>
					</div>
				</div>
			{:else}
				<EmptyState
					icon="📝"
					title="选择一个页面"
					description="从左侧列表中选择页面进行编辑，或创建一个新页面"
				/>
			{/if}
		</div>
	</div>
</div>

<style>
	.wiki-detail {
		display: flex;
		flex-direction: column;
		height: 100%;
		background: var(--color-bg);
	}

	.detail-header {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--color-separator);
		background: var(--color-bg);
	}

	.back-btn {
		width: 32px;
		height: 32px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg-elevated);
		color: var(--color-fg-secondary);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}
	.back-btn:hover { background: var(--color-bg-hover); color: var(--color-fg); }

	.header-info {
		flex: 1;
		min-width: 0;
	}
	.header-info h2 {
		font-size: var(--text-headline);
		font-weight: 600;
		margin: 0;
	}
	.header-desc {
		font-size: var(--text-xs);
		color: var(--color-fg-secondary);
	}

	.header-actions {
		display: flex;
		gap: var(--space-2);
	}

	.detail-body {
		display: flex;
		flex: 1;
		overflow: hidden;
	}

	/* Sidebar */
	.page-sidebar {
		width: 240px;
		min-width: 240px;
		border-right: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		display: flex;
		flex-direction: column;
		overflow-y: auto;
	}

	.search-box {
		padding: var(--space-2);
		border-bottom: 1px solid var(--color-separator);
	}

	.page-section {
		padding: var(--space-2);
	}

	.section-label {
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--color-fg-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
		padding: var(--space-1) var(--space-2);
	}

	.page-item {
		display: flex;
		flex-direction: column;
		gap: 2px;
		width: 100%;
		padding: var(--space-2);
		border-radius: 6px;
		border: none;
		background: transparent;
		cursor: pointer;
		text-align: left;
		transition: background 0.15s;
	}
	.page-item:hover { background: var(--color-bg-tertiary); }
	.page-item.active { background: var(--color-accent); color: #fff; }

	.page-title {
		font-size: var(--text-sm);
		font-weight: 500;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.page-meta {
		font-size: var(--text-xs);
		color: var(--color-fg-tertiary);
	}
	.page-item.active .page-meta { color: rgba(255,255,255,0.7); }

	.page-snippet {
		font-size: var(--text-xs);
		color: var(--color-fg-tertiary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.no-pages {
		padding: var(--space-3);
		text-align: center;
		font-size: var(--text-sm);
		color: var(--color-fg-secondary);
	}

	/* Editor */
	.editor-area {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.editor-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--color-separator);
	}

	.editor-path {
		font-size: var(--text-sm);
		font-family: var(--font-mono);
		color: var(--color-fg-secondary);
	}

	.editor {
		flex: 1;
		width: 100%;
		padding: var(--space-3);
		border: none;
		background: var(--color-bg);
		color: var(--color-fg);
		font-family: var(--font-mono);
		font-size: var(--text-sm);
		line-height: 1.6;
		resize: none;
		outline: none;
	}

	.ai-section {
		padding: var(--space-3);
		border-top: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
	}

	.ai-label {
		font-size: var(--text-xs);
		font-weight: 600;
		color: var(--color-fg-secondary);
		margin-bottom: var(--space-2);
	}

	.ai-row {
		display: flex;
		gap: var(--space-2);
	}
	.ai-row :global(.input) {
		flex: 1;
	}
</style>
