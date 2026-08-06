use std::sync::Arc;
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
