<script lang="ts">
	import { fileApi } from '$lib/api';
	import { composeStore } from '$lib/stores/compose.svelte';

	let {
		disabled = false,
		generating = false,
		onSend,
		onAbort
	}: {
		disabled?: boolean;
		generating?: boolean;
		onSend: (content: string, attachments?: string[]) => void;
		onAbort?: () => void;
	} = $props();

	let input = $state('');
	let textareaEl = $state<HTMLTextAreaElement | null>(null);
	type AgentMode = 'build' | 'compose';
	let mode = $state<AgentMode>('build');

	let attachments = $state<{ path: string; content: string }[]>([]);
	let attaching = $state(false);
	let attachPath = $state('');
	let attachError = $state('');
	let attachLoading = $state(false);
	let attachInputEl = $state<HTMLInputElement | null>(null);

	$effect(() => {
		if (attaching) attachInputEl?.focus();
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			send();
		}
	}

	function handleInput() {
		resize();
		// Auto-detect mode from slash commands
		if (input.startsWith('/compose')) {
			mode = 'compose';
		} else if (input.startsWith('/build')) {
			mode = 'build';
		}
	}

	function resize() {
		const el = textareaEl;
		if (!el) return;
		el.style.height = 'auto';
		el.style.height = Math.min(el.scrollHeight, 120) + 'px';
	}

	async function confirmAttach() {
		const path = attachPath.trim();
		if (!path || attachLoading) return;
		attachLoading = true;
		attachError = '';
		try {
			const content = await fileApi.readText(path);
			attachments = [...attachments, { path, content }];
			attachPath = '';
			attaching = false;
		} catch (e) {
			attachError = '读取失败: ' + String(e);
		} finally {
			attachLoading = false;
		}
	}

	function closeAttach() {
		attaching = false;
		attachPath = '';
		attachError = '';
	}

	function removeAttachment(path: string) {
		attachments = attachments.filter((a) => a.path !== path);
	}

	function send() {
		if (!input.trim() || disabled) return;
		const content = input.trim();
		const paths = attachments.map((a) => a.path);

		if (mode === 'compose') {
			// Strip /compose prefix if present
			const request = content.replace(/^\/compose\s*/, '');
			if (request) {
				composeStore.startCompose(request, '');
			}
		} else {
			// Build mode: strip /build prefix if present, then normal send
			const cleaned = content.replace(/^\/build\s*/, '');
			if (cleaned) {
				onSend(cleaned, paths);
			}
		}

		input = '';
		attachments = [];
		requestAnimationFrame(resize);
	}

	function handleAbort() {
		onAbort?.();
	}
</script>

<div class="composer">
	{#if attachments.length > 0}
		<div class="attach-chips">
			{#each attachments as a}
				<span class="attach-chip" title={a.path}>
					<span class="chip-name">{a.path.split(/[\\/]/).pop()}</span>
					<button class="chip-remove" onclick={() => removeAttachment(a.path)} title="移除附件">×</button>
				</span>
			{/each}
		</div>
	{/if}

	{#if attaching}
		<div class="attach-input-row">
			<input
				bind:this={attachInputEl}
				bind:value={attachPath}
				type="text"
				placeholder="输入文件路径，如 C:\Users\me\notes.txt"
				onkeydown={(e) => { if (e.key === 'Enter') { e.preventDefault(); confirmAttach(); } }}
				disabled={attachLoading}
			/>
			<button class="attach-add" onclick={confirmAttach} disabled={attachLoading || !attachPath.trim()}>
				{attachLoading ? '读取中…' : '添加'}
			</button>
			<button class="attach-cancel" onclick={closeAttach}>取消</button>
			{#if attachError}
				<span class="attach-error">{attachError}</span>
			{/if}
		</div>
	{/if}

	<div class="tools-row">
		<button class="attach-btn" onclick={() => { attaching = !attaching; }} title="添加附件">
			<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/>
			</svg>
		</button>

		<button
			class="mode-toggle"
			class:compose={mode === 'compose'}
			onclick={() => { mode = mode === 'build' ? 'compose' : 'build'; }}
			title={mode === 'build' ? '当前: Build 模式，点击切换到 Compose' : '当前: Compose 模式，点击切换到 Build'}
		>
			{#if mode === 'build'}
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2"/>
				</svg>
			{:else}
				<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M12 2L2 7l10 5 10-5-10-5z"/>
					<path d="M2 17l10 5 10-5"/>
					<path d="M2 12l10 5 10-5"/>
				</svg>
			{/if}
			<span class="mode-label">{mode === 'build' ? 'Build' : 'Compose'}</span>
		</button>
	</div>

	<div class="input-row">
		<textarea
			bind:this={textareaEl}
			bind:value={input}
			onkeydown={handleKeydown}
			oninput={handleInput}
			placeholder={mode === 'compose' ? '描述需求，Agent 将自动分析和执行...' : '输入消息...'}
			rows="1"
			disabled={disabled}
		></textarea>

		<button
			class="send-btn"
			onclick={generating ? handleAbort : send}
			disabled={!generating && (!input.trim() || disabled)}
			title={generating ? '停止生成' : '发送'}
		>
			{#if generating}
				{#if onAbort}
					<svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
						<rect x="6" y="6" width="12" height="12" rx="2"/>
					</svg>
				{:else}
					<span class="spinner"></span>
				{/if}
			{:else}
				<svg width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
					<path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"/>
				</svg>
			{/if}
		</button>
	</div>
</div>

<style>
	.composer {
		padding: 12px 16px;
		border-top: 1px solid var(--color-separator);
		background: var(--color-glass);
		backdrop-filter: saturate(180%) blur(20px);
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.attach-chips {
		display: flex;
		flex-wrap: wrap;
		gap: 6px;
	}
	.attach-chip {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		max-width: 260px;
		padding: 4px 10px;
		border-radius: 12px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		font-size: 13px;
		color: var(--color-fg);
	}
	.chip-name {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.chip-remove {
		border: none;
		background: transparent;
		color: var(--color-fg-tertiary);
		font-size: 15px;
		line-height: 1;
		cursor: pointer;
		padding: 0 2px;
	}
	.chip-remove:hover {
		color: var(--color-fg);
	}

	.attach-input-row {
		display: flex;
		align-items: center;
		gap: 8px;
		flex-wrap: wrap;
	}
	.attach-input-row input {
		flex: 1;
		min-width: 180px;
		padding: 8px 12px;
		border-radius: 10px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 14px;
		font-family: var(--font-sans);
		outline: none;
	}
	.attach-input-row input:focus {
		border-color: var(--color-accent);
	}
	.attach-add {
		padding: 8px 16px;
		border: none;
		border-radius: 10px;
		background: var(--color-accent);
		color: #fff;
		font-size: 14px;
		cursor: pointer;
		transition: all 0.12s;
	}
	.attach-add:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
	.attach-cancel {
		padding: 8px 12px;
		border: 1px solid var(--color-separator);
		border-radius: 10px;
		background: var(--color-bg);
		color: var(--color-fg-secondary);
		font-size: 14px;
		cursor: pointer;
	}
	.attach-error {
		flex-basis: 100%;
		font-size: 13px;
		color: var(--color-red);
	}

	.tools-row {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.attach-btn {
		width: 32px;
		height: 32px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg-secondary);
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		transition: all 0.12s;
	}
	.attach-btn:hover {
		border-color: var(--color-accent);
		color: var(--color-accent);
	}
	.attach-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.mode-toggle {
		display: flex;
		align-items: center;
		gap: 5px;
		padding: 5px 10px;
		border-radius: 8px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg-secondary);
		cursor: pointer;
		flex-shrink: 0;
		transition: all 0.15s ease;
		font-size: 12px;
		font-weight: 500;
		font-family: inherit;
	}
	.mode-toggle:hover {
		border-color: var(--color-accent);
		color: var(--color-accent);
	}
	.mode-toggle.compose {
		border-color: var(--color-accent);
		background: var(--color-accent);
		color: #fff;
	}
	.mode-label {
		white-space: nowrap;
	}

	.input-row {
		display: flex;
		gap: 8px;
		align-items: flex-end;
	}

	textarea {
		flex: 1;
		padding: 10px 14px;
		border-radius: 20px;
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: 15px;
		font-family: var(--font-sans);
		resize: none;
		outline: none;
		min-height: 40px;
		max-height: 120px;
		line-height: 1.4;
	}
	textarea:focus {
		border-color: var(--color-accent);
	}

	.send-btn {
		width: 40px;
		height: 40px;
		border-radius: 50%;
		border: none;
		background: var(--color-accent);
		color: #fff;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		transition: all 0.12s;
	}
	.send-btn:hover {
		background: var(--color-accent-hover);
	}
	.send-btn:active {
		transform: scale(0.95);
	}
	.send-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.spinner {
		width: 18px;
		height: 18px;
		border: 2px solid rgba(255, 255, 255, 0.3);
		border-top-color: #fff;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to { transform: rotate(360deg); }
	}
</style>
