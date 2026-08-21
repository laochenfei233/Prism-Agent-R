import { composeApi } from '$lib/api';
import { listen } from '$lib/api/client';

// ── Types ─────────────────────────────────────────────────

export interface ComposeTask {
  id: string;
  description: string;
  acceptance: string;
  status: string;
  depends_on: string[];
  result: string | null;
  error: string | null;
}

export interface SpecDocument {
  summary: string;
  tasks: { id: string; title: string; description: string; acceptance: string }[];
  dependencies: Record<string, string[]>;
}

export interface ReviewResult {
  critical: string[];
  important: string[];
  minor: string[];
  ready_to_merge: boolean;
}

export interface ComposeSession {
  id: string;
  user_request: string;
  status: string;
  spec: SpecDocument | null;
  tasks: ComposeTask[];
  review: ReviewResult | null;
  created_at: number;
  updated_at: number;
}

export type ComposeStage =
  | 'idle'
  | 'brainstorming'
  | 'designing'
  | 'implementing'
  | 'verifying'
  | 'reviewing'
  | 'completed'
  | 'failed';

// ── Store ─────────────────────────────────────────────────

function createComposeStore() {
  let session = $state<ComposeSession | null>(null);
  let stage = $state<ComposeStage>('idle');
  let paused = $state(false);
  let error = $state<string | null>(null);

  // Throttle streaming updates
  let pendingTasks: ComposeTask[] | null = null;
  let pendingSpec: SpecDocument | null = null;
  let pendingReview: ReviewResult | null = null;
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  let unsubs: (() => void)[] = [];

  function scheduleFlush() {
    if (flushTimer) return;
    flushTimer = setTimeout(() => {
      flushTimer = null;
      if (pendingTasks !== null && session) {
        session.tasks = pendingTasks;
        session.updated_at = Date.now();
        pendingTasks = null;
      }
      if (pendingSpec !== null && session) {
        session.spec = pendingSpec;
        session.updated_at = Date.now();
        pendingSpec = null;
      }
      if (pendingReview !== null && session) {
        session.review = pendingReview;
        session.updated_at = Date.now();
        pendingReview = null;
      }
    }, 30);
  }

  function flushNow() {
    if (flushTimer) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    if (pendingTasks !== null && session) {
      session.tasks = pendingTasks;
      session.updated_at = Date.now();
      pendingTasks = null;
    }
    if (pendingSpec !== null && session) {
      session.spec = pendingSpec;
      session.updated_at = Date.now();
      pendingSpec = null;
    }
    if (pendingReview !== null && session) {
      session.review = pendingReview;
      session.updated_at = Date.now();
      pendingReview = null;
    }
  }

  function discardPending() {
    if (flushTimer) {
      clearTimeout(flushTimer);
      flushTimer = null;
    }
    pendingTasks = null;
    pendingSpec = null;
    pendingReview = null;
  }

  async function subscribeEvents(sessionId: string) {
    cleanup();

    const unlistenStage = await listen<{ session_id: string; stage: string }>(
      'compose:stage',
      (e) => {
        if (e.session_id !== sessionId) return;
        stage = e.stage as ComposeStage;
      },
    );

    const unlistenSpec = await listen<{ session_id: string; spec: SpecDocument }>(
      'compose:spec',
      (e) => {
        if (e.session_id !== sessionId) return;
        pendingSpec = e.spec;
        scheduleFlush();
      },
    );

    const unlistenTask = await listen<{ session_id: string; tasks: ComposeTask[] }>(
      'compose:task',
      (e) => {
        if (e.session_id !== sessionId) return;
        pendingTasks = e.tasks;
        scheduleFlush();
      },
    );

    const unlistenProgress = await listen<{
      session_id: string;
      completed: number;
      total: number;
    }>('compose:progress', (e) => {
      if (e.session_id !== sessionId) return;
      // Progress is derived from tasks; event triggers re-render
    });

    const unlistenReview = await listen<{
      session_id: string;
      review: ReviewResult;
    }>('compose:review', (e) => {
      if (e.session_id !== sessionId) return;
      pendingReview = e.review;
      scheduleFlush();
    });

    const unlistenDone = await listen<{ session_id: string }>('compose:done', (e) => {
      if (e.session_id !== sessionId) return;
      flushNow();
      stage = 'completed';
    });

    const unlistenErr = await listen<{ session_id: string; message: string }>(
      'compose:error',
      (e) => {
        if (e.session_id !== sessionId) return;
        flushNow();
        stage = 'failed';
        error = e.message;
      },
    );

    unsubs = [
      unlistenStage,
      unlistenSpec,
      unlistenTask,
      unlistenProgress,
      unlistenReview,
      unlistenDone,
      unlistenErr,
    ];
  }

  function cleanup() {
    unsubs.forEach((u) => u());
    unsubs = [];
    discardPending();
  }

  // ── Computed helpers ───────────────────────────────────

  function completedTaskCount(): number {
    return session?.tasks.filter((t) => t.status === 'completed').length ?? 0;
  }

  function totalTaskCount(): number {
    return session?.tasks.length ?? 0;
  }

  function progressPercent(): number {
    const total = totalTaskCount();
    if (total === 0) return 0;
    return Math.round((completedTaskCount() / total) * 100);
  }

  // ── Public API ────────────────────────────────────────

  function isActive(): boolean {
    return stage !== 'idle' && stage !== 'completed' && stage !== 'failed';
  }

  async function startCompose(request: string, agentId: string) {
    error = null;
    paused = false;
    try {
      const result = await composeApi.start(request, agentId);
      if (result && typeof result === 'object' && 'id' in (result as Record<string, unknown>)) {
        session = result as unknown as ComposeSession;
      } else {
        session = {
          id: 'pending',
          user_request: request,
          status: 'running',
          spec: null,
          tasks: [],
          review: null,
          created_at: Date.now(),
          updated_at: Date.now(),
        };
      }
      stage = 'brainstorming';
      if (session.id !== 'pending') {
        await subscribeEvents(session.id);
      }
    } catch (e) {
      console.error('Failed to start compose:', e);
      error = String(e);
      stage = 'failed';
    }
  }

  async function pauseCompose() {
    if (!session || session.id === 'pending') return;
    try {
      await composeApi.pause(session.id);
      paused = true;
    } catch (e) {
      console.error('Failed to pause compose:', e);
    }
  }

  async function resumeCompose() {
    if (!session || session.id === 'pending') return;
    try {
      await composeApi.resume(session.id);
      paused = false;
    } catch (e) {
      console.error('Failed to resume compose:', e);
    }
  }

  async function stopCompose() {
    if (!session || session.id === 'pending') {
      reset();
      return;
    }
    try {
      await composeApi.stop(session.id);
    } catch (e) {
      console.error('Failed to stop compose:', e);
    }
    cleanup();
    stage = 'idle';
    session = null;
    paused = false;
    error = null;
  }

  function reset() {
    cleanup();
    stage = 'idle';
    session = null;
    paused = false;
    error = null;
  }

  return {
    get session() {
      return session;
    },
    get stage() {
      return stage;
    },
    get paused() {
      return paused;
    },
    get error() {
      return error;
    },
    get active() {
      return isActive();
    },
    completedTaskCount,
    totalTaskCount,
    progressPercent,
    startCompose,
    pauseCompose,
    resumeCompose,
    stopCompose,
    reset,
  };
}

export const composeStore = createComposeStore();
