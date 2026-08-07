<script lang="ts">
	import { invoke } from '$lib/api/client';

	let { hit }: { hit: any } = $props();

	let installing = $state(false);
	let installed = $state(false);
	let error = $state<string | null>(null);

	const sourceColors: Record<string, string> = {
		'skills.sh': '#7C3AED',
		'claude-plugins.dev': '#D97706',
		'clawhub.ai': '#059669',
		local: '#6B7280',
	};

	function getSourceColor(source: string): string {
		return sourceColors[source] || '#6B7280';
	}

	async function install() {
		installing = true;
		error = null;
		try {
			await invoke('skill_install_market', { source: hit.install_source });
			installed = true;
		} catch (e) {
			console.error('Install failed:', e);
			error = e instanceof Error ? e.message : String(e);
		} finally {
			installing = false;
		}
	}
</script>

<div class="skill-card">
	<div class="card-header">
		<span class="source-badge" style="background: {getSourceColor(hit.source)}20; color: {getSourceColor(hit.source)}">
			{hit.source}
		</span>
		{#if hit.stars != null}
			<span class="stars">&#9733; {hit.stars}</span>
		{/if}
	</div>

	<h4 class="skill-name">{hit.name || hit.skill_name}</h4>
	<p class="skill-desc">{hit.description || 'No description available.'}</p>

	{#if hit.tags?.length}
		<div class="tags">
			{#each hit.tags.slice(0, 5) as tag}
				<span class="tag">{tag}</span>
			{/each}
		</div>
	{/if}

	<div class="card-actions">
		{#if hit.installed || installed}
			<span class="installed-label">&#10003; Installed</span>
		{:else}
			<button class="install-btn" onclick={install} disabled={installing}>
				{installing ? 'Installing...' : 'Install'}
			</button>
		{/if}
		{#if error}
			<p class="install-error">{error}</p>
		{/if}
	</div>
</div>

<style>
	.skill-card {
		background: var(--color-bg);
		border: 1px solid var(--color-separator);
		border-radius: var(--radius-lg);
		padding: var(--space-4);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
		transition: border-color 0.15s ease, box-shadow 0.15s ease;
	}

	.skill-card:hover {
		border-color: var(--color-accent);
		box-shadow: 0 2px 12px rgba(0, 0, 0, 0.06);
	}

	.card-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.source-badge {
		padding: 2px 8px;
		border-radius: var(--radius-sm);
		font-size: 11px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.3px;
	}

	.stars {
		font-size: 13px;
		color: #F59E0B;
		font-weight: 500;
	}

	.skill-name {
		margin: 0;
		font-size: 15px;
		font-weight: 600;
		color: var(--color-fg);
	}

	.skill-desc {
		margin: 0;
		font-size: 13px;
		color: var(--color-fg-secondary);
		line-height: 1.5;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.tags {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}

	.tag {
		padding: 2px 8px;
		border-radius: var(--radius-sm);
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
		font-size: 11px;
	}

	.card-actions {
		margin-top: var(--space-2);
	}

	.install-btn {
		width: 100%;
		padding: 8px 16px;
		border-radius: var(--radius-md);
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: 14px;
		font-weight: 500;
		cursor: pointer;
		transition: all 0.15s ease;
	}
	.install-btn:hover { opacity: 0.9; }
	.install-btn:disabled { opacity: 0.5; cursor: not-allowed; }

	.installed-label {
		display: block;
		text-align: center;
		padding: 8px 16px;
		font-size: 14px;
		font-weight: 500;
		color: #34C759;
	}

	.install-error {
		margin: 8px 0 0;
		padding: 6px 10px;
		border-radius: var(--radius-sm);
		background: rgba(239, 68, 68, 0.08);
		color: #EF4444;
		font-size: 12px;
		line-height: 1.4;
		word-break: break-word;
	}
</style>
