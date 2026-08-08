<script lang="ts">
	import { page } from '$app/stores';
	import { goto } from '$app/navigation';
	import { themeStore } from '$lib/stores/theme.svelte';

	// Cherry Studio 风格最左侧窄导航
	const items = [
		{
			id: 'dashboard', label: '面板', path: '/',
			icon: '<rect x="3" y="3" width="7" height="9"/><rect x="14" y="3" width="7" height="5"/><rect x="14" y="12" width="7" height="9"/><rect x="3" y="16" width="7" height="5"/>'
		},
		{
			id: 'agent', label: 'Agent', path: '/agent',
			icon: '<path d="M21 11.5a8.38 8.38 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.38 8.38 0 0 1-3.8-.9L3 21l1.9-5.7a8.38 8.38 0 0 1-.9-3.8 8.5 8.5 0 0 1 4.7-7.6 8.38 8.38 0 0 1 3.8-.9h.5a8.48 8.48 0 0 1 8 8v.5z"/>'
		},
		{
			id: 'meetings', label: '会议', path: '/meetings',
			icon: '<path d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"/><path d="M19 10v2a7 7 0 0 1-14 0v-2"/><line x1="12" y1="19" x2="12" y2="23"/>'
		},
		{
			id: 'wiki', label: '知识库', path: '/wiki',
			icon: '<path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/><path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>'
		},
		{
			id: 'translate', label: '翻译', path: '/translate',
			icon: '<path d="m5 8 6 6"/><path d="m4 14 6-6 2-3"/><path d="M2 5h12"/><path d="M7 2h1"/>'
		},
	] as const;

	const bottomItems = [
		{
			id: 'settings', label: '设置', path: '/settings',
			icon: '<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>'
		},
	] as const;

	const pathname = $derived($page.url.pathname);

	function isActive(item: { path: string }): boolean {
		if (item.path === '/') return pathname === '/';
		return pathname.startsWith(item.path);
	}
</script>

<nav class="primary-nav" aria-label="主导航">
	<div class="nav-logo" title="Prism">
		<img src="/icon.svg" alt="" width="22" height="22" />
	</div>

	<div class="nav-items">
		{#each items as item}
			<button
				class="nav-btn"
				class:active={isActive(item)}
				onclick={() => goto(item.path)}
				title={item.label}
				aria-label={item.label}
				aria-current={isActive(item) ? 'page' : undefined}
			>
				<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
					{@html item.icon}
				</svg>
			</button>
		{/each}
	</div>

	<div class="nav-bottom">
		<button
			class="nav-btn"
			class:active={false}
			onclick={() => themeStore.toggle()}
			title={themeStore.theme === 'dark' ? '切换到浅色模式' : '切换到深色模式'}
			aria-label="切换主题"
		>
			{#if themeStore.theme === 'dark'}
				<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/></svg>
			{:else}
				<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>
			{/if}
		</button>
		{#each bottomItems as item}
			<button
				class="nav-btn"
				class:active={isActive(item)}
				onclick={() => goto(item.path)}
				title={item.label}
				aria-label={item.label}
				aria-current={isActive(item) ? 'page' : undefined}
			>
				<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
					{@html item.icon}
				</svg>
			</button>
		{/each}
	</div>
</nav>

<style>
	.primary-nav {
		width: 56px;
		min-width: 56px;
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		background: var(--color-bg-secondary);
		border-right: 1px solid var(--color-separator);
		padding: 10px 0;
		gap: 4px;
		overflow-y: auto;
	}
	.nav-logo {
		width: 36px;
		height: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 10px;
		margin-bottom: 8px;
	}
	.nav-items {
		display: flex;
		flex-direction: column;
		gap: 4px;
		width: 100%;
		align-items: center;
	}
	.nav-bottom {
		margin-top: auto;
		display: flex;
		flex-direction: column;
		gap: 4px;
		width: 100%;
		align-items: center;
	}
	.nav-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 40px;
		height: 40px;
		border: none;
		border-radius: 10px;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		transition: background 0.15s ease, color 0.15s ease;
	}
	.nav-btn:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-fg);
	}
	.nav-btn.active {
		background: var(--color-accent);
		color: #fff;
	}
	.nav-btn.active:hover {
		background: var(--color-accent-hover);
		color: #fff;
	}
</style>
