import { chatApi, streamEvents, type MessageDto } from '$lib/api';
import { invoke } from '$lib/api/client';

class ChatStore {
	messages = $state<MessageDto[]>([]);
	streaming = $state(false);
	streamingText = $state('');
	streamingReasoningText = $state('');
	isGenerating = $state(false);
	private unsubs: (() => void)[] = [];
	// Throttle: deltas accumulate here and flush to streamingText at most once
	// per ~30ms so the markdown renderer isn't re-run on every token.
	private pendingDelta = '';
	private pendingReasoning = '';
	private flushTimer: ReturnType<typeof setTimeout> | null = null;

	private scheduleFlush() {
		if (this.flushTimer) return;
		this.flushTimer = setTimeout(() => {
			this.flushTimer = null;
			if (this.pendingDelta) {
				this.streamingText += this.pendingDelta;
				this.pendingDelta = '';
			}
			if (this.pendingReasoning) {
				this.streamingReasoningText += this.pendingReasoning;
				this.pendingReasoning = '';
			}
		}, 30);
	}

	private flushNow() {
		if (this.flushTimer) {
			clearTimeout(this.flushTimer);
			this.flushTimer = null;
		}
		if (this.pendingDelta) {
			this.streamingText += this.pendingDelta;
			this.pendingDelta = '';
		}
		if (this.pendingReasoning) {
			this.streamingReasoningText += this.pendingReasoning;
			this.pendingReasoning = '';
		}
	}

	private discardPending() {
		if (this.flushTimer) {
			clearTimeout(this.flushTimer);
			this.flushTimer = null;
		}
		this.pendingDelta = '';
		this.pendingReasoning = '';
	}

	async loadHistory(sessionId: string) {
		try {
			this.messages = await chatApi.history(sessionId);
		} catch (e) {
			console.error('Failed to load history:', e);
			this.messages = [];
		}
	}

	async send(sessionId: string, content: string, attachments?: string[]) {
		if (this.isGenerating || !content.trim()) return;

		// Save user message
		const userMsg = await chatApi.send(sessionId, content, attachments);
		this.messages = [...this.messages, userMsg];

		// Start streaming
		this.isGenerating = true;
		this.streaming = true;
		this.streamingText = '';
		this.streamingReasoningText = '';

		// Subscribe to stream events
		this.cleanup();

		const unsubs = await Promise.all([
			streamEvents.onDelta(sessionId, (delta) => {
				this.pendingDelta += delta;
				this.scheduleFlush();
			}),
			streamEvents.onReasoning(sessionId, (delta) => {
				this.pendingReasoning += delta;
				this.scheduleFlush();
			}),
			streamEvents.onToolCall(sessionId, (call) => {
				console.log('Tool call:', call);
			}),
			streamEvents.onDone(sessionId, () => {
				// Flush buffered deltas so the final chunk renders before reset.
				this.flushNow();
				// Reload history to get the assistant message from server
				this.loadHistory(sessionId);
				this.streaming = false;
				this.isGenerating = false;
				this.streamingText = '';
				this.streamingReasoningText = '';
				this.cleanup();
			}),
			streamEvents.onError(sessionId, (message) => {
				console.error('Stream error:', message);
				this.flushNow();
				this.streaming = false;
				this.isGenerating = false;
				this.streamingText = '';
				this.streamingReasoningText = '';
				this.cleanup();
			}),
		]);

		this.unsubs = unsubs;
	}

	cleanup() {
		this.unsubs.forEach((u) => u());
		this.unsubs = [];
		this.discardPending();
	}

	async abort(sessionId: string) {
		if (!this.isGenerating) return;
		try {
			await invoke<void>('chat_abort', { sessionId });
		} catch (e) {
			console.error('Failed to abort generation:', e);
		}
		this.cleanup();
		this.streaming = false;
		this.isGenerating = false;
		this.streamingText = '';
		this.streamingReasoningText = '';
	}
}

export const chatStore = new ChatStore();
