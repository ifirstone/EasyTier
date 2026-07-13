use std::process::Command;

use anyhow::Result;
use tracing::{debug, info};

pub struct ProcessManager {
    bin_path: String,
    uri: String,
    machine_id: Option<String>,
}

impl ProcessManager {
    pub fn new(config: super::AgentConfig) -> Self {
        Self {
            bin_path: config.bin_path,
            uri: config.uri,
            machine_id: config.machine_id,
        }
    }

    pub fn start(&self, hostname: &str) -> Result<Option<u32>> {
        debug!(hostname = %hostname, "starting easytier-core");
        let mut cmd = Command::new(&self.bin_path);
        cmd.arg("-w").arg(&self.uri).arg("--hostname").arg(hostname);
        if let Some(mid) = &self.machine_id {
            cmd.arg("--machine-id").arg(mid);
        }

        let child = cmd.spawn()?;
        let pid = child.id();
        info!(pid = %pid, "started easytier-core");
        Ok(Some(pid))
    }

    pub fn stop(&self, pid: u32) -> Result<()> {
        debug!(pid = %pid, "stopping easytier-core");
        #[cfg(unix)]
        {
            let _ = Command::new("kill").arg(pid.to_string()).status();
        }
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .status();
        }
        Ok(())
    }

    pub fn is_running(&self, pid: Option<u32>) -> bool {
        let Some(_pid_val) = pid else { return false };
        #[cfg(unix)]
        {
            Command::new("kill")
                .arg("-0")
                .arg(_pid_val.to_string())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            true
        }
    }
}
