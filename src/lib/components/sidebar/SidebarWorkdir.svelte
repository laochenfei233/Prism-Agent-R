<script lang="ts">
	import { invoke } from '$lib/api/client';
	import { contextStore } from '$lib/stores/context.svelte';
	import type { AgentContext } from '$lib/stores/context.svelte';

	let { data }: { data: AgentContext } = $props();

	const workspace = $derived(data.workspace);

	let editing = $state(false);
	let draftPath = $state('');
	let busy = $state(false);
	let error = $state<string | null>(null);
	let watchEnabled = $state(false);

	function startEdit() {
		draftPath = workspace.current_dir;
		error = null;
		editing = true;
	}

	async function submit() {
		const path = draftPath.trim();
		if (!path || busy) return;
		await setDir(path);
		if (!error) editing = false;
	}

	async function setDir(dir: string) {
		busy = true;
		error = null;
		try {
			await invoke('workspace_set', { path: dir });
			await contextStore.refresh();
			if (watchEnabled) {
				void invoke('fs_watch', { workdir: dir, enable: true }).catch((e) =>
					console.error('fs_watch failed:', e)
				);
			}
		} catch (e) {
			error = errMessage(e);
		} finally {
			busy = false;
		}
	}

	async function toggleWatch() {
		const dir = workspace.current_dir;
		if (!dir || busy) return;
		const enable = !watchEnabled;
		watchEnabled = enable;
		try {
			await invoke('fs_watch', { workdir: dir, enable });
		} catch (e) {
			watchEnabled = !enable;
			error = errMessage(e);
		}
	}

	function errMessage(e: unknown): string {
		if (typeof e === 'string') return e;
		if (e && typeof e === 'object' && 'message' in e) return String((e as { message: unknown }).message);
		return String(e);
	}
</script>

<div class="workdir-panel">
	<!-- Current Directory -->
	<div class="section">
		<div class="section-label">
			<span>当前目录</span>
			{#if !editing}
				<button class="edit-btn" onclick={startEdit} disabled={busy} title="编辑目录">
					<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M17 3a2.83 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5z"/>
					</svg>
				</button>
			{/if}
		</div>

		{#if editing}
			<div class="edit-row">
				<input
					type="text"
					bind:value={draftPath}
					placeholder="输入工作目录路径"
					onkeydown={(e) => {
						if (e.key === 'Enter') void submit();
						if (e.key === 'Escape') editing = false;
					}}
				/>
				<div class="edit-actions">
					<button class="btn-confirm" onclick={() => void submit()} disabled={busy}>
						{busy ? '切换中…' : '确定'}
					</button>
					<button class="btn-cancel" onclick={() => (editing = false)} disabled={busy}>取消</button>
				</div>
			</div>
		{:else if workspace.current_dir}
			<div class="dir-path">
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
				</svg>
				<span class="path-text">{workspace.current_dir}</span>
			</div>
		{:else}
			<div class="warn">未设置工作目录</div>
		{/if}

		{#if error}
			<div class="error">{error}</div>
		{/if}

		{#if workspace.current_dir}
			<button class="watch-row" onclick={() => void toggleWatch()} disabled={busy}>
				<input type="checkbox" checked={watchEnabled} onchange={() => void toggleWatch()} onclick={(e) => e.stopPropagation()} />
				<span>监视文件变更（fs watch）</span>
				<span class="watch-state">{watchEnabled ? '开启' : '关闭'}</span>
			</button>
		{/if}
	</div>

	<!-- Binding Status -->
	<div class="section">
		<div class="section-label">绑定状态</div>
		{#if workspace.bound_agent_id}
			<div class="badge badge-active">
				<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<polyline points="20 6 9 17 4 12"></polyline>
				</svg>
				已绑定: {workspace.bound_agent_id}
			</div>
		{:else}
			<div class="badge badge-inactive">
				未绑定
			</div>
		{/if}
	</div>

	<!-- Recent Directories -->
	{#if workspace.recent_dirs.length > 0}
		<div class="section">
			<div class="section-label">最近目录</div>
			<div class="recent-list">
				{#each workspace.recent_dirs as dir}
					<button
						class="recent-item"
						onclick={() => void setDir(dir)}
						disabled={busy}
						title="切换到该目录"
					>
						<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<circle cx="12" cy="12" r="1"></circle>
						</svg>
						<span class="recent-path">{dir}</span>
					</button>
				{/each}
			</div>
		</div>
	{/if}
</div>

<style>
	.workdir-panel {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.section {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.section-label {
		display: flex;
		align-items: center;
		justify-content: space-between;
		font-size: 11px;
		font-weight: 600;
		color: var(--color-fg-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.edit-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 22px;
		height: 22px;
		border-radius: 5px;
		border: none;
		background: transparent;
		color: var(--color-fg-secondary);
		cursor: pointer;
		transition: background 0.15s ease, color 0.15s ease;
	}
	.edit-btn:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-accent);
	}
	.edit-btn:disabled {
		opacity: 0.4;
		cursor: default;
	}

	.edit-row {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.edit-row input {
		padding: 6px 10px;
		border-radius: 6px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 12px;
		font-family: var(--font-mono);
		outline: none;
	}
	.edit-row input:focus {
		border-color: var(--color-accent);
	}

	.edit-actions {
		display: flex;
		gap: 6px;
	}

	.btn-confirm {
		padding: 4px 12px;
		border-radius: 6px;
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: 12px;
		font-weight: 500;
		cursor: pointer;
	}
	.btn-confirm:hover { opacity: 0.9; }

	.btn-cancel {
		padding: 4px 12px;
		border-radius: 6px;
		border: 1px solid var(--color-separator);
		background: transparent;
		color: var(--color-fg-secondary);
		font-size: 12px;
		cursor: pointer;
	}
	.btn-cancel:hover {
		background: var(--color-bg-tertiary);
	}

	.dir-path {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 10px 12px;
		background: var(--color-bg);
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		color: var(--color-fg);
	}

	.path-text {
		font-size: 13px;
		font-family: var(--font-mono);
		word-break: break-all;
	}

	.warn,
	.error {
		font-size: 12px;
		color: var(--color-red, #ef4444);
		padding: 6px 8px;
		background: rgba(239, 68, 68, 0.08);
		border-radius: 4px;
		word-break: break-all;
	}

	.watch-row {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 6px 8px;
		border-radius: 6px;
		border: none;
		background: none;
		color: var(--color-fg-secondary);
		font-size: 12px;
		cursor: pointer;
		text-align: left;
		transition: background 0.15s ease;
	}
	.watch-row:hover {
		background: var(--color-bg-tertiary);
	}

	.watch-row input {
		accent-color: var(--color-accent);
		margin: 0;
		cursor: pointer;
	}

	.watch-state {
		margin-left: auto;
		font-size: 11px;
	}

	.badge {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		padding: 6px 10px;
		border-radius: 6px;
		font-size: 12px;
		font-weight: 500;
	}

	.badge-active {
		background: rgba(16, 185, 129, 0.1);
		color: var(--color-green, #10b981);
	}

	.badge-inactive {
		background: var(--color-bg);
		color: var(--color-fg-secondary);
		border: 1px solid var(--color-separator);
	}

	.recent-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.recent-item {
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 6px 10px;
		border-radius: 6px;
		border: none;
		background: none;
		font-size: 12px;
		color: var(--color-fg-secondary);
		cursor: pointer;
		text-align: left;
		width: 100%;
		transition: background 0.15s ease;
	}
	.recent-item:hover {
		background: var(--color-bg-tertiary);
		color: var(--color-fg);
	}
	.recent-item:disabled {
		opacity: 0.5;
		cursor: default;
	}

	.recent-path {
		font-family: var(--font-mono);
		word-break: break-all;
	}
</style>
