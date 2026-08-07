<script lang="ts">
	import type { MessageDto } from '$lib/api';
	import MessageBubble from './MessageBubble.svelte';
	import MarkdownViewer from './MarkdownViewer.svelte';

	let {
		messages = [],
		streaming = false,
		streamingText = ''
	}: {
		messages: MessageDto[];
		streaming?: boolean;
		streamingText?: string;
	} = $props();

	let listEl = $state<HTMLElement | null>(null);
	let stick = $state(true);

	function handleScroll() {
		const el = listEl;
		if (!el) return;
		stick = el.scrollTop + el.clientHeight >= el.scrollHeight - 48;
	}

	$effect(() => {
		// 追踪内容变化以便自动滚动
		void messages.length;
		void streamingText;
		const el = listEl;
		if (el && stick) el.scrollTop = el.scrollHeight;
	});
</script>

<div class="message-list" bind:this={listEl} onscroll={handleScroll}>
	{#if messages.length === 0 && !streaming}
		<div class="empty-state">
			<div class="empty-icon">
				<svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/></svg>
			</div>
			<p>开始对话吧</p>
			<span>发送一条消息，AI 助手会在这里回复</span>
		</div>
	{:else}
		{#each messages as msg (msg.id)}
			<MessageBubble message={msg} />
		{/each}

		{#if streaming}
			<div class="bubble-wrap">
				<div class="bubble streaming">
					{#if streamingText}
						<MarkdownViewer content={streamingText} streaming={true} />
					{/if}
					<span class="cursor">|</span>
				</div>
			</div>
		{/if}
	{/if}
</div>

<style>
	.message-list {
		flex: 1;
		overflow-y: auto;
		padding: 20px;
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.empty-state {
		margin: auto;
		text-align: center;
		color: var(--color-fg-tertiary);
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 6px;
	}

	.empty-icon {
		font-size: 36px;
		opacity: 0.5;
	}

	.empty-state p {
		margin: 0;
		font-size: var(--text-headline);
		font-weight: var(--font-weight-semibold);
		color: var(--color-fg-secondary);
	}

	.empty-state span {
		font-size: var(--text-footnote);
	}

	.bubble-wrap {
		display: flex;
		max-width: 80%;
		min-width: 0;
	}

	.bubble {
		padding: 10px 14px;
		border-radius: 18px;
		font-size: var(--text-subheadline);
		line-height: 1.5;
		color: var(--color-fg);
		background: var(--color-bg-secondary);
		word-break: break-word;
		border-bottom-left-radius: 4px;
		min-width: 0;
		max-width: 100%;
	}

	.cursor {
		animation: blink 1s infinite;
		color: var(--color-accent);
		font-weight: var(--font-weight-bold);
	}

	@keyframes blink {
		0%, 100% { opacity: 1; }
		50% { opacity: 0; }
	}
</style>
