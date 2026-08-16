<script lang="ts">
	import type { MessageDto } from '$lib/api';
	import MarkdownViewer from './MarkdownViewer.svelte';
	import ToolCallCard from './ToolCallCard.svelte';

	let { message }: { message: MessageDto } = $props();

	function toolCalls(): unknown[] {
		const raw = message.tool_calls;
		if (raw == null) return [];
		let value: unknown = raw;
		if (typeof raw === 'string') {
			try {
				value = JSON.parse(raw);
			} catch {
				return [];
			}
		}
		if (Array.isArray(value)) return value.filter((v) => v && typeof v === 'object');
		if (value && typeof value === 'object') return [value];
		return [];
	}

	// Extract thinking/reasoning content from message metadata
	// Reasoning is stored in usage.reasoning_content or as a separate field
	function getReasoning(): string {
		if (!message.usage) return '';
		const usage = message.usage as any;
		return usage?.reasoning_content ?? '';
	}

	const reasoning = $derived(getReasoning());
</script>

<div class="bubble-wrap" class:user={message.role === 'user'}>
	<div class="bubble">
		{#if message.role === 'user'}
			{message.content}
		{:else}
			{#if reasoning}
				<div class="thinking-section">
					{#await import('./ThinkingBlock.svelte') then { default: ThinkingBlock }}
						<ThinkingBlock content={reasoning} />
					{/await}
				</div>
			{/if}
			{#if message.content}
				<MarkdownViewer content={message.content} />
			{/if}
			{#if toolCalls().length > 0}
				<div class="tool-calls">
					{#each toolCalls() as call, i (i)}
						<ToolCallCard call={call} />
					{/each}
				</div>
			{/if}
		{/if}
	</div>
</div>

<style>
	.bubble-wrap {
		display: flex;
		max-width: 80%;
		min-width: 0;
	}

	.bubble-wrap.user {
		justify-content: flex-end;
		margin-left: auto;
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
	}

	.bubble-wrap.user .bubble {
		background: var(--color-accent);
		color: #fff;
		border-bottom-right-radius: 4px;
		border-bottom-left-radius: 18px;
		white-space: pre-wrap;
	}

	.tool-calls {
		margin-top: 8px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.thinking-section {
		margin-bottom: 8px;
	}
</style>
