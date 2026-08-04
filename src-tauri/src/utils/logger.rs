use tracing_subscriber::{fmt, EnvFilter};

pub fn init_logger() {
    let log_dir = super::paths::log_dir();
    let _ = std::fs::create_dir_all(&log_dir);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .init();
}
