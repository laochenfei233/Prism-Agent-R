import { chatApi, streamEvents, type MessageDto } from '$lib/api';
import { invoke } from '$lib/api/client';

export interface StreamingToolCall {
  id: string;
  name: string;
  argumentsText: string;
  status: 'running' | 'done' | 'error';
  output?: string;
}

class ChatStore {
  messages = $state<MessageDto[]>([]);
  streaming = $state(false);
  streamingText = $state('');
  streamingReasoningText = $state('');
  streamingToolCalls = $state<StreamingToolCall[]>([]);
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
    this.streamingToolCalls = [];

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
        const argsText =
          typeof call.arguments === 'string'
            ? call.arguments
            : JSON.stringify(call.arguments ?? {}, null, 2);
        this.streamingToolCalls = [
          ...this.streamingToolCalls,
          {
            id: call.id,
            name: call.name,
            argumentsText: argsText,
            status: 'running',
          },
        ];
      }),
      streamEvents.onToolResult(sessionId, (result) => {
        this.streamingToolCalls = this.streamingToolCalls.map((c) =>
          c.id === result.call_id
            ? {
                ...c,
                status: result.is_error ? 'error' : 'done',
                output: result.output,
              }
            : c,
        );
      }),
      streamEvents.onDone(sessionId, () => {
        // Flush buffered deltas so the final chunk renders before reset.
        this.flushNow();
        this.streamingToolCalls = [];
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
        this.streamingToolCalls = [];
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
    this.streamingToolCalls = [];
  }
}

export const chatStore = new ChatStore();
