<script lang="ts">
	import { invoke } from '$lib/api/client';
	import SkillCard from './SkillCard.svelte';

	let query = $state('');
	let results = $state<any[]>([]);
	let loading = $state(false);
	let sourceFilter = $state('all');

	let debounceTimer: ReturnType<typeof setTimeout>;

	function onInput() {
		clearTimeout(debounceTimer);
		debounceTimer = setTimeout(() => search(), 300);
	}

	async function search() {
		if (!query.trim()) {
			results = [];
			return;
		}
		loading = true;
		try {
			results = await invoke<any[]>('skill_search_market', { query });
		} catch (e) {
			console.error('Search failed:', e);
			results = [];
		} finally {
			loading = false;
		}
	}

	let filtered = $derived(
		sourceFilter === 'all' ? results : results.filter((r) => r.source === sourceFilter)
	);
</script>

<div class="skill-market">
	<div class="market-header">
		<div class="search-wrapper">
			<svg class="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
			</svg>
			<input
				type="text"
				placeholder="Search skills..."
				bind:value={query}
				oninput={onInput}
				class="search-input"
			/>
		</div>
		<div class="source-filters">
			{#each [
				{ value: 'all', label: 'All' },
				{ value: 'skills.sh', label: 'skills.sh' },
				{ value: 'claude-plugins.dev', label: 'Claude 插件' },
				{ value: 'clawhub.ai', label: 'ClawHub' },
			] as { value, label }}
				<button
					class="filter-chip"
					class:active={sourceFilter === value}
					onclick={() => (sourceFilter = value)}
				>
					{label}
				</button>
			{/each}
		</div>
	</div>

	{#if loading}
		<div class="loading">
			<div class="spinner"></div>
			<span>Searching...</span>
		</div>
	{:else if filtered.length > 0}
		<div class="results-grid">
			{#each filtered as hit (hit.id || hit.name)}
				<SkillCard {hit} />
			{/each}
		</div>
	{:else if query && !loading}
		<div class="empty">
			<p>No results found for "{query}".</p>
		</div>
	{/if}
</div>

<style>
	.skill-market {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
	}

	.market-header {
		display: flex;
		flex-direction: column;
		gap: var(--space-3);
	}

	.search-wrapper {
		position: relative;
	}

	.search-icon {
		position: absolute;
		left: 12px;
		top: 50%;
		transform: translateY(-50%);
		color: var(--color-fg-tertiary);
		pointer-events: none;
	}

	.search-input {
		width: 100%;
		padding: 10px 12px 10px 38px;
		border-radius: var(--radius-md);
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		font-size: 15px;
		outline: none;
		box-sizing: border-box;
		transition: border-color 0.15s ease;
	}
	.search-input:focus {
		border-color: var(--color-accent);
	}

	.source-filters {
		display: flex;
		gap: var(--space-2);
		flex-wrap: wrap;
	}

	.filter-chip {
		padding: 6px 14px;
		border-radius: var(--radius-md);
		border: 1px solid var(--color-separator);
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
		font-size: 13px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.15s ease;
	}
	.filter-chip:hover {
		background: var(--color-bg-tertiary);
	}
	.filter-chip.active {
		background: var(--color-accent);
		color: #fff;
		border-color: var(--color-accent);
	}

	.loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--space-2);
		padding: var(--space-8);
		color: var(--color-fg-secondary);
		font-size: 14px;
	}

	.spinner {
		width: 20px;
		height: 20px;
		border: 2px solid var(--color-separator);
		border-top-color: var(--color-accent);
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	.results-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
		gap: var(--space-3);
	}

	.empty {
		text-align: center;
		padding: var(--space-8);
		color: var(--color-fg-tertiary);
		font-size: 14px;
	}

	.empty p {
		margin: 0;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}
</style>
