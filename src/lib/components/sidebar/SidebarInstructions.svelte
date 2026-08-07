<script lang="ts">
	import type { AgentContext } from '$lib/stores/context.svelte';
	import { sessionApi } from '$lib/api';
	import { agentStore } from '$lib/stores/agents.svelte';

	let { data }: { data: AgentContext } = $props();

	const instructions = $derived(data.instructions);
	const sorted = $derived([...instructions].sort((a, b) => b.priority - a.priority));

	let injectedPaths = $state<Set<string>>(new Set());
	let injectingPath = $state<string | null>(null);
	let injectError = $state<string | null>(null);

	function isInjected(path: string): boolean {
		const backend = data.instructions.find((f) => f.path === path)?.injected;
		return Boolean(backend) || injectedPaths.has(path);
	}

	async function handleInject(path: string) {
		if (!agentStore.currentSession || injectingPath) return;
		injectingPath = path;
		injectError = null;
		try {
			await sessionApi.injectFile(agentStore.currentSession.id, path);
			injectedPaths = new Set(injectedPaths).add(path);
		} catch (e) {
			injectError = e instanceof Error ? e.message : String(e);
		} finally {
			injectingPath = null;
		}
	}
</script>

<div class="instructions-panel">
	{#if injectError}
		<div class="inject-error">注入失败: {injectError}</div>
	{/if}
	{#if sorted.length === 0}
		<div class="empty">
			<span>无指令文件</span>
		</div>
	{:else}
		<div class="file-list">
			{#each sorted as file}
				<div class="file-item" class:injected={isInjected(file.path)}>
					<div class="file-header">
						<div class="file-icon">
							<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
								<polyline points="14 2 14 8 20 8"/>
							</svg>
						</div>
						<div class="file-info">
							<span class="file-name">{file.name}</span>
							<span class="file-meta">{file.lines} 行 · 优先级 {file.priority}</span>
						</div>
						<div class="inject-status" class:active={isInjected(file.path)}>
							{#if isInjected(file.path)}
								<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
									<polyline points="20 6 9 17 4 12"></polyline>
								</svg>
								注入
							{:else}
								排除
							{/if}
						</div>
					</div>
					<div class="file-footer">
						<span class="file-path">{file.path}</span>
						<button
							class="inject-btn"
							class:active={isInjected(file.path)}
							disabled={isInjected(file.path) || !agentStore.currentSession || injectingPath === file.path}
							onclick={() => handleInject(file.path)}
							title={!agentStore.currentSession ? '无当前会话' : '注入到当前会话'}
						>
							{#if isInjected(file.path)}
								已注入
							{:else if injectingPath === file.path}
								注入中...
							{:else}
								注入
							{/if}
						</button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.instructions-panel {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.empty {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 32px 0;
		font-size: 13px;
		color: var(--color-fg-secondary);
	}

	.file-list {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.file-item {
		padding: 10px 12px;
		background: var(--color-bg);
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		transition: border-color 0.15s ease;
	}
	.file-item:hover {
		border-color: var(--color-accent);
	}
	.file-item.injected {
		border-left: 3px solid var(--color-accent);
	}

	.file-header {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.file-icon {
		color: var(--color-fg-secondary);
		flex-shrink: 0;
	}

	.file-info {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.file-name {
		font-size: 13px;
		font-weight: 500;
		color: var(--color-fg);
	}

	.file-meta {
		font-size: 11px;
		color: var(--color-fg-secondary);
	}

	.inject-status {
		display: flex;
		align-items: center;
		gap: 4px;
		padding: 3px 8px;
		border-radius: 4px;
		font-size: 11px;
		font-weight: 500;
		color: var(--color-fg-secondary);
		background: var(--color-bg-secondary);
		flex-shrink: 0;
	}
	.inject-status.active {
		color: var(--color-green);
		background: color-mix(in srgb, var(--color-green) 10%, transparent);
	}

	.inject-error {
		margin-bottom: 8px;
		padding: 6px 10px;
		border-radius: 6px;
		font-size: 12px;
		color: var(--color-red);
		background: color-mix(in srgb, var(--color-red) 10%, transparent);
		word-break: break-all;
	}

	.file-footer {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-top: 6px;
	}

	.file-path {
		flex: 1;
		min-width: 0;
		font-size: 11px;
		font-family: var(--font-mono);
		color: var(--color-fg-secondary);
		word-break: break-all;
	}

	.inject-btn {
		flex-shrink: 0;
		padding: 3px 10px;
		border-radius: 4px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: 11px;
		font-weight: 500;
		cursor: pointer;
		transition: opacity 0.15s ease;
	}
	.inject-btn:hover:not(:disabled) { opacity: 0.9; }
	.inject-btn:disabled { opacity: 0.5; cursor: not-allowed; }
	.inject-btn.active {
		background: color-mix(in srgb, var(--color-green) 15%, transparent);
		color: var(--color-green);
	}
</style>
