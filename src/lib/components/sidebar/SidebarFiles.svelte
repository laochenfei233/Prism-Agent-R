<script lang="ts">
	import { invoke } from '$lib/api/client';
	import type { AgentContext, DirTree } from '$lib/stores/context.svelte';

	let { data }: { data: AgentContext } = $props();

	const tree = $derived(data.tree);
	let filterText = $state('');
	let expandedPaths = $state<Set<string>>(new Set());
	let treeCache = $state<Record<string, DirTree>>({});
	let loadingPaths = $state<Set<string>>(new Set());

	let previewPath = $state<string | null>(null);
	let previewContent = $state('');
	let previewLoading = $state(false);
	let previewError = $state<string | null>(null);

	let lastClickPath: string | null = null;
	let lastClickTime = 0;

	function childrenOf(path: string): DirTree[] | null {
		const cached = treeCache[path];
		return cached ? cached.children : null;
	}

	function matchesFilter(node: DirTree): boolean {
		if (!filterText) return true;
		const q = filterText.toLowerCase();
		if (node.name.toLowerCase().includes(q)) return true;
		if (node.is_dir) {
			const kids = childrenOf(node.path);
			if (kids) return kids.some(matchesFilter);
		}
		return false;
	}

	async function toggleExpand(node: DirTree) {
		const next = new Set(expandedPaths);
		if (next.has(node.path)) {
			next.delete(node.path);
			expandedPaths = next;
			return;
		}
		next.add(node.path);
		expandedPaths = next;
		if (!treeCache[node.path]) {
			await loadChildren(node.path);
		}
	}

	async function loadChildren(path: string) {
		const next = new Set(loadingPaths);
		next.add(path);
		loadingPaths = next;
		try {
			const sub = await invoke<DirTree>('workspace_tree', { path, depth: 2 });
			treeCache = { ...treeCache, [path]: sub };
		} catch (e) {
			console.error('Failed to load tree:', path, e);
		} finally {
			const done = new Set(loadingPaths);
			done.delete(path);
			loadingPaths = done;
		}
	}

	function togglePreview(path: string) {
		if (previewPath === path) {
			previewPath = null;
			return;
		}
		previewPath = path;
		void loadPreview(path);
	}

	async function loadPreview(path: string) {
		previewLoading = true;
		previewError = null;
		previewContent = '';
		try {
			previewContent = await invoke<string>('workspace_read_file', { path });
		} catch (e) {
			previewError = errMessage(e);
		} finally {
			previewLoading = false;
		}
	}

	function openExternal(path: string) {
		void invoke('workspace_open_file', { path }).catch((e) => {
			console.error('Failed to open file:', path, e);
		});
	}

	function onFileClick(path: string) {
		const now = Date.now();
		if (lastClickPath === path && now - lastClickTime < 300) {
			return; // 双击的第二次点击，交给 ondblclick 处理
		}
		lastClickPath = path;
		lastClickTime = now;
		togglePreview(path);
	}

	function errMessage(e: unknown): string {
		if (typeof e === 'string') return e;
		if (e && typeof e === 'object' && 'message' in e) return String((e as { message: unknown }).message);
		return String(e);
	}

	function langIcon(lang: string | null): string {
		if (!lang) return '?';
		const map: Record<string, string> = {
			typescript: 'TS',
			javascript: 'JS',
			rust: 'RS',
			python: 'PY',
			go: 'GO',
			css: 'CS',
			html: 'HT',
			json: 'JS',
			markdown: 'MD',
			toml: 'TM',
			yaml: 'YML',
			svelte: 'SV'
		};
		return map[lang] || lang.slice(0, 2).toUpperCase();
	}

	function langColor(lang: string | null): string {
		if (!lang) return 'var(--color-fg-secondary)';
		const map: Record<string, string> = {
			typescript: '#3178c6',
			javascript: '#f7df1e',
			rust: '#dea584',
			python: '#3572a5',
			go: '#00add8',
			svelte: '#ff3e00'
		};
		return map[lang] || 'var(--color-fg-secondary)';
	}
</script>

{#snippet nodeRow(n: DirTree, depth: number)}
	{#if n.is_dir}
		<div class="tree-node">
			<button
				class="node-row"
				class:active-dir={expandedPaths.has(n.path)}
				style:padding-left={`${8 + depth * 16}px`}
				onclick={() => void toggleExpand(n)}
			>
				<svg
					class="dir-icon"
					class:expanded={expandedPaths.has(n.path)}
					width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
				>
					<polyline points="9 18 15 12 9 6"/>
				</svg>
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="folder-icon">
					<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
				</svg>
				<span class="node-name">{n.name}</span>
			</button>
			{#if expandedPaths.has(n.path)}
				{#if loadingPaths.has(n.path)}
					<div class="hint-row" style:padding-left={`${24 + depth * 16}px`}>加载中…</div>
				{:else}
					{@const kids = childrenOf(n.path)}
					{#if kids && kids.length > 0}
						<div class="children">
							{#each kids as c}
								{#if matchesFilter(c)}
									{@render nodeRow(c, depth + 1)}
								{/if}
							{/each}
						</div>
					{:else}
						<div class="hint-row" style:padding-left={`${24 + depth * 16}px`}>空目录</div>
					{/if}
				{/if}
			{/if}
		</div>
	{:else}
		<div
			class="node-row file-row"
			class:active-file={previewPath === n.path}
			style:padding-left={`${8 + depth * 16}px`}
			role="button"
			tabindex="0"
			onclick={() => onFileClick(n.path)}
			ondblclick={() => openExternal(n.path)}
			onkeydown={(e) => {
				if (e.key === 'Enter') onFileClick(n.path);
			}}
		>
			<span class="lang-badge" style:color={langColor(n.language)}>{langIcon(n.language)}</span>
			<span class="node-name">{n.name}</span>
			{#if n.line_count !== null}
				<span class="line-count">{n.line_count}L</span>
			{/if}
		</div>
		{#if previewPath === n.path}
			<div class="preview">
				<div class="preview-header">
					<span class="preview-name">{n.name}</span>
					<span class="preview-hint">单击收起 · 双击外部打开</span>
				</div>
				{#if previewLoading}
					<div class="preview-state">加载中…</div>
				{:else if previewError}
					<div class="preview-state preview-error">{previewError}</div>
				{:else}
					<pre class="preview-content">{previewContent}</pre>
				{/if}
			</div>
		{/if}
	{/if}
{/snippet}

<div class="files-panel">
	<div class="search-bar">
		<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
			<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
		</svg>
		<input
			type="text"
			placeholder="过滤文件..."
			bind:value={filterText}
		/>
	</div>

	{#if tree}
		<div class="tree">
			{#each tree.children ?? [] as child}
				{#if matchesFilter(child)}
					{@render nodeRow(child, 0)}
				{/if}
			{/each}
		</div>
	{:else}
		<div class="empty">
			<span>无文件树</span>
		</div>
	{/if}
</div>

<style>
	.files-panel {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.search-bar {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 10px;
		background: var(--color-bg);
		border-radius: 6px;
		border: 1px solid var(--color-separator);
		color: var(--color-fg-secondary);
	}
	.search-bar:focus-within {
		border-color: var(--color-accent);
	}

	.search-bar input {
		flex: 1;
		border: none;
		background: none;
		color: var(--color-fg);
		font-size: 12px;
		outline: none;
	}
	.search-bar input::placeholder {
		color: var(--color-fg-secondary);
	}

	.tree {
		display: flex;
		flex-direction: column;
		gap: 1px;
		font-size: 12px;
	}

	.node-row {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 4px 6px;
		border-radius: 4px;
		border: none;
		background: none;
		color: var(--color-fg);
		cursor: pointer;
		width: 100%;
		text-align: left;
		font-size: 12px;
		transition: background 0.1s ease;
	}
	.node-row:hover {
		background: var(--color-bg-tertiary);
	}

	.active-dir,
	.active-file {
		background: var(--color-bg-tertiary);
	}

	.dir-icon {
		color: var(--color-fg-secondary);
		flex-shrink: 0;
		transition: transform 0.15s ease;
	}
	.dir-icon.expanded {
		transform: rotate(90deg);
	}

	.folder-icon {
		color: var(--color-accent);
		flex-shrink: 0;
	}

	.node-name {
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.lang-badge {
		font-size: 9px;
		font-weight: 700;
		font-family: var(--font-mono);
		flex-shrink: 0;
		min-width: 16px;
		text-align: center;
	}

	.line-count {
		font-size: 10px;
		color: var(--color-fg-secondary);
		font-family: var(--font-mono);
		flex-shrink: 0;
	}

	.children {
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.hint-row {
		font-size: 11px;
		color: var(--color-fg-secondary);
		padding: 3px 6px;
	}

	.preview {
		margin: 2px 8px 4px;
		padding: 8px 10px;
		background: var(--color-bg);
		border: 1px solid var(--color-separator);
		border-radius: 6px;
	}

	.preview-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		margin-bottom: 6px;
	}

	.preview-name {
		font-size: 12px;
		font-weight: 600;
		color: var(--color-fg);
	}

	.preview-hint {
		font-size: 10px;
		color: var(--color-fg-secondary);
	}

	.preview-state {
		font-size: 12px;
		color: var(--color-fg-secondary);
		padding: 4px 0;
	}

	.preview-error {
		color: var(--color-red);
		word-break: break-all;
	}

	.preview-content {
		margin: 0;
		max-height: 240px;
		overflow: auto;
		font-size: 11px;
		font-family: var(--font-mono);
		line-height: 1.5;
		color: var(--color-fg);
		white-space: pre;
		word-break: break-all;
	}

	.empty {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 32px 0;
		font-size: 13px;
		color: var(--color-fg-secondary);
	}
</style>
