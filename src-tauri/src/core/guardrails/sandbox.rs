use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SandboxPolicy {
    pub filesystem: FilesystemPolicy,
    pub network: NetworkPolicy,
    pub process: ProcessPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemPolicy {
    pub allowed_paths: Vec<String>,
    pub denied_paths: Vec<String>,
    pub read_only_paths: Vec<String>,
    pub max_file_size: u64,
}

impl Default for FilesystemPolicy {
    fn default() -> Self {
        Self {
            allowed_paths: vec![".".into()],
            denied_paths: vec!["/etc".into(), "/proc".into(), "/sys".into()],
            read_only_paths: Vec::new(),
            max_file_size: 10 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    pub allowed_domains: Vec<String>,
    pub denied_domains: Vec<String>,
    pub allowed_ports: Vec<u16>,
    pub max_requests_per_minute: u32,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            allowed_domains: vec!["*".into()],
            denied_domains: Vec::new(),
            allowed_ports: vec![80, 443],
            max_requests_per_minute: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessPolicy {
    pub allowed_commands: Vec<String>,
    pub denied_commands: Vec<String>,
    pub max_execution_time_secs: u64,
    pub max_output_bytes: u64,
}

impl Default for ProcessPolicy {
    fn default() -> Self {
        Self {
            allowed_commands: vec!["ls".into(), "cat".into(), "grep".into(), "find".into()],
            denied_commands: vec!["rm".into(), "dd".into(), "mkfs".into(), "shutdown".into()],
            max_execution_time_secs: 30,
            max_output_bytes: 1024 * 1024,
        }
    }
}

impl SandboxPolicy {
    pub fn check_path(&self, path: &str) -> Result<(), String> {
        for denied in &self.filesystem.denied_paths {
            if path.starts_with(denied) {
                return Err(format!("路径 '{path}' 在黑名单中"));
            }
        }
        if !self.filesystem.allowed_paths.is_empty() {
            let allowed = self
                .filesystem
                .allowed_paths
                .iter()
                .any(|p| path.starts_with(p));
            if !allowed {
                return Err(format!("路径 '{path}' 不在白名单中"));
            }
        }
        Ok(())
    }

    pub fn check_domain(&self, domain: &str) -> Result<(), String> {
        for denied in &self.network.denied_domains {
            if domain == denied || domain.ends_with(&format!(".{denied}")) {
                return Err(format!("域名 '{domain}' 在黑名单中"));
            }
        }
        if !self.network.allowed_domains.contains(&"*".to_string())
            && !self.network.allowed_domains.contains(&domain.to_string())
        {
            return Err(format!("域名 '{domain}' 不在白名单中"));
        }
        Ok(())
    }

    pub fn check_command(&self, command: &str) -> Result<(), String> {
        let cmd = command.split_whitespace().next().unwrap_or(command);
        for denied in &self.process.denied_commands {
            if cmd == denied {
                return Err(format!("命令 '{cmd}' 被禁止"));
            }
        }
        if !self.process.allowed_commands.is_empty()
            && !self.process.allowed_commands.contains(&cmd.to_string())
        {
            return Err(format!("命令 '{cmd}' 不在白名单中"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_check_denied() {
        let policy = SandboxPolicy::default();
        assert!(policy.check_path("/etc/passwd").is_err());
    }

    #[test]
    fn path_check_allowed() {
        let policy = SandboxPolicy::default();
        assert!(policy.check_path("./src/main.rs").is_ok());
    }

    #[test]
    fn command_check_denied() {
        let policy = SandboxPolicy::default();
        assert!(policy.check_command("rm -rf /").is_err());
    }
}
