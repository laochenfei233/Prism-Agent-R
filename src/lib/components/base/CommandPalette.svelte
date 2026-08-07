<script module lang="ts">
	export interface CommandItem {
		id: string;
		title: string;
		shortcut?: string;
		icon?: 'plus' | 'settings' | 'chat' | 'back' | 'search';
		action: () => void;
	}
</script>

<script lang="ts">
	import { sessionApi, type SessionDto } from '$lib/api';

	let {
		open = $bindable(false),
		items = [] as CommandItem[],
		onclose,
		onOpenSession
	}: {
		open?: boolean;
		items?: CommandItem[];
		onclose?: () => void;
		onOpenSession?: (session: SessionDto) => void;
	} = $props();

	let query = $state('');
	let selected = $state(0);
	let inputEl = $state<HTMLInputElement | null>(null);
	let searchResults = $state<SessionDto[]>([]);
	let searchDone = $state(false);
	let searchSeq = 0;

	const searchItems = $derived<CommandItem[]>(
		searchResults.map((s) => ({
			id: `search-${s.id}`,
			title: `打开会话：${s.title || '新会话'}`,
			icon: 'chat',
			action: () => onOpenSession?.(s)
		}))
	);

	const filtered = $derived.by(() => {
		const q = query.trim().toLowerCase();
		if (!q) return items;
		const searchIds = new Set(searchResults.map((s) => s.id));
		const statics = items.filter((i) => {
			if (!i.title.toLowerCase().includes(q)) return false;
			const m = /^session-(.+)$/.exec(i.id);
			return !(m && searchIds.has(m[1]));
		});
		return [...searchItems, ...statics];
	});

	$effect(() => {
		const q = query.trim();
		if (q.length < 2) {
			searchResults = [];
			searchDone = false;
			return;
		}
		searchDone = false;
		const seq = ++searchSeq;
		const t = setTimeout(async () => {
			try {
				const hits = await sessionApi.search(q);
				if (seq !== searchSeq) return;
				searchResults = hits;
			} catch (e) {
				if (seq !== searchSeq) return;
				searchResults = [];
				console.error('会话搜索失败:', e);
			} finally {
				if (seq === searchSeq) {
					searchDone = true;
				}
			}
		}, 300);
		return () => clearTimeout(t);
	});

	$effect(() => {
		if (!open) return;
		query = '';
		selected = 0;
		requestAnimationFrame(() => inputEl?.focus());
	});

	$effect(() => {
		if (selected >= filtered.length) {
			selected = Math.max(0, filtered.length - 1);
		}
	});

	function close() {
		open = false;
		onclose?.();
	}

	function run(item: CommandItem | undefined) {
		if (!item) return;
		close();
		item.action();
	}

	function onKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			close();
		} else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
			e.preventDefault();
			close();
		} else if (e.key === 'ArrowDown') {
			e.preventDefault();
			const n = Math.max(1, filtered.length);
			selected = (selected + 1) % n;
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			const n = Math.max(1, filtered.length);
			selected = (selected - 1 + n) % n;
		} else if (e.key === 'Enter') {
			e.preventDefault();
			run(filtered[selected]);
		}
	}
</script>

{#if open}
	<div class="overlay" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) close(); }}>
		<div
			class="palette"
			role="dialog"
			aria-modal="true"
			aria-label="命令面板"
			tabindex="-1"
		>
			<div class="search-box">
				<svg class="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
				</svg>
				<input
					bind:this={inputEl}
					bind:value={query}
					onkeydown={onKeydown}
					placeholder="输入命令或搜索…"
					aria-label="搜索命令"
				/>
				<span class="kbd">esc</span>
			</div>

			<ul class="cmd-list">
				{#each filtered as item, i (item.id)}
					<li>
						<button
							type="button"
							class="cmd-item"
							class:selected={i === selected}
							onmouseenter={() => (selected = i)}
							onclick={() => run(item)}
						>
							<span class="cmd-icon">
								{#if item.icon === 'plus'}
									<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
										<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
									</svg>
								{:else if item.icon === 'settings'}
									<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
										<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
									</svg>
								{:else if item.icon === 'chat'}
									<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
										<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>
									</svg>
								{:else if item.icon === 'back'}
									<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
										<polyline points="15 18 9 12 15 6"/>
									</svg>
								{:else}
									<span class="cmd-bullet"></span>
								{/if}
							</span>
							<span class="cmd-title">{item.title}</span>
							{#if item.shortcut}
								<span class="cmd-shortcut">{item.shortcut}</span>
							{/if}
						</button>
					</li>
				{/each}
			</ul>

			{#if filtered.length === 0}
				<div class="empty">
					{#if query.trim().length >= 2 && searchDone}
						未找到会话
					{:else}
						无匹配命令
					{/if}
				</div>
			{/if}

			<div class="footer">
				<span><span class="kbd">↑↓</span> 选择</span>
				<span><span class="kbd">Enter</span> 执行</span>
				<span><span class="kbd">Esc</span> 关闭</span>
			</div>
		</div>
	</div>
{/if}

<style>
	.overlay {
		position: fixed;
		inset: 0;
		background: var(--color-overlay);
		display: flex;
		align-items: flex-start;
		justify-content: center;
		padding-top: 12vh;
		z-index: 1200;
		animation: fadeIn var(--duration-fast) ease;
	}

	.palette {
		width: 560px;
		max-width: calc(100vw - 32px);
		background: var(--color-bg-elevated);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-lg);
		box-shadow: var(--shadow-lg);
		overflow: hidden;
		animation: dropIn var(--duration-normal) var(--spring);
	}

	.search-box {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 14px 16px;
		border-bottom: 1px solid var(--color-separator);
	}

	.search-icon {
		color: var(--color-fg-tertiary);
		flex-shrink: 0;
	}

	.search-box input {
		flex: 1;
		min-width: 0;
		border: none;
		outline: none;
		background: transparent;
		color: var(--color-fg);
		font-size: var(--text-base);
	}
	.search-box input::placeholder {
		color: var(--color-fg-tertiary);
	}

	.cmd-list {
		list-style: none;
		margin: 0;
		padding: 6px;
		max-height: 360px;
		overflow-y: auto;
	}

	.cmd-item {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 9px 10px;
		border: none;
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--color-fg);
		font-size: var(--text-subheadline);
		text-align: left;
		cursor: pointer;
		transition: background 0.1s ease;
	}

	.cmd-item:hover {
		background: var(--color-bg-secondary);
	}

	.cmd-item.selected {
		background: var(--color-accent);
		color: #fff;
	}

	.cmd-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		flex-shrink: 0;
		color: var(--color-fg-secondary);
	}
	.cmd-item.selected .cmd-icon {
		color: rgba(255, 255, 255, 0.9);
	}

	.cmd-bullet {
		width: 6px;
		height: 6px;
		border-radius: 50%;
		background: var(--color-fg-tertiary);
	}

	.cmd-title {
		flex: 1;
		min-width: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.cmd-shortcut {
		flex-shrink: 0;
		font-size: var(--text-caption1);
		color: var(--color-fg-tertiary);
	}
	.cmd-item.selected .cmd-shortcut {
		color: rgba(255, 255, 255, 0.8);
	}

	.kbd {
		padding: 2px 6px;
		border: 1px solid var(--color-separator);
		border-radius: 5px;
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
		font-size: var(--text-caption2);
		font-family: var(--font-mono);
		white-space: nowrap;
	}

	.empty {
		padding: 20px;
		text-align: center;
		color: var(--color-fg-secondary);
		font-size: var(--text-footnote);
	}

	.footer {
		display: flex;
		gap: 16px;
		padding: 8px 16px;
		border-top: 1px solid var(--color-separator);
		color: var(--color-fg-tertiary);
		font-size: var(--text-caption1);
	}

	.footer span {
		display: inline-flex;
		align-items: center;
		gap: 6px;
	}

	@keyframes fadeIn { from { opacity: 0; } to { opacity: 1; } }
	@keyframes dropIn { from { opacity: 0; transform: translateY(-8px) scale(0.98); } to { opacity: 1; transform: translateY(0) scale(1); } }
</style>
