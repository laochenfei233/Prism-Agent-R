<script lang="ts">
	import { goto } from '$app/navigation';
	import { agentStore } from '$lib/stores/agents.svelte';
	import { chatStore } from '$lib/stores/chat.svelte';
	import type { MessageDto } from '$lib/api';

	let input = $state('');
	let messagesEnd = $state<HTMLElement>();

	$effect(() => {
		if (messagesEnd && chatStore.messages.length > 0) {
			messagesEnd.scrollIntoView({ behavior: 'smooth' });
		}
	});

	async function handleSend() {
		if (!input.trim() || !agentStore.currentSession) return;
		const content = input.trim();
		input = '';
		await chatStore.send(agentStore.currentSession.id, content);
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend();
		}
	}

	function formatTime(ts: number) {
		return new Date(ts).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' });
	}
</script>

<div class="chat-page">
	{#if !agentStore.currentSession}
		<div class="welcome">
			<div class="welcome-content">
				<img src="/icon.svg" alt="Prism" width="80" height="80" />
				<h1>Prism Agent</h1>
				<p>AI Agent 驱动的智能助手</p>

				{#if agentStore.agents.length === 0}
					<!-- Quick Setup Guide -->
					<div class="quick-setup">
						<h2>快速开始</h2>
						<div class="setup-steps">
							<div class="setup-step">
								<span class="step-num">1</span>
								<div>
									<strong>配置模型</strong>
									<p>添加 LLM Provider（OpenAI / Ollama / 自定义）</p>
								</div>
							</div>
							<div class="setup-step">
								<span class="step-num">2</span>
								<div>
									<strong>创建 Agent</strong>
									<p>设置 Agent 名称和系统提示词</p>
								</div>
							</div>
							<div class="setup-step">
								<span class="step-num">3</span>
								<div>
									<strong>开始对话</strong>
									<p>与 Agent 进行智能对话</p>
								</div>
							</div>
						</div>
						<button class="setup-btn" onclick={() => goto('/settings')}>
							⚙ 前往设置，配置模型
						</button>
					</div>
				{:else}
					<p>选择左侧 Agent 开始对话，或点击 <strong>+</strong> 创建新 Agent</p>
				{/if}
			</div>
		</div>
	{:else}
		<!-- Chat Header -->
		<div class="chat-header">
			<h2>{agentStore.currentAgent?.name || 'Agent'}</h2>
			<span class="session-label">{agentStore.currentSession.title || '新会话'}</span>
		</div>

		<!-- Messages -->
		<div class="messages">
			{#each chatStore.messages as msg}
				<div class="message" class:user={msg.role === 'user'} class:assistant={msg.role === 'assistant'}>
					<div class="message-avatar">
						{#if msg.role === 'user'}你{:else}{agentStore.currentAgent?.name?.[0] || 'A'}{/if}
					</div>
					<div class="message-body">
						<div class="message-content">{msg.content}</div>
						<div class="message-time">{formatTime(msg.created_at)}</div>
					</div>
				</div>
			{/each}

			{#if chatStore.streaming && chatStore.streamingText}
				<div class="message assistant">
					<div class="message-avatar">{agentStore.currentAgent?.name?.[0] || 'A'}</div>
					<div class="message-body">
						<div class="message-content">{chatStore.streamingText}<span class="cursor">|</span></div>
					</div>
				</div>
			{/if}

			<div bind:this={messagesEnd}></div>
		</div>

		<!-- Composer -->
		<div class="composer">
			<textarea
				bind:value={input}
				onkeydown={handleKeydown}
				placeholder="输入消息... (Enter 发送, Shift+Enter 换行)"
				rows="1"
				disabled={chatStore.isGenerating}
			></textarea>
			<button
				class="send-btn"
				onclick={handleSend}
				disabled={!input.trim() || chatStore.isGenerating}
			>
				{#if chatStore.isGenerating}
					<span class="spinner"></span>
				{:else}
					<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M22 2L11 13M22 2L15 22L11 13M22 2L2 9L11 13"/>
					</svg>
				{/if}
			</button>
		</div>
	{/if}
</div>

<style>
	.chat-page {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.welcome {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
	}
	.welcome-content {
		text-align: center;
		gap: var(--space-4);
		display: flex;
		flex-direction: column;
		align-items: center;
	}
	.welcome-content h1 {
		font-size: var(--text-3xl);
		font-weight: 700;
		margin: 0;
	}
	.welcome-content p {
		color: var(--color-fg-secondary);
		font-size: var(--text-lg);
		margin: 0;
	}

	.chat-header {
		padding: var(--space-3) var(--space-6);
		border-bottom: 1px solid var(--color-separator);
		display: flex;
		align-items: baseline;
		gap: var(--space-3);
	}
	.chat-header h2 {
		font-size: var(--text-lg);
		font-weight: 600;
		margin: 0;
	}
	.session-label {
		font-size: var(--text-sm);
		color: var(--color-fg-secondary);
	}

	.messages {
		flex: 1;
		overflow-y: auto;
		padding: var(--space-6);
		display: flex;
		flex-direction: column;
		gap: var(--space-6);
	}

	.message {
		display: flex;
		gap: var(--space-3);
		max-width: 800px;
		width: 100%;
	}
	.message.user {
		flex-direction: row-reverse;
	}

	.message-avatar {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: var(--color-bg-secondary);
		color: var(--color-fg-secondary);
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		font-size: var(--text-sm);
		flex-shrink: 0;
	}
	.message.user .message-avatar {
		background: var(--color-accent);
		color: #fff;
	}

	.message-body {
		flex: 1;
		min-width: 0;
	}

	.quick-setup {
		margin-top: var(--space-8);
		padding: var(--space-6);
		background: var(--color-bg-secondary);
		border-radius: var(--radius-xl);
		text-align: left;
		max-width: 480px;
	}
	.quick-setup h2 {
		font-size: var(--text-lg);
		font-weight: 600;
		margin: 0 0 var(--space-4);
		text-align: center;
	}
	.setup-steps {
		display: flex;
		flex-direction: column;
		gap: var(--space-4);
		margin-bottom: var(--space-6);
	}
	.setup-step {
		display: flex;
		gap: var(--space-3);
		align-items: flex-start;
	}
	.step-num {
		width: 24px;
		height: 24px;
		border-radius: 50%;
		background: var(--color-accent);
		color: #fff;
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 700;
		font-size: var(--text-sm);
		flex-shrink: 0;
	}
	.setup-step strong {
		display: block;
		font-size: var(--text-base);
		margin-bottom: 2px;
	}
	.setup-step p {
		margin: 0;
		font-size: var(--text-sm);
		color: var(--color-fg-secondary);
	}
	.setup-btn {
		width: 100%;
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-lg);
		border: none;
		background: var(--color-accent);
		color: #fff;
		font-size: var(--text-base);
		font-weight: 600;
		cursor: pointer;
		transition: background var(--duration-fast);
	}
	.setup-btn:hover { background: var(--color-accent-hover); }

	.message-content {
		padding: var(--space-3) var(--space-4);
		border-radius: var(--radius-lg);
		font-size: var(--text-base);
		line-height: 1.6;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.message.user .message-content {
		background: var(--color-accent);
		color: #fff;
		border-bottom-right-radius: var(--radius-sm);
	}
	.message.assistant .message-content {
		background: var(--color-bg-secondary);
		color: var(--color-fg);
		border-bottom-left-radius: var(--radius-sm);
	}

	.message-time {
		font-size: var(--text-xs);
		color: var(--color-fg-tertiary);
		margin-top: var(--space-1);
		padding: 0 var(--space-2);
	}
	.message.user .message-time {
		text-align: right;
	}

	.cursor {
		animation: blink 1s infinite;
		color: var(--color-accent);
	}

	.composer {
		padding: var(--space-4) var(--space-6);
		border-top: 1px solid var(--color-separator);
		display: flex;
		gap: var(--space-3);
		align-items: flex-end;
	}

	textarea {
		flex: 1;
		padding: var(--space-3);
		border-radius: var(--radius-lg);
		border: 1px solid var(--color-separator);
		background: var(--color-bg);
		color: var(--color-fg);
		font-size: var(--text-base);
		font-family: var(--font-sans);
		resize: none;
		outline: none;
		min-height: 44px;
		max-height: 150px;
		transition: border-color var(--duration-fast);
	}
	textarea:focus {
		border-color: var(--color-accent);
	}

	.send-btn {
		width: 44px;
		height: 44px;
		border-radius: 50%;
		border: none;
		background: var(--color-accent);
		color: #fff;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		transition: background var(--duration-fast), transform var(--duration-fast) var(--spring);
		flex-shrink: 0;
	}
	.send-btn:hover:not(:disabled) { background: var(--color-accent-hover); }
	.send-btn:active:not(:disabled) { transform: scale(0.92); }
	.send-btn:disabled { opacity: 0.4; cursor: not-allowed; }

	.spinner {
		width: 18px;
		height: 18px;
		border: 2px solid rgba(255,255,255,0.3);
		border-top-color: #fff;
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes blink { 0%, 100% { opacity: 1; } 50% { opacity: 0; } }
	@keyframes spin { to { transform: rotate(360deg); } }
</style>
