<script lang="ts">
	let {
		disabled = false,
		generating = false,
		onSend,
		onAbort
	}: {
		disabled?: boolean;
		generating?: boolean;
		onSend: (content: string) => void;
		onAbort?: () => void;
	} = $props();

	let input = $state('');
	let textareaEl = $state<HTMLTextAreaElement | null>(null);

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			send();
		}
	}

	function handleInput() {
		resize();
	}

	function resize() {
		const el = textareaEl;
		if (!el) return;
		el.style.height = 'auto';
		el.style.height = Math.min(el.scrollHeight, 120) + 'px';
	}

	function send() {
		if (!input.trim() || disabled) return;
		onSend(input.trim());
		input = '';
		requestAnimationFrame(resize);
	}

	function handleAbort() {
		onAbort?.();
	}
</script>

<div class="composer">
	<button class="attach-btn" disabled title="附件（即将支持）">
		<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
			<path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"/>
		</svg>
	</button>

	<textarea
		bind:this={textareaEl}
		bind:value={input}
		onkeydown={handleKeydown}
		oninput={handleInput}
		placeholder="输入消息..."
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

<style>
	.composer {
		padding: 12px 16px;
		border-top: 1px solid var(--color-separator);
		background: var(--color-glass);
		backdrop-filter: saturate(180%) blur(20px);
		display: flex;
		gap: 10px;
		align-items: flex-end;
	}

	.attach-btn {
		width: 40px;
		height: 40px;
		border-radius: 50%;
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
	.attach-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
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
