import { chatApi, streamEvents, type MessageDto } from '$lib/api';

class ChatStore {
	messages = $state<MessageDto[]>([]);
	streaming = $state(false);
	streamingText = $state('');
	isGenerating = $state(false);
	private unsubs: (() => void)[] = [];

	async loadHistory(sessionId: string) {
		try {
			this.messages = await chatApi.history(sessionId);
		} catch (e) {
			console.error('Failed to load history:', e);
			this.messages = [];
		}
	}

	async send(sessionId: string, content: string) {
		if (this.isGenerating || !content.trim()) return;

		// Save user message
		const userMsg = await chatApi.send(sessionId, content);
		this.messages = [...this.messages, userMsg];

		// Start streaming
		this.isGenerating = true;
		this.streaming = true;
		this.streamingText = '';

		// Subscribe to stream events
		this.cleanup();

		const unsubs = await Promise.all([
			streamEvents.onDelta(sessionId, (delta) => {
				this.streamingText += delta;
			}),
			streamEvents.onToolCall(sessionId, (call) => {
				console.log('Tool call:', call);
			}),
			streamEvents.onDone(sessionId, () => {
				if (this.streamingText) {
					const assistantMsg: MessageDto = {
						id: crypto.randomUUID(),
						session_id: sessionId,
						role: 'assistant',
						content: this.streamingText,
						tool_calls: null,
						tool_call_id: null,
						model_id: null,
						usage: null,
						created_at: Date.now(),
					};
					this.messages = [...this.messages, assistantMsg];
				}
				this.streaming = false;
				this.isGenerating = false;
				this.streamingText = '';
				this.cleanup();
			}),
			streamEvents.onError(sessionId, (message) => {
				console.error('Stream error:', message);
				this.streaming = false;
				this.isGenerating = false;
				this.streamingText = '';
				this.cleanup();
			}),
		]);

		this.unsubs = unsubs;
	}

	cleanup() {
		this.unsubs.forEach((u) => u());
		this.unsubs = [];
	}
}

export const chatStore = new ChatStore();
