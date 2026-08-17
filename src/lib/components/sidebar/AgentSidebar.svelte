<script lang="ts">
	import { contextStore } from '$lib/stores/context.svelte';
	import { composeStore } from '$lib/stores/compose.svelte';
	import SidebarUsage from './SidebarUsage.svelte';
	import SidebarMcp from './SidebarMcp.svelte';
	import SidebarFiles from './SidebarFiles.svelte';
	import SidebarWorkdir from './SidebarWorkdir.svelte';
	import SidebarLsp from './SidebarLsp.svelte';
	import SidebarInstructions from './SidebarInstructions.svelte';
	import ComposePanel from '$lib/components/chat/ComposePanel.svelte';

	const tabs = [
		{ id: 'usage', label: '用量', icon: 'chart' },
		{ id: 'workdir', label: '目录', icon: 'folder' },
		{ id: 'files', label: '文件', icon: 'files' },
		{ id: 'mcp', label: 'MCP', icon: 'puzzle' },
		{ id: 'lsp', label: 'LSP', icon: 'server' },
		{ id: 'instructions', label: '指令', icon: 'instructions' },
		{ id: 'compose', label: 'Compose', icon: 'compose' }
	] as const;

	const tabComponents: Record<string, any> = {
		usage: SidebarUsage,
		workdir: SidebarWorkdir,
		files: SidebarFiles,
		mcp: SidebarMcp,
		lsp: SidebarLsp,
		instructions: SidebarInstructions,
		compose: ComposePanel
	};

	function formatNumber(n: number): string {
		if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M';
		if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K';
		return n.toString();
	}
</script>

{#if contextStore.collapsed}
	<button class="collapsed-bar" onclick={() => contextStore.toggleCollapse()} title="展开侧边栏">
		<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
			<polyline points="9 18 15 12 9 6"></polyline>
		</svg>
	</button>
{:else}
	<aside class="sidebar" style:width="{contextStore.sidebarWidth}px">
		<div class="sidebar-header">
			<span class="header-title">Agent 上下文</span>
			<button class="collapse-btn" onclick={() => contextStore.toggleCollapse()} title="折叠侧边栏">
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<polyline points="15 18 9 12 15 6"></polyline>
				</svg>
			</button>
		</div>

		<div class="tab-bar" role="tablist">
			{#each tabs as tab}
				<button
					class="tab-btn"
					class:active={contextStore.activeTab === tab.id}
					role="tab"
					aria-selected={contextStore.activeTab === tab.id}
					onclick={() => (contextStore.activeTab = tab.id)}
					title={tab.label}
				>
					{#if tab.icon === 'chart'}
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<rect x="3" y="12" width="4" height="9"/><rect x="10" y="7" width="4" height="14"/><rect x="17" y="3" width="4" height="18"/>
						</svg>
					{:else if tab.icon === 'puzzle'}
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
						</svg>
					{:else if tab.icon === 'folder'}
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
						</svg>
					{:else if tab.icon === 'files'}
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
							<polyline points="14 2 14 8 20 8"/>
						</svg>
					{:else if tab.icon === 'server'}
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<rect x="2" y="2" width="20" height="8" rx="2"/>
							<rect x="2" y="14" width="20" height="8" rx="2"/>
							<line x1="6" y1="6" x2="6.01" y2="6"/>
							<line x1="6" y1="18" x2="6.01" y2="18"/>
						</svg>
					{:else if tab.icon === 'instructions'}
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
							<polyline points="14 2 14 8 20 8"/>
							<line x1="9" y1="13" x2="15" y2="13"/>
							<line x1="9" y1="17" x2="15" y2="17"/>
						</svg>
					{:else if tab.icon === 'compose'}
						<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M12 2L2 7l10 5 10-5-10-5z"/>
							<path d="M2 17l10 5 10-5"/>
							<path d="M2 12l10 5 10-5"/>
						</svg>
					{/if}
					<span class="tab-label">{tab.label}</span>
				</button>
			{/each}
		</div>

		<div class="tab-content">
			{#if contextStore.loading}
				<div class="loading">
					<div class="spinner"></div>
				</div>
			{:else if contextStore.activeTab === 'compose'}
				{#key contextStore.activeTab}
					{#if tabComponents[contextStore.activeTab]}
						{@const SvelteComponent = tabComponents[contextStore.activeTab]}
						<SvelteComponent />
					{/if}
				{/key}
			{:else if contextStore.context}
				{#key contextStore.activeTab}
					{#if tabComponents[contextStore.activeTab]}
						{@const SvelteComponent = tabComponents[contextStore.activeTab]}
						<SvelteComponent data={contextStore.context} />
					{/if}
				{/key}
			{:else}
				<div class="empty-state">
					<span>无 Agent 上下文</span>
				</div>
			{/if}
		</div>
	</aside>
{/if}

<style>
	.collapsed-bar {
		width: 44px;
		min-width: 44px;
		background: var(--color-bg-secondary);
		border-left: 1px solid var(--color-separator);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		border: none;
		color: var(--color-fg-secondary);
		transition: background 0.15s ease, color 0.15s ease;
	}
	.collapsed-bar:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-fg);
	}

	.sidebar {
		background: var(--color-bg-secondary);
		border-left: 1px solid var(--color-separator);
		display: flex;
		flex-direction: column;
		overflow: hidden;
		min-width: 280px;
		max-width: 480px;
	}

	.sidebar-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 12px;
		border-bottom: 1px solid var(--color-separator);
		min-height: 44px;
	}

	.header-title {
		font-size: 13px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.collapse-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		transition: background 0.15s ease;
	}
	.collapse-btn:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-fg);
	}

	/* ── Tab Bar ─────────────────────────────────── */
	.tab-bar {
		display: flex;
		gap: 2px;
		padding: 6px 8px;
		border-bottom: 1px solid var(--color-separator);
		overflow-x: auto;
		scrollbar-width: none;
	}
	.tab-bar::-webkit-scrollbar { display: none; }

	.tab-btn {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 5px 8px;
		border-radius: 6px;
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		font-size: 12px;
		font-weight: 500;
		white-space: nowrap;
		transition: background 0.15s ease, color 0.15s ease;
	}
	.tab-btn:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-fg);
	}
	.tab-btn.active {
		background: var(--color-accent);
		color: #fff;
	}

	.tab-label {
		display: none;
	}
	@media (min-width: 400px) {
		.tab-label { display: inline; }
	}

	/* ── Tab Content ─────────────────────────────── */
	.tab-content {
		flex: 1;
		overflow-y: auto;
		padding: 12px;
	}

	.loading {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 40px 0;
	}

	.spinner {
		width: 24px;
		height: 24px;
		border: 2px solid var(--color-separator);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}

	.empty-state {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 40px 0;
		font-size: 13px;
		color: var(--color-fg-secondary);
	}
</style>
