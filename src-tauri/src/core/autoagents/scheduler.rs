use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;

// ── 任务调度器 ────────────────────────────────────────────

pub struct TaskScheduler {
    semaphore: Arc<Semaphore>,
    max_concurrent: usize,
}

impl TaskScheduler {
    pub fn new(max_concurrent: usize) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_concurrent,
        }
    }

    /// 获取并发许可
    pub async fn acquire(&self) -> tokio::sync::SemaphorePermit<'_> {
        self.semaphore.acquire().await.unwrap()
    }

    /// 获取当前可用许可数
    pub fn available(&self) -> usize {
        self.semaphore.available_permits()
    }

    /// 最大并发数
    pub fn max_concurrent(&self) -> usize {
        self.max_concurrent
    }
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new(4) // 默认 4 个并发 worker
    }
}

// ── 全局调度器 ────────────────────────────────────────────

static GLOBAL_SCHEDULER: OnceLock<TaskScheduler> = OnceLock::new();

/// 获取进程级全局调度器（懒初始化，默认 4 并发）
pub fn global() -> &'static TaskScheduler {
    GLOBAL_SCHEDULER.get_or_init(TaskScheduler::default)
}
