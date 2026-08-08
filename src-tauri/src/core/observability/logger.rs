use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Fatal => "fatal",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "trace" => Self::Trace,
            "debug" => Self::Debug,
            "info" => Self::Info,
            "warn" => Self::Warn,
            "error" => Self::Error,
            "fatal" => Self::Fatal,
            _ => Self::Info,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub workflow_id: Option<String>,
    pub run_id: Option<String>,
    pub stage_id: Option<String>,
    pub event: String,
    pub model: Option<String>,
    pub tokens: Option<TokenUsage>,
    pub cost: Option<f64>,
    pub duration_ms: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt: u64,
    pub completion: u64,
}

pub struct AgentLogger {
    level: LogLevel,
    entries: std::sync::Mutex<Vec<LogEntry>>,
    max_entries: usize,
}

impl AgentLogger {
    pub fn new(level: LogLevel) -> Self {
        Self {
            level,
            entries: std::sync::Mutex::new(Vec::new()),
            max_entries: 1000,
        }
    }

    pub fn log(&self, entry: LogEntry) {
        if LogLevel::from_str(&entry.level) >= self.level {
            let mut entries = self.entries.lock().unwrap();
            entries.push(entry);
            let excess = entries.len().saturating_sub(self.max_entries);
            if excess > 0 {
                entries.drain(0..excess);
            }
        }
    }

    pub fn info(&self, event: &str, message: &str) {
        self.log(LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: "info".into(),
            session_id: None,
            agent_id: None,
            workflow_id: None,
            run_id: None,
            stage_id: None,
            event: event.into(),
            model: None,
            tokens: None,
            cost: None,
            duration_ms: None,
            message: message.into(),
        });
    }

    pub fn warn(&self, event: &str, message: &str) {
        self.log(LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: "warn".into(),
            session_id: None,
            agent_id: None,
            workflow_id: None,
            run_id: None,
            stage_id: None,
            event: event.into(),
            model: None,
            tokens: None,
            cost: None,
            duration_ms: None,
            message: message.into(),
        });
    }

    pub fn error(&self, event: &str, message: &str) {
        self.log(LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: "error".into(),
            session_id: None,
            agent_id: None,
            workflow_id: None,
            run_id: None,
            stage_id: None,
            event: event.into(),
            model: None,
            tokens: None,
            cost: None,
            duration_ms: None,
            message: message.into(),
        });
    }

    pub fn llm_call(
        &self,
        model: &str,
        tokens: TokenUsage,
        cost: f64,
        duration_ms: u64,
    ) {
        self.log(LogEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            level: "info".into(),
            session_id: None,
            agent_id: None,
            workflow_id: None,
            run_id: None,
            stage_id: None,
            event: "llm_call".into(),
            model: Some(model.into()),
            tokens: Some(tokens),
            cost: Some(cost),
            duration_ms: Some(duration_ms),
            message: format!("LLM 调用完成: {model}"),
        });
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries.lock().unwrap().clone()
    }

    pub fn entries_json_lines(&self) -> String {
        let entries = self.entries.lock().unwrap();
        entries.iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logger_captures_entries() {
        let logger = AgentLogger::new(LogLevel::Debug);
        logger.info("test", "hello");
        logger.warn("test", "warning");
        assert_eq!(logger.entries().len(), 2);
    }

    #[test]
    fn logger_filters_by_level() {
        let logger = AgentLogger::new(LogLevel::Warn);
        logger.info("test", "should not appear");
        logger.warn("test", "should appear");
        assert_eq!(logger.entries().len(), 1);
    }
}
