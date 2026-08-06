<script lang="ts">
	import type { AgentContext, DirTree } from '$lib/stores/context.svelte';

	let { data }: { data: AgentContext } = $props();

	const tree = $derived(data.tree);
	let filterText = $state('');
	let expandedPaths = $state<Set<string>>(new Set());

	function toggleExpand(path: string) {
		const next = new Set(expandedPaths);
		if (next.has(path)) {
			next.delete(path);
		} else {
			next.add(path);
		}
		expandedPaths = next;
	}

	function matchesFilter(node: DirTree): boolean {
		if (!filterText) return true;
		const q = filterText.toLowerCase();
		if (node.name.toLowerCase().includes(q)) return true;
		if (node.children) return node.children.some(matchesFilter);
		return false;
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
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div class="tree-node">
						{#if child.is_dir}
							<button class="node-row" onclick={() => toggleExpand(child.path)}>
								<svg
									class="dir-icon"
									class:expanded={expandedPaths.has(child.path)}
									width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
								>
									<polyline points="9 18 15 12 9 6"/>
								</svg>
								<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="folder-icon">
									<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
								</svg>
								<span class="node-name">{child.name}</span>
							</button>
							{#if expandedPaths.has(child.path) && child.children}
								<div class="children">
									{#each child.children as grandchild}
										{#if matchesFilter(grandchild)}
											<!-- svelte-ignore a11y_no_static_element_interactions -->
											<div class="tree-node">
												{#if grandchild.is_dir}
													<button class="node-row indent" onclick={() => toggleExpand(grandchild.path)}>
														<svg
															class="dir-icon"
															class:expanded={expandedPaths.has(grandchild.path)}
															width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
														>
															<polyline points="9 18 15 12 9 6"/>
														</svg>
														<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="folder-icon">
															<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
														</svg>
														<span class="node-name">{grandchild.name}</span>
													</button>
													{#if expandedPaths.has(grandchild.path) && grandchild.children}
														<div class="children">
															{#each grandchild.children as leaf}
																{#if matchesFilter(leaf)}
																	<div class="node-row indent-2">
																		<span class="lang-badge" style:color={langColor(leaf.language)}>{langIcon(leaf.language)}</span>
																		<span class="node-name">{leaf.name}</span>
																		{#if leaf.line_count !== null}
																			<span class="line-count">{leaf.line_count}L</span>
																		{/if}
																	</div>
																{/if}
															{/each}
														</div>
													{/if}
												{:else}
													<div class="node-row indent">
														<span class="lang-badge" style:color={langColor(grandchild.language)}>{langIcon(grandchild.language)}</span>
														<span class="node-name">{grandchild.name}</span>
														{#if grandchild.line_count !== null}
															<span class="line-count">{grandchild.line_count}L</span>
														{/if}
													</div>
												{/if}
											</div>
										{/if}
									{/each}
								</div>
							{/if}
						{:else}
							<div class="node-row">
								<span class="lang-badge" style:color={langColor(child.language)}>{langIcon(child.language)}</span>
								<span class="node-name">{child.name}</span>
								{#if child.line_count !== null}
									<span class="line-count">{child.line_count}L</span>
								{/if}
							</div>
						{/if}
					</div>
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
		cursor: default;
		width: 100%;
		text-align: left;
		font-size: 12px;
		transition: background 0.1s ease;
	}
	.node-row:hover {
		background: var(--color-bg-tertiary);
	}

	.indent {
		padding-left: 20px;
	}
	.indent-2 {
		padding-left: 36px;
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

	.empty {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 32px 0;
		font-size: 13px;
		color: var(--color-fg-secondary);
	}
</style>
