use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub server_url: String,
    pub token: String,
    pub agent_id: String,
    pub check_interval: u64,
    pub bin_path: String,
    pub uri: String,
    pub key: String,
    pub machine_id: Option<String>,
    pub arch: Option<String>,
    pub log: bool,
    pub owner: String,
    #[serde(skip)]
    pub no_install: bool,
    #[serde(skip)]
    pub config_file: PathBuf,
}

impl AgentConfig {
    pub fn defaults() -> Self {
        Self {
            server_url: "http://localhost:11211".to_string(),
            token: String::new(),
            agent_id: String::new(),
            check_interval: 180,
            bin_path: "/usr/local/bin/easytier-core".to_string(),
            uri: String::new(),
            key: "sdwan".to_string(),
            machine_id: None,
            arch: None,
            log: false,
            owner: "default".to_string(),
            no_install: false,
            config_file: default_config_path(),
        }
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let mut cfg: Self = serde_json::from_str(&contents)?;
        cfg.config_file = path.to_path_buf();
        Ok(cfg)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.config_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&self.config_file, json)?;
        Ok(())
    }
}

pub fn default_config_path() -> PathBuf {
    let etc_path = PathBuf::from("/etc/easytier-agent/config.json");
    if etc_path.parent().map(|p| p.exists()).unwrap_or(false) {
        return etc_path;
    }
    if let Some(parent) = etc_path.parent() {
        if std::fs::create_dir_all(parent).is_ok() {
            return etc_path;
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".config/easytier-agent/config.json")
}

pub fn is_installed() -> bool {
    Path::new("/usr/local/bin/easytier-agent").exists()
        && Path::new("/etc/systemd/system/easytier-agent.service").exists()
}

pub fn is_running_under_systemd() -> bool {
    std::env::var("INVOCATION_ID").is_ok() || std::env::var("SYSTEMD_EXEC_PID").is_ok()
}

pub fn installed_binary_path() -> PathBuf {
    PathBuf::from("/usr/local/bin/easytier-agent")
}

pub fn service_file_path() -> PathBuf {
    PathBuf::from("/etc/systemd/system/easytier-agent.service")
}

pub fn install_dir() -> PathBuf {
    PathBuf::from("/etc/easytier-agent")
}
