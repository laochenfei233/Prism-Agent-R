use std::path::PathBuf;

pub fn app_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("prism-agent")
}

pub fn wiki_dir() -> PathBuf {
    app_data_dir().join("wiki")
}

pub fn skill_dir() -> PathBuf {
    app_data_dir().join("skills")
}

pub fn memory_dir() -> PathBuf {
    app_data_dir().join("memory")
}

pub fn log_dir() -> PathBuf {
    app_data_dir().join("logs")
}

pub fn meetings_dir() -> PathBuf {
    app_data_dir().join("meetings")
}
